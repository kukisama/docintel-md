<#
.SYNOPSIS
  Resolve a usable Python 3 executable quickly, optionally installing Python silently via winget when missing.

.DESCRIPTION
  This script prints exactly one value to the pipeline on success: the resolved python.exe path.
  Use Write-Host only for human progress so callers can safely capture stdout.
#>
param(
  [string]$PreferredPython = '',
  [switch]$InstallIfMissing
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Test-PythonExe {
  param([string]$Exe)

  if ([string]::IsNullOrWhiteSpace($Exe)) {
    return $null
  }

  try {
    $resolved = $Exe
    if (Test-Path -LiteralPath $Exe) {
      $resolved = (Resolve-Path -LiteralPath $Exe).Path
    } else {
      $cmd = Get-Command $Exe -ErrorAction SilentlyContinue
      if (-not $cmd) { return $null }
      $resolved = $cmd.Source
    }

    $probe = & $resolved -c "import sys; print(sys.executable if sys.version_info.major >= 3 else '')" 2>$null
    if ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($probe)) {
      return ($probe | Select-Object -First 1).Trim()
    }
  } catch {
    return $null
  }

  return $null
}

function Test-PyLauncher {
  $py = Get-Command py -ErrorAction SilentlyContinue
  if (-not $py) { return $null }

  try {
    $probe = & $py.Source -3 -c "import sys; print(sys.executable)" 2>$null
    if ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($probe)) {
      return ($probe | Select-Object -First 1).Trim()
    }
  } catch {
    return $null
  }

  return $null
}

function Resolve-ExistingPython {
  $candidates = @(
    $PreferredPython,
    $env:DOCINTEL_PYTHON,
    $env:PYTHON,
    'python',
    'python3',
    (Join-Path $env:LOCALAPPDATA 'Programs\Python\Python312\python.exe'),
    (Join-Path $env:LOCALAPPDATA 'Programs\Python\Python311\python.exe')
  ) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }

  foreach ($candidate in $candidates) {
    $resolved = Test-PythonExe -Exe $candidate
    if ($resolved) { return $resolved }
  }

  $launcherResolved = Test-PyLauncher
  if ($launcherResolved) { return $launcherResolved }

  return $null
}

$python = Resolve-ExistingPython
if ($python) {
  Write-Output $python
  exit 0
}

if ($InstallIfMissing) {
  $winget = Get-Command winget -ErrorAction SilentlyContinue
  if ($winget) {
    Write-Host 'Python 3 not found. Installing Python 3.12 silently via winget...' -ForegroundColor Yellow
    & $winget.Source install --id Python.Python.3.12 --exact --silent --accept-package-agreements --accept-source-agreements
    if ($LASTEXITCODE -ne 0) {
      throw "winget failed to install Python.Python.3.12 with exit code $LASTEXITCODE"
    }

    $python = Resolve-ExistingPython
    if ($python) {
      Write-Output $python
      exit 0
    }

    throw 'Python was installed but could not be resolved. Open a new terminal or set DOCINTEL_PYTHON to python.exe.'
  }
}

throw 'Python 3 was not found. Install Python 3.12 or set DOCINTEL_PYTHON / PYTHON to python.exe.'
