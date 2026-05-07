param([int]$Rounds=35,[string]$Model='gpt-5.5',[string]$Effort='medium',[string]$Python='C:/Users/kukisama/AppData/Local/Programs/Python/Python312/python.exe')
$Prompt = '用 sc100-vision-md 继续处理。不要问页码；自动检测下一批。若检测到 done=true，停止本回合不做任何操作。'
for ($i = 1; $i -le $Rounds; $i++) {
  $State = (& $Python .github/skills/sc100-vision-md/scripts/detect_next_batch.py | ConvertFrom-Json)
  if ($State.done) { Write-Host "Done: $($State.done_reason)"; break }
  copilot --model $Model --effort $Effort -p "$Prompt`n本轮建议页范围：$($State.next_page_range -join '-')" --allow-all
  if ($LASTEXITCODE -ne 0) { break }
}
