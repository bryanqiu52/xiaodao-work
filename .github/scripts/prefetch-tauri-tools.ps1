# Seed tauri-bundler's NSIS toolset before `tauri build`.
#
# The bundler downloads nsis-3.11.zip and nsis_tauri_utils.dll with a single
# unretried GET, after the whole Rust compile. One github.com 504 kills the
# release. If its tools directory already holds every required file with a
# matching hash, the bundler skips the download entirely.
#
# Constants below come from the CLI binary actually used by this repo
# (@tauri-apps/cli 2.11.5). Newer Tauri sources use nsis-3.13 - do not copy them.
param(
  [string] $ToolsDir = (Join-Path $env:LOCALAPPDATA 'tauri'),
  [int] $Attempts = 6
)

$ErrorActionPreference = 'Stop'

$NsisUrl  = 'https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.11/nsis-3.11.zip'
$NsisSha1 = 'EF7FF767E5CBD9EDD22ADD3A32C9B8F4500BB10D'
$UtilsUrl  = 'https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll'
$UtilsSha1 = '75197FEE3C6A814FE035788D1C34EAD39349B860'
$ZipRoot  = 'nsis-3.11'
$DllRel   = 'Plugins\x86-unicode\additional\nsis_tauri_utils.dll'

$Required = @(
  'makensis.exe',
  'Bin\makensis.exe',
  'Stubs\lzma-x86-unicode',
  'Stubs\lzma_solid-x86-unicode',
  $DllRel,
  'Include\MUI2.nsh',
  'Include\FileFunc.nsh',
  'Include\x64.nsh',
  'Include\nsDialogs.nsh',
  'Include\WinMessages.nsh',
  'Include\Win\COM.nsh',
  'Include\Win\Propkey.nsh',
  'Include\Win\RestartManager.nsh'
)

function Get-Sha1([string] $Path) {
  return (Get-FileHash -LiteralPath $Path -Algorithm SHA1).Hash.ToUpperInvariant()
}

function Save-Verified([string] $Uri, [string] $OutFile, [string] $ExpectedSha1) {
  for ($i = 1; $i -le $Attempts; $i++) {
    try {
      Invoke-WebRequest -Uri $Uri -OutFile $OutFile -TimeoutSec 120 -UseBasicParsing
      $actual = Get-Sha1 $OutFile
      if ($actual -ne $ExpectedSha1) { throw "SHA1 mismatch: $actual" }
      return $true
    } catch {
      Remove-Item -LiteralPath $OutFile -Force -ErrorAction SilentlyContinue
      Write-Host "::warning::Download attempt $i/$Attempts failed: $($_.Exception.Message)"
      if ($i -eq $Attempts) { return $false }
      Start-Sleep -Seconds ([int][math]::Min(60, 5 * [math]::Pow(2, $i - 1)))
    }
  }
  return $false
}

$NsisDir = Join-Path $ToolsDir 'NSIS'
$missing = @($Required | Where-Object { -not (Test-Path -LiteralPath (Join-Path $NsisDir $_)) })
if ((Test-Path -LiteralPath $NsisDir) -and $missing.Count -eq 0) {
  if ((Get-Sha1 (Join-Path $NsisDir $DllRel)) -eq $UtilsSha1) {
    Write-Host 'NSIS toolset is complete and verified; skipping prefetch.'
    exit 0
  }
}

$tmp = Join-Path $env:TEMP 'tauri-nsis-prefetch'
New-Item -ItemType Directory -Path $tmp -Force | Out-Null
$zipPath = Join-Path $tmp 'nsis-3.11.zip'
$dllTmp  = Join-Path $tmp 'nsis_tauri_utils.dll'

try {
  if (-not (Save-Verified $NsisUrl $zipPath $NsisSha1)) { throw 'nsis-3.11.zip' }
  if (-not (Save-Verified $UtilsUrl $dllTmp $UtilsSha1)) { throw 'nsis_tauri_utils.dll' }

  # Stage first, swap into place only when complete. A half-written NSIS dir
  # makes the bundler redownload, which is exactly what we are avoiding.
  $stage = Join-Path $tmp 'stage'
  if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
  New-Item -ItemType Directory -Path $stage | Out-Null
  Expand-Archive -LiteralPath $zipPath -DestinationPath $stage -Force

  $extracted = Join-Path $stage $ZipRoot
  if (-not (Test-Path -LiteralPath $extracted)) { throw "Zip root '$ZipRoot' not found" }
  Rename-Item -LiteralPath $extracted -NewName 'NSIS'
  $target = Join-Path $stage 'NSIS'

  $dllDir = Split-Path -Parent (Join-Path $target $DllRel)
  New-Item -ItemType Directory -Path $dllDir -Force | Out-Null
  Copy-Item -LiteralPath $dllTmp -Destination (Join-Path $target $DllRel) -Force

  $stillMissing = @($Required | Where-Object { -not (Test-Path -LiteralPath (Join-Path $target $_)) })
  if ($stillMissing.Count -gt 0) { throw "Incomplete toolset after extract: $($stillMissing -join ', ')" }

  if (Test-Path -LiteralPath $NsisDir) { Remove-Item -LiteralPath $NsisDir -Recurse -Force }
  New-Item -ItemType Directory -Path $ToolsDir -Force | Out-Null
  Move-Item -LiteralPath $target -Destination $NsisDir

  Write-Host "NSIS toolset ready at $NsisDir"
} catch {
  # Non-fatal on purpose: if prefetch fails, leave the bundler's own download
  # path untouched. That is no worse than the behaviour before this step existed.
  Write-Host "::warning::Prefetch failed: $($_.Exception.Message). tauri-bundler will download on its own."
} finally {
  Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
exit 0
