#Requires -Version 7
# Fetches only hash-pinned native libraries using the upstream lockfile.
param()
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$editor = Join-Path $root 'editor'
$bash = Join-Path $env:ProgramFiles 'Git\bin\bash.exe'
if (-not (Test-Path $bash)) { throw 'Install Git for Windows first.' }
function Fetch([string]$name,[string]$destination) {
    & $bash scripts/ci/fetch.sh $name $destination
    if ($LASTEXITCODE -ne 0) { throw "Pinned download failed: $name" }
}
Push-Location $editor
try {
    $deps = Join-Path $editor 'cutflow-deps'
    New-Item -ItemType Directory -Force $deps | Out-Null
    $ffDir = Join-Path $deps 'ffmpeg'
    if (-not (Test-Path "$ffDir\bin\ffmpeg.exe")) {
        Fetch 'ffmpeg-win64' 'cutflow-deps/ffmpeg.zip'
        Expand-Archive 'cutflow-deps/ffmpeg.zip' -DestinationPath 'cutflow-deps/ffmpeg-unpack' -Force
        $unpacked = Get-ChildItem 'cutflow-deps/ffmpeg-unpack' -Directory | Select-Object -First 1
        if (-not $unpacked) { throw 'FFmpeg archive is empty.' }
        New-Item -ItemType Directory -Force $ffDir | Out-Null
        Copy-Item "$($unpacked.FullName)\*" $ffDir -Recurse -Force
    }
    $ortDir = Join-Path $deps 'onnxruntime-win-x64-1.28.0'
    if (-not (Test-Path "$ortDir\lib\onnxruntime.lib")) {
        Fetch 'onnxruntime-win-x64' 'cutflow-deps/onnxruntime.zip'
        Expand-Archive 'cutflow-deps/onnxruntime.zip' -DestinationPath $deps -Force
    }
    $version = (Select-String -Path src\Cargo.lock -Pattern 'name = "sherpa-onnx-sys"' -Context 0,1).Context.PostContext[0] -replace '.*"(.*)".*', '$1'
    $stem = "sherpa-onnx-v$version-win-x64-shared-MT-Release-lib"
    $sherpaDir = Join-Path $deps $stem
    $sherpaName = 'sherpa-onnx-win-x64-shared-MT-Release-lib'
    $url = & $bash scripts/ci/fetch.sh --url $sherpaName
    if ($url -notmatch "/v$([regex]::Escape($version))/") { throw 'sherpa version differs from deps.lock.' }
    if (-not (Test-Path "$sherpaDir\lib")) {
        Fetch $sherpaName 'cutflow-deps/sherpa.tar.bz2'
        python -c "import tarfile; t=tarfile.open('cutflow-deps/sherpa.tar.bz2'); t.extractall('cutflow-deps', filter='data')"
        if ($LASTEXITCODE -ne 0) { throw 'sherpa extraction failed. Python 3.12+ is required.' }
    }
    $importLibs = Join-Path $deps 'sherpa-imports'
    New-Item -ItemType Directory -Force $importLibs | Out-Null
    Copy-Item "$sherpaDir\lib\sherpa-onnx-*.lib" $importLibs -Force
    if (-not $env:LIBCLANG_PATH) {
        foreach ($candidate in @('C:\Program Files\LLVM\bin', 'C:\llvm\bin')) {
            if (Test-Path "$candidate\libclang.dll") { $env:LIBCLANG_PATH = $candidate; break }
        }
    }
    if (-not $env:LIBCLANG_PATH -or -not (Test-Path "$env:LIBCLANG_PATH\libclang.dll")) {
        throw 'Install LLVM x64 (libclang.dll), then set LIBCLANG_PATH to its bin directory.'
    }
    $env:FFMPEG_DIR = $ffDir
    $env:ORT_LIB_LOCATION = "$ortDir\lib"
    $env:ORT_PREFER_DYNAMIC_LINK = '1'
    $env:ORT_DIR = $ortDir
    $env:SHERPA_ONNX_LIB_DIR = $importLibs
    $env:SHERPA_DLL_DIR = "$sherpaDir\lib"
    $env:PATH = "$ffDir\bin;$ortDir\lib;$env:LIBCLANG_PATH;$env:PATH"
    foreach ($key in @('GGML_NATIVE','GGML_AVX2','GGML_FMA','GGML_F16C','GGML_BMI2','GGML_AVX512')) {
        [Environment]::SetEnvironmentVariable($key, 'OFF', 'Process')
    }
    Write-Host 'Pinned native libraries are ready.'
} finally { Pop-Location }
