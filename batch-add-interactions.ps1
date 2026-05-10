<#
.SYNOPSIS
  批量为 hotspot/drag_drop 题补充 Interaction Options/Targets 结构化数据。
  每轮处理一个 MD 文件中的缺失题目，Copilot CLI 视觉读 PDF 后原地修改 MD。
  全部完成后执行一次 SQLite 重导入。

.PARAMETER Rounds
  最大处理轮次（默认 30，超过自动停止）。

.PARAMETER Model
  Copilot CLI 使用的模型（默认 gpt-5.5）。

.PARAMETER Effort
  Copilot CLI effort 级别（默认 medium）。

.PARAMETER SkipImport
  跳过最终导入步骤（调试用）。

.EXAMPLE
  .\batch-add-interactions.ps1 -Rounds 30 -Model gpt-5.5 -Effort medium
#>
param(
    [int]$Rounds = 30,
    [string]$Model = 'gpt-5.5',
    [string]$Effort = 'medium',
    [switch]$SkipImport,
    [string]$Python = 'C:/Users/kukisama/AppData/Local/Programs/Python/Python312/python.exe',
    [string]$Db = 'question-banks/SC-100.sqlite'
)

$DetectScript = '.github/skills/sc100-vision-md/scripts/detect_missing_interactions.py'
$ImportScript = '.github/skills/sc100-vision-md/scripts/import_question_md_to_sqlite.py'

$processed = 0

for ($i = 1; $i -le $Rounds; $i++) {
    Write-Host "`n===== Round $i / $Rounds =====" -ForegroundColor Cyan

    # 1. 检测下一个缺失批次
    $json = & $Python $DetectScript --db $Db
    $state = $json | ConvertFrom-Json

    if ($state.done) {
        Write-Host "All done: $($state.done_reason)" -ForegroundColor Green
        break
    }

    Write-Host "Remaining: $($state.total_missing) questions in $($state.total_batches) batches" -ForegroundColor Yellow

    # 取第一个批次
    $batch = $state.batches[0]
    $mdFile = $batch.md_file
    $pdfFile = $batch.pdf_file

    # 构造题目列表描述
    $qDescs = @()
    foreach ($q in $batch.questions) {
        $qDescs += "Q$($q.number)($($q.type), pages $($q.source_pages))"
    }
    $qList = $qDescs -join ', '
    $qCount = $batch.questions.Count

    Write-Host "Processing: $mdFile -> $qList" -ForegroundColor White

    # 2. 构造 prompt
    $prompt = @"
用 sc100-vision-md skill 的 Regenerate Mode 为以下题目补充结构化交互数据。

目标 MD 文件: $mdFile
PDF 文件: $pdfFile
需要处理的题目 ($qCount 道): $qList

具体任务:
1. 用 render_pdf_pages.py 渲染每道题 source_pages 对应的 PDF 页
2. 视觉读取每道题的 HOTSPOT 下拉框或 DRAG DROP 候选区
3. 为每道题补充 Interaction Options 和 Interaction Targets 表格
4. 原地编辑 $mdFile，在每道题的 Answer Area 之后、Source Answer 之前插入结构化表格
5. 不要修改题目文字、答案、解析等其他内容
6. 格式参考 output/vision-md/sc-100-structured-overrides/sc-100-questions-001-010-023-032-042.md 中已有的 Interaction Options/Targets

注意:
- hotspot 下拉题: 每行有独立的下拉候选项，用不同 Group 区分
- drag_drop 题: 所有候选项共享一个池子
- 必须保留干扰项 (Distractor = Yes)
- Option ID 用 opt-xxx 格式，Target ID 用 target-N 格式
- 不要创建新 MD 文件，只原地编辑
"@

    # 3. 调用 Copilot CLI
    copilot --model $Model --effort $Effort -p $prompt --allow-all

    if ($LASTEXITCODE -ne 0) {
        Write-Host "Copilot CLI failed with exit code $LASTEXITCODE" -ForegroundColor Red
        break
    }

    $processed++
    Write-Host "Round $i done. Processed $qCount questions in $mdFile" -ForegroundColor Green
}

Write-Host "`n===== Summary =====" -ForegroundColor Cyan
Write-Host "Processed $processed batches across $i rounds."

# 4. 最终导入
if (-not $SkipImport -and $processed -gt 0) {
    Write-Host "`nRunning final SQLite import with --reset..." -ForegroundColor Yellow
    & $Python $ImportScript --input output/vision-md --exam SC-100 --db $Db --reset

    if ($LASTEXITCODE -eq 0) {
        # 同步到 AppData
        $appDataDb = "$env:LOCALAPPDATA\TauriExam\question-banks\SC-100.sqlite"
        if (Test-Path (Split-Path $appDataDb)) {
            Copy-Item $Db $appDataDb -Force
            Write-Host "Synced to $appDataDb" -ForegroundColor Green
        }
        Write-Host "Import complete." -ForegroundColor Green
    } else {
        Write-Host "Import failed!" -ForegroundColor Red
    }
} elseif ($processed -eq 0) {
    Write-Host "No batches processed, skipping import." -ForegroundColor Yellow
} else {
    Write-Host "SkipImport flag set, skipping import." -ForegroundColor Yellow
}
