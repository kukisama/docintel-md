<#
.SYNOPSIS
  DB 优先批量为 hotspot/drag_drop 题补充 Interaction Options/Targets 结构化数据。
  每轮从 SQLite 精确选择缺失题号，再查看对应 MD，调用 Copilot CLI 原地补表，随后立即增量导入并验证数据库。

.PARAMETER Rounds
  最大处理轮次（默认 30，超过自动停止）。

.PARAMETER BatchSize
  每轮最多处理多少道题（默认 8）。

.PARAMETER Questions
  指定题号列表，例如 214,229,243,257。指定后只处理这些题中仍缺交互数据的题。

.PARAMETER Type
  仅处理指定题型：all、hotspot、drag_drop。

.PARAMETER Priority
  自动模式下的优先级：drag_drop、hotspot、question。

.PARAMETER Model
  Copilot CLI 使用的模型（默认 gpt-5.5）。

.PARAMETER Effort
  Copilot CLI effort 级别（默认 medium）。

.PARAMETER SkipImport
  跳过最终导入步骤（调试用）。

.PARAMETER DryRun
  仅预览 DB 选题和 MD 诊断，不调用 Copilot、不导入数据库。

.EXAMPLE
  .\batch-add-interactions.ps1 -Rounds 30 -Model gpt-5.5 -Effort medium

.EXAMPLE
  .\batch-add-interactions.ps1 -Questions 214,229,243,257 -BatchSize 8 -Model gpt-5.5 -Effort medium

.EXAMPLE
  .\batch-add-interactions.ps1 -Type drag_drop -Priority drag_drop -BatchSize 8 -Rounds 2
#>
param(
    [int]$Rounds = 30,
  [int]$BatchSize = 8,
  [string]$Questions = '',
  [ValidateSet('all', 'hotspot', 'drag_drop')]
  [string]$Type = 'all',
  [ValidateSet('drag_drop', 'hotspot', 'question')]
  [string]$Priority = 'drag_drop',
    [string]$Model = 'gpt-5.5',
    [string]$Effort = 'medium',
    [switch]$SkipImport,
    [switch]$DryRun,
    [string]$Python = '',
    [switch]$NoInstallPython,
  [string]$Db = 'question-banks/SC-100.sqlite',
  [string]$Exam = 'SC-100',
  [string]$InputRoot = 'output/vision-md'
)

$ErrorActionPreference = 'Stop'

$Root = $PSScriptRoot
$Python = & (Join-Path $Root 'scripts/Resolve-Python.ps1') -PreferredPython $Python -InstallIfMissing:(-not $NoInstallPython)
$DetectScript = Join-Path $Root '.github/skills/sc100-vision-md/scripts/detect_missing_interactions.py'
$ImportScript = Join-Path $Root '.github/skills/sc100-vision-md/scripts/import_question_md_to_sqlite.py'
if (-not [System.IO.Path]::IsPathRooted($Db)) { $Db = Join-Path $Root $Db }
if (-not [System.IO.Path]::IsPathRooted($InputRoot)) { $InputRoot = Join-Path $Root $InputRoot }

function Convert-ToQuestionSet {
  param([string]$Value)

  $set = @{}
  if ([string]::IsNullOrWhiteSpace($Value)) {
    return $set
  }

  foreach ($item in ($Value -split '[,;\s]+' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })) {
    $number = 0
    if (-not [int]::TryParse($item, [ref]$number)) {
      throw "Invalid question number: $item"
    }
    $set[$number] = $true
  }
  return $set
}

function Get-MissingState {
  $json = & $Python $DetectScript --db $Db
  if ($LASTEXITCODE -ne 0) {
    throw "Detection failed with exit code $LASTEXITCODE"
  }
  return ($json | ConvertFrom-Json)
}

function Get-MissingQuestions {
  param(
    [object]$State,
    [hashtable]$QuestionSet,
    [hashtable]$Attempted
  )

  $items = @()
  foreach ($batch in $State.batches) {
    foreach ($q in $batch.questions) {
      $number = [int]$q.number
      if ($QuestionSet.Count -gt 0 -and -not $QuestionSet.ContainsKey($number)) {
        continue
      }
      if ($Type -ne 'all' -and $q.type -ne $Type) {
        continue
      }
      if ($QuestionSet.Count -eq 0 -and $Attempted.ContainsKey($number)) {
        continue
      }

      $priorityRank = switch ($Priority) {
        'drag_drop' { if ($q.type -eq 'drag_drop') { 0 } else { 1 } }
        'hotspot' { if ($q.type -eq 'hotspot') { 0 } else { 1 } }
        default { 0 }
      }

      $items += [pscustomobject]@{
        id = $q.id
        number = $number
        type = [string]$q.type
        source_pages = [string]$q.source_pages
        md_file = [string]$batch.md_file
        pdf_file = [string]$batch.pdf_file
        priority_rank = $priorityRank
      }
    }
  }

  return @($items | Sort-Object priority_rank, number | Select-Object -First $BatchSize)
}

function Get-MdDiagnosis {
  param([object[]]$Selected)

  $diagnostics = @()
  foreach ($q in $Selected) {
    $hasOptions = $false
    $hasTargets = $false
    $hasAnswerArea = $false
    $hasSourceAnswer = $false
    $status = 'md_file_missing'

    if (Test-Path $q.md_file) {
      $text = Get-Content -Path $q.md_file -Raw -Encoding UTF8
      $pattern = "(?ms)^##\s+Question\s+$($q.number)\s*$.*?(?=^##\s+Question\s+\d+\s*$|\z)"
      $match = [regex]::Match($text, $pattern)
      if ($match.Success) {
        $block = $match.Value
        $hasOptions = $block -match '(?m)^###\s+Interaction Options\s*$'
        $hasTargets = $block -match '(?m)^###\s+Interaction Targets\s*$'
        $hasAnswerArea = $block -match '(?m)^###\s+Answer Area\s*$'
        $hasSourceAnswer = $block -match '(?m)^###\s+Source Answer\s*$'
        if ($hasOptions -and $hasTargets) {
          $status = 'md_has_interactions_db_missing'
        } else {
          $status = 'md_missing_interactions'
        }
      } else {
        $status = 'question_block_missing'
      }
    }

    $diagnostics += [pscustomobject]@{
      number = $q.number
      md_file = $q.md_file
      has_interaction_options = $hasOptions
      has_interaction_targets = $hasTargets
      has_answer_area = $hasAnswerArea
      has_source_answer = $hasSourceAnswer
      status = $status
    }
  }

  return $diagnostics
}

function Import-MdFiles {
  param(
    [string[]]$Files,
    [switch]$Reset
  )

  $uniqueFiles = @($Files | Sort-Object -Unique)
  if ($uniqueFiles.Count -eq 0) {
    return
  }

  if ($Reset) {
    Write-Host "Running full SQLite import with --reset..." -ForegroundColor Yellow
    & $Python $ImportScript --input $InputRoot --exam $Exam --db $Db --reset
    if ($LASTEXITCODE -ne 0) {
      throw "Full import failed with exit code $LASTEXITCODE"
    }
    return
  }

  foreach ($file in $uniqueFiles) {
    if (-not (Test-Path $file)) {
      throw "MD file not found: $file"
    }
    $inputDir = Split-Path -Path $file -Parent
    $pattern = Split-Path -Path $file -Leaf
    Write-Host "Importing updated MD: $file" -ForegroundColor DarkYellow
    & $Python $ImportScript --input $inputDir --pattern $pattern --exam $Exam --db $Db
    if ($LASTEXITCODE -ne 0) {
      throw "Import failed for $file with exit code $LASTEXITCODE"
    }
  }
}

function Test-DbInteractions {
  param([int[]]$Numbers)

  if ($Numbers.Count -eq 0) {
    return @()
  }

  $numsArg = ($Numbers -join ',')
  $code = @'
import json
import sqlite3
import sys

db = sys.argv[1]
numbers = [int(x) for x in sys.argv[2].split(',') if x]
conn = sqlite3.connect(db)
conn.row_factory = sqlite3.Row
placeholders = ','.join('?' for _ in numbers)
rows = conn.execute(f'''
  SELECT q.source_question_number AS number,
       q.id AS question_id,
       q.question_type AS type,
       COUNT(DISTINCT io.id) AS option_count,
       COUNT(DISTINCT it.id) AS target_count
  FROM questions q
  LEFT JOIN interaction_options io ON io.question_id = q.id
  LEFT JOIN interaction_targets it ON it.question_id = q.id
  WHERE q.source_question_number IN ({placeholders})
  GROUP BY q.id
  ORDER BY q.source_question_number
''', numbers).fetchall()
conn.close()
print(json.dumps([dict(row) for row in rows], ensure_ascii=False))
'@

  $json = $code | & $Python - $Db $numsArg
  if ($LASTEXITCODE -ne 0) {
    throw "DB validation failed with exit code $LASTEXITCODE"
  }
  return @($json | ConvertFrom-Json)
}

function Sync-DbCopies {
  $targets = @(
    (Join-Path $env:LOCALAPPDATA 'TauriExam/question-banks/SC-100.sqlite'),
    (Join-Path $Root 'TauriExam/question-banks/SC-100.sqlite')
  )

  foreach ($target in $targets) {
    $dir = Split-Path $target -Parent
    if (Test-Path $dir) {
      Copy-Item $Db $target -Force
      Write-Host "Synced to $target" -ForegroundColor Green
    }
  }
}

$questionSet = Convert-ToQuestionSet -Value $Questions
$attempted = @{}
$processed = 0
$validated = 0
$importOnly = 0

for ($i = 1; $i -le $Rounds; $i++) {
    Write-Host "`n===== Round $i / $Rounds =====" -ForegroundColor Cyan

  # 1. DB 优先：检测当前 SQLite 中仍缺 Interaction 的题目。
  $state = Get-MissingState

    if ($state.done) {
        Write-Host "All done: $($state.done_reason)" -ForegroundColor Green
        break
    }

    Write-Host "Remaining: $($state.total_missing) questions in $($state.total_batches) batches" -ForegroundColor Yellow

  $selected = @(Get-MissingQuestions -State $state -QuestionSet $questionSet -Attempted $attempted)
  if ($selected.Count -eq 0) {
    if ($questionSet.Count -gt 0) {
      Write-Host "No selected question remains missing in DB." -ForegroundColor Green
    } else {
      Write-Host "No unattempted missing question matched Type=$Type Priority=$Priority." -ForegroundColor Yellow
    }
    break
  }

  $qDescs = foreach ($q in $selected) {
    "Q$($q.number)($($q.type), pages $($q.source_pages), file $($q.md_file))"
    }
    $qList = $qDescs -join ', '
  $qCount = $selected.Count
  $files = @($selected | ForEach-Object { $_.md_file } | Sort-Object -Unique)

  Write-Host "Selected $qCount question(s): $qList" -ForegroundColor White

  # 2. MD 后看：只针对 DB 缺失题检查 MD 是否已经有结构段。
  $diagnostics = @(Get-MdDiagnosis -Selected $selected)
  foreach ($diag in $diagnostics) {
    Write-Host ("MD check Q{0}: {1} (options={2}, targets={3})" -f $diag.number, $diag.status, $diag.has_interaction_options, $diag.has_interaction_targets) -ForegroundColor DarkCyan
  }

  if ($DryRun) {
    Write-Host "DryRun set: preview complete, skipping Copilot call and import." -ForegroundColor Yellow
    break
  }

  $allAlreadyInMd = ($diagnostics | Where-Object { -not ($_.has_interaction_options -and $_.has_interaction_targets) }).Count -eq 0
  if ($allAlreadyInMd -and -not $SkipImport) {
    Write-Host "All selected questions already have Interaction sections in MD. Importing without AI call..." -ForegroundColor Yellow
    Import-MdFiles -Files $files
    $result = @(Test-DbInteractions -Numbers @($selected | ForEach-Object { [int]$_.number }))
    $failed = @($result | Where-Object { $_.option_count -eq 0 -or $_.target_count -eq 0 })
    if ($failed.Count -eq 0) {
      $importOnly += $qCount
      $validated += $qCount
      Write-Host "Import-only validation passed for $qCount question(s)." -ForegroundColor Green
      Sync-DbCopies
      continue
    }
    Write-Host "MD import did not populate all selected questions; invoking AI for failed/missing structures." -ForegroundColor Yellow
  }

  # 3. 构造 prompt。AI 只负责读取视觉源并原地补 MD；脚本随后负责导入和 DB 验证。
  $targetFiles = ($files | ForEach-Object { "- $_" }) -join "`n"
  $targetQuestions = ($selected | ForEach-Object { "- Q$($_.number): type=$($_.type), source_pages=$($_.source_pages), md_file=$($_.md_file)" }) -join "`n"
    $prompt = @"
用 sc100-vision-md skill 的 Regenerate Mode 为以下题目补充结构化交互数据。

这是 DB 优先的定向修复任务。数据库显示这些题缺少 Interaction Options / Interaction Targets；请只处理下列题号，不要扩展到其他题。

目标 MD 文件:
$targetFiles

需要处理的题目 ($qCount 道):
$targetQuestions

具体任务:
1. 用 render_pdf_pages.py 渲染每道题 source_pages 对应的 PDF 页
2. 视觉读取每道题的 HOTSPOT 下拉框或 DRAG DROP 候选区
3. 为每道题补充 Interaction Options 和 Interaction Targets 表格
4. 原地编辑对应的目标 MD 文件，在每道题的 Answer Area 之后、Source Answer 之前插入结构化表格
5. 不要修改题目文字、答案、解析等其他内容
6. 格式参考 output/vision-md/sc-100-structured-overrides/sc-100-questions-001-010-023-032-042.md 中已有的 Interaction Options/Targets

注意:
- hotspot 下拉题: 每行有独立的下拉候选项，用不同 Group 区分
- drag_drop 题: 所有候选项共享一个池子
- 必须保留干扰项 (Distractor = Yes)
- Option ID 用 opt-xxx 格式，Target ID 用 target-N 格式
- 每道题必须同时生成 Interaction Options 和 Interaction Targets，不能只生成 Answer Area 或 Source Answer
- Correct Option ID 必须引用已存在的 Option ID，不能引用不存在的 ID
- 不要创建新 MD 文件，只原地编辑上述目标 MD 文件
"@

  # 4. 调用 Copilot CLI
    copilot --model $Model --effort $Effort -p $prompt --allow-all

    if ($LASTEXITCODE -ne 0) {
        Write-Host "Copilot CLI failed with exit code $LASTEXITCODE" -ForegroundColor Red
        break
    }

  foreach ($q in $selected) {
    $attempted[[int]$q.number] = $true
  }

    $processed++

  # 5. 逐轮导入并验证 DB，而不是等所有轮次结束才统一 reset。
  if ($SkipImport) {
    Write-Host "SkipImport flag set, skipping per-round import and DB validation." -ForegroundColor Yellow
    continue
  }

  Import-MdFiles -Files $files

  $validation = @(Test-DbInteractions -Numbers @($selected | ForEach-Object { [int]$_.number }))
  $failedValidation = @($validation | Where-Object { $_.option_count -eq 0 -or $_.target_count -eq 0 })
  foreach ($row in $validation) {
    Write-Host ("DB check Q{0}: options={1}, targets={2}" -f $row.number, $row.option_count, $row.target_count) -ForegroundColor DarkCyan
  }

  if ($failedValidation.Count -gt 0) {
    $failedNumbers = ($failedValidation | ForEach-Object { $_.number }) -join ', '
    Write-Host "Round $i finished, but DB validation still failed for: $failedNumbers" -ForegroundColor Red
  } else {
    $validated += $qCount
    Write-Host "Round $i validation passed for $qCount question(s)." -ForegroundColor Green
    Sync-DbCopies
  }
}

Write-Host "`n===== Summary =====" -ForegroundColor Cyan
Write-Host "AI processed $processed round(s)."
Write-Host "Import-only fixed $importOnly question(s)."
Write-Host "DB-validated $validated question(s)."

if (-not $SkipImport) {
  $finalState = Get-MissingState
  if ($finalState.done) {
    Write-Host "All done: $($finalState.done_reason)" -ForegroundColor Green
  } else {
    Write-Host "Remaining after this run: $($finalState.total_missing) questions in $($finalState.total_batches) batches" -ForegroundColor Yellow
  }
}
