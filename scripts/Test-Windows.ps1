#Requires -Version 7
param([Parameter(Mandatory)][string]$Bundle)
$ErrorActionPreference = 'Stop'
$bundle = (Resolve-Path $Bundle).Path
$python = (Get-Command python -ErrorAction Stop).Source
$oldPath=$env:PATH
function Expect([string]$File,[string[]]$Arguments,[int]$Seconds=60) {
    $p=Start-Process -FilePath $File -ArgumentList $Arguments -PassThru -WindowStyle Hidden
    if (-not $p.WaitForExit($Seconds*1000)) { $p.Kill($true); throw "Timed out: $File" }
    if ($p.ExitCode -ne 0) { throw "$File failed with exit code $($p.ExitCode)" }
}
try {
    $env:PATH="$env:SystemRoot\System32;$env:SystemRoot"
    Expect "$bundle\CutFlow.exe" @('--version')
    Expect "$bundle\CutFlow.exe" @('--smoke') 120
    & $python "$PSScriptRoot\..\tests\check_windows_startup.py" --exe "$bundle\CutFlow.exe" --state-dir "$bundle\..\.startup-check"
    if ($LASTEXITCODE -ne 0) { throw 'Visible GUI startup check failed.' }
    Expect "$bundle\tools\bdl.exe" @('--version')
    Expect "$bundle\tools\ffmpeg.exe" @('-version')
    Expect "$bundle\tools\ffprobe.exe" @('-version')
    Write-Host 'Packaged application and tools start with only the Windows system PATH.'
} finally { $env:PATH=$oldPath }
