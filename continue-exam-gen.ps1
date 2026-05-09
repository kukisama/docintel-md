param([int]$Rounds = 20, [string]$Exam = 'AI-900', [string]$Model = 'gpt-5.5', [string]$Effort = 'medium')
$Dir = "output/exam-gen/$Exam"
Write-Host "=== $Exam generate: $Rounds rounds, model=$Model ==="
for ($i = 1; $i -le $Rounds; $i++) {
    $Last = (Get-ChildItem $Dir -Filter 'questions-*.md' -ErrorAction SilentlyContinue | Sort-Object Name | Select-Object -Last 1)
    $Num = if ($Last -and $Last.BaseName -match '(\d+)$') { [int]$Matches[1] } else { 0 }
    Write-Host "--- Round $i/$Rounds (current: $Num) ---"
    $P = "用 exam-question-gen 为 $Exam 继续生成模拟题。已有题目到第 $Num 题，从第 $($Num+1) 题续接。直接生成，不要问问题。"
    copilot --model $Model --effort $Effort -p $P --allow-all
    if ($LASTEXITCODE -ne 0) { Write-Host "Copilot error, stopping."; break }
}
$files = Get-ChildItem "output/exam-gen/$Exam" -Filter 'questions-*.md' | Sort-Object Name
Write-Host "=== Done: $($files.Count) files ==="
$files | Select-Object Name | Format-Table -AutoSize