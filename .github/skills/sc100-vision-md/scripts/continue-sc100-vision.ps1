param(
    [string]$Pdf = "SC-100 题目+答案+讨论.pdf",
    [string]$Python = "",
    [int]$PagesPerBatch = 30,
    [switch]$RenderAndExtract,
    [switch]$OpenPrompt,
    [switch]$NoInstallPython
)

$ErrorActionPreference = "Stop"
$OutputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false)
$env:PYTHONIOENCODING = "utf-8"

$SkillRoot = Split-Path -Parent $PSScriptRoot
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$Python = & (Join-Path $RepoRoot 'scripts/Resolve-Python.ps1') -PreferredPython $Python -InstallIfMissing:(-not $NoInstallPython)
$DetectScript = Join-Path $PSScriptRoot "detect_next_batch.py"
$RenderScript = Join-Path $PSScriptRoot "render_pdf_pages.py"
$ExtractScript = Join-Path $PSScriptRoot "extract_page_text.py"

$stateJson = & $Python $DetectScript --pdf $Pdf --pages-per-batch $PagesPerBatch
if ($LASTEXITCODE -ne 0) {
    throw "detect_next_batch.py failed"
}
$state = $stateJson | ConvertFrom-Json

if ($state.done -eq $true) {
    Write-Host "SC-100 vision processing is already complete. No operation performed."
    Write-Host $state.done_reason
    exit 0
}

$from = [int]$state.next_page_range[0]
$to = [int]$state.next_page_range[1]
$pagesOutput = [string]$state.suggested_pages_output
$textOutput = [string]$state.suggested_text_output
$mdOutputDir = [string]$state.suggested_md_output_dir

Write-Host "Next batch detected: pages $from-$to"
Write-Host "Expected next question start: $($state.expected_next_question_start)"
Write-Host "Pages output: $pagesOutput"
Write-Host "Text output: $textOutput"
Write-Host "Markdown output dir: $mdOutputDir"

if ($RenderAndExtract) {
    & $Python $RenderScript --pdf $Pdf --from $from --to $to --output $pagesOutput
    if ($LASTEXITCODE -ne 0) { throw "render_pdf_pages.py failed" }

    & $Python $ExtractScript --pdf $Pdf --from $from --to $to --output $textOutput
    if ($LASTEXITCODE -ne 0) { throw "extract_page_text.py failed" }
}

New-Item -ItemType Directory -Force -Path $mdOutputDir | Out-Null
$promptPath = Join-Path $mdOutputDir "copilot-next-prompt.md"
$carryover = if ($state.previous_carryover) { ($state.previous_carryover -join "`n") } else { "无" }

$prompt = @"
用 sc100-vision-md 继续处理。

请不要要求我提供页码；先使用已有输出自动检测下一批。当前检测结果如下：

- PDF: $($state.pdf)
- PDF 总页数: $($state.pdf_page_count)
- 已完成 Markdown: $($state.latest_markdown)
- 已完成页范围: $($state.latest_page_range -join '-')
- 已完成题号范围: $($state.latest_question_range -join '-')
- 下一批建议页范围: $from-$to
- 预期下一题开始: Question #$($state.expected_next_question_start)
- 图片目录: $pagesOutput
- 文本辅助文件: $textOutput
- Markdown 输出目录: $mdOutputDir

上批 carryover：
$carryover

请基于渲染图片进行视觉读题，PDF 文本层只作为辅助。输出下一组 10 题 Markdown。每题必须包含：

- Source pages
- Topic
- Type
- Status
- Question
- Options 或 Answer Area
- Source Answer
- My Recommended Answer
- 我的判断（中文）
- Reasoning
- Notes

如果检测发现所有页已经处理完，则不要做任何操作，直接停止本回合。
"@

Set-Content -Path $promptPath -Value $prompt -Encoding UTF8
Write-Host "Prompt written: $promptPath"

if ($OpenPrompt) {
    Invoke-Item $promptPath
}

Write-Host "Next step: paste the prompt file content into Copilot Chat/Agent, or feed it to your Copilot CLI if your CLI supports prompt input."
