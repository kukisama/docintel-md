param(
    [switch]$NoRun
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Root = $PSScriptRoot
$AppDir = Join-Path $Root 'TauriExam'
$TauriDir = Join-Path $AppDir 'src-tauri'
$ReleaseDir = Join-Path $TauriDir 'target\release'
$ExePath = Join-Path $ReleaseDir 'tauri-exam.exe'
$PdfiumSource = Join-Path $TauriDir 'resources\pdfium.dll'
$PdfiumTarget = Join-Path $ReleaseDir 'pdfium.dll'

if (-not (Test-Path $AppDir)) {
    throw "TauriExam directory not found: $AppDir"
}

if (-not (Test-Path $PdfiumSource)) {
    throw "pdfium.dll not found: $PdfiumSource"
}

Write-Host '==> Closing running TauriExam release process if needed...'
Get-Process -Name 'tauri-exam' -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -eq $ExePath } |
    Stop-Process -Force

Write-Host '==> Building release exe only (no installer bundle; includes frontend build)...'
npm --prefix $AppDir run tauri -- build --no-bundle

Write-Host '==> Ensuring pdfium.dll is beside the release exe...'
New-Item -ItemType Directory -Force -Path $ReleaseDir | Out-Null
Copy-Item -Force $PdfiumSource $PdfiumTarget

if (-not (Test-Path $ExePath)) {
    throw "Release exe was not produced: $ExePath"
}
if (-not (Test-Path $PdfiumTarget)) {
    throw "pdfium.dll was not copied to: $PdfiumTarget"
}

Write-Host "==> Release exe: $ExePath"
Write-Host "==> PDFium DLL:  $PdfiumTarget"

if (-not $NoRun) {
    Write-Host '==> Starting TauriExam...'
    Start-Process -FilePath $ExePath -WorkingDirectory $ReleaseDir
}
