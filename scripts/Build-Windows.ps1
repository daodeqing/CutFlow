#Requires -Version 7
param([switch]$SkipPrepare, [switch]$Installer, [switch]$SkipSmoke)
$ErrorActionPreference = 'Stop'
$arguments = @('-B', '-X', 'utf8', "$PSScriptRoot\build_windows.py")
if ($Installer) { $arguments += '--installer' }
if ($SkipSmoke) { $arguments += '--skip-smoke' }
& python @arguments
if ($LASTEXITCODE -ne 0) { throw 'CutFlow build failed.' }
