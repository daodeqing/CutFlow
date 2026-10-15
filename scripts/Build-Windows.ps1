#Requires -Version 7
param([switch]$SkipPrepare, [switch]$Installer, [switch]$SkipSmoke)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
function Run-Cargo([string[]]$CargoArgs) {
    & cargo +1.93.1 @CargoArgs
    if ($LASTEXITCODE -ne 0) { throw "Cargo failed: $($CargoArgs -join ' ')" }
}
if (-not [Environment]::Is64BitOperatingSystem) { throw 'Windows x64 is required.' }
if (-not $SkipPrepare) { & "$PSScriptRoot\Prepare-Windows.ps1" }
Push-Location $root
try {
    Run-Cargo -CargoArgs @('test','--locked','--manifest-path','downloader/Cargo.toml')
    Run-Cargo -CargoArgs @('check','--locked','--manifest-path','tests/ui-compile/Cargo.toml')
    Run-Cargo -CargoArgs @('check','--locked','--lib','--manifest-path','tests/native-ui/Cargo.toml')
    python tests/check_callback_wiring.py
    if ($LASTEXITCODE -ne 0) { throw 'UI callback registration check failed.' }
    Push-Location reference/bdl
    try { Run-Cargo -CargoArgs @('build','--locked','--release','-p','bdl-cli','--target','x86_64-pc-windows-msvc') }
    finally { Pop-Location }
    Push-Location editor/src
    try { Run-Cargo -CargoArgs @('build','--locked','--profile','app','-p','concat','--target','x86_64-pc-windows-msvc') }
    finally { Pop-Location }
    $stage = Join-Path $root 'dist\CutFlow-0.2.0-windows-x64'
    if (Test-Path $stage) { throw "Stage already exists: $stage. Move the previous dist folder before rebuilding." }
    New-Item -ItemType Directory -Force "$stage\tools", "$stage\licenses" | Out-Null
    Copy-Item 'editor\src\target\x86_64-pc-windows-msvc\app\concat.exe' "$stage\CutFlow.exe"
    Copy-Item "$env:FFMPEG_DIR\bin\*.dll" $stage
    Copy-Item "$env:SHERPA_DLL_DIR\sherpa-onnx-*.dll" $stage
    Copy-Item "$env:ORT_DIR\lib\*.dll" $stage
    Copy-Item 'reference\bdl\target\x86_64-pc-windows-msvc\release\bdl.exe' "$stage\tools"
    Copy-Item "$env:FFMPEG_DIR\bin\ffmpeg.exe", "$env:FFMPEG_DIR\bin\ffprobe.exe" "$stage\tools"
    # BDL launches tools/ffmpeg.exe: its loader needs DLLs beside that executable.
    Copy-Item "$env:FFMPEG_DIR\bin\*.dll" "$stage\tools"
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    $vs = & $vswhere -latest -products * -property installationPath
    $crt = Get-ChildItem "$vs\VC\Redist\MSVC\*\x64\Microsoft.VC14*.CRT" -Directory | Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $crt) { throw 'Visual C++ runtime not found.' }
    Copy-Item "$($crt.FullName)\*.dll" $stage
    Copy-Item "$($crt.FullName)\*.dll" "$stage\tools"
    Copy-Item 'editor\LICENSE', 'editor\LICENSE-EXCEPTIONS.md', 'editor\THIRD_PARTY_NOTICES.md' "$stage\licenses"
    Copy-Item 'reference\bdl\LICENSE' "$stage\licenses\BDL-MIT.txt"
    Copy-Item 'THIRD_PARTY_NOTICES.md', 'upstreams.lock.json' "$stage\licenses"
    Copy-Item 'README.md' "$stage\使用说明.md"
    Copy-Item 'docs\验收与已知限制.md' "$stage\验收与已知限制.md"
    Copy-Item 'docs\SOURCE_OFFER.md' "$stage\SOURCE_OFFER.md"
    foreach ($file in Get-ChildItem $env:FFMPEG_DIR -File -Recurse | Where-Object { $_.Name -match 'LICENSE|COPYING' }) {
        Copy-Item $file.FullName "$stage\licenses\FFmpeg-$($file.Name)" -Force
    }
    if (-not $SkipSmoke) { & "$PSScriptRoot\Test-Windows.ps1" -Bundle $stage }
    $zip = "$stage.zip"
    Compress-Archive -Path $stage -DestinationPath $zip
    (Get-FileHash $zip -Algorithm SHA256).Hash + '  ' + (Split-Path $zip -Leaf) | Set-Content "$zip.sha256"
    if ($Installer) {
        $iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
        if (-not (Test-Path $iscc)) { throw 'Install Inno Setup 6 for the optional installer.' }
        & $iscc "/DStage=$stage" "/DOutput=$root\dist" "$PSScriptRoot\CutFlow.iss"
        if ($LASTEXITCODE -ne 0) { throw 'Installer build failed.' }
    }
    Write-Host "Built $zip"
} finally { Pop-Location }
