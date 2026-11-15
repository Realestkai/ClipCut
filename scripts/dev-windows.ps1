# Sets a PowerShell session up to build ClipCut on Windows, without
# installing anything: the prebuilt dependencies go into the repository's
# ignored vendor\ folder, and the compiler is found where it already is.
#
#   . scripts\dev-windows.ps1        (dot-sourced: note the leading dot)
#   cd src
#   cargo dev
#
# What CI's "Windows" steps in .github/workflows/build-app.yml do, for one
# machine:
#
# - FFmpeg's development build and ONNX Runtime, the files scripts/ci/deps.lock
#   pins, fetched and checked by scripts/ci/fetch.sh into vendor\ on the
#   first run, and FFMPEG_DIR, ORT_LIB_LOCATION and ORT_PREFER_DYNAMIC_LINK
#   pointed at them. Their DLLs go on PATH so the built program starts.
# - libclang for bindgen. CI runs LLVM's installer; here it is the
#   `libclang` wheel from PyPI unpacked into vendor\libclang, which is the
#   same library with nothing installed. LIBCLANG_PATH points at it. An
#   LLVM already at C:\Program Files\LLVM or C:\llvm is used instead.
# - The MSVC compiler and the Windows SDK, by hand rather than through
#   vcvarsall.bat: that reads the SDK's place from the registry, and on a
#   machine whose SDK sits on another drive than the registry says, it sets
#   paths with no libraries in them (rustc then reports "linker link.exe
#   not found"). This looks for a Windows Kits folder that really has its
#   Lib and Include, on every drive.
# - cmake and Ninja from Visual Studio's own copy, for whisper.cpp. Ninja,
#   not the Visual Studio generator, for the same registry reason.
#
# Needs: Rust (rustup installs the toolchain src/rust-toolchain.toml pins),
# Git for Windows, Python 3, and Visual Studio 2022 or newer with "Desktop
# development with C++".

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$vendor = Join-Path $repo "vendor"
New-Item -ItemType Directory -Force (Join-Path $vendor "dl") | Out-Null

function Fail($message) { throw "dev-windows: $message" }

# -- the prebuilt dependencies --------------------------------------------
# Git's bash by its full path: a bare `bash` from PowerShell can be WSL's.
$bash = "$env:ProgramFiles\Git\bin\bash.exe"
if (-not (Test-Path $bash)) { Fail "Git for Windows is not at $bash" }

function Fetch($name, $zip, $into) {
    $target = Join-Path $vendor $into
    if (Test-Path $target) { return }
    Push-Location $repo
    try {
        & $bash scripts/ci/fetch.sh $name "vendor/dl/$zip"
        if ($LASTEXITCODE -ne 0) { Fail "the $name download failed" }
    } finally { Pop-Location }
    Expand-Archive (Join-Path $vendor "dl\$zip") -DestinationPath $target
}

Fetch "ffmpeg-win64" "ffmpeg-win.zip" "ffmpeg"
Fetch "onnxruntime-win-x64" "onnxruntime.zip" "onnxruntime"
$ffmpeg = (Get-ChildItem (Join-Path $vendor "ffmpeg") -Directory | Select-Object -First 1).FullName
$ort = (Get-ChildItem (Join-Path $vendor "onnxruntime") -Directory | Select-Object -First 1).FullName
if (-not (Test-Path "$ffmpeg\lib\avcodec.lib")) { Fail "$ffmpeg has no lib\avcodec.lib" }
if (-not (Test-Path "$ort\lib\onnxruntime.lib")) { Fail "$ort has no lib\onnxruntime.lib" }

# -- libclang -------------------------------------------------------------
$libclang = @("$env:ProgramFiles\LLVM\bin", "C:\llvm\bin", (Join-Path $vendor "libclang\clang\native")) |
    Where-Object { Test-Path "$_\libclang.dll" } | Select-Object -First 1
if (-not $libclang) {
    python -m pip install --quiet --target (Join-Path $vendor "libclang") libclang
    if ($LASTEXITCODE -ne 0) { Fail "could not fetch the libclang wheel" }
    $libclang = Join-Path $vendor "libclang\clang\native"
    if (-not (Test-Path "$libclang\libclang.dll")) { Fail "the libclang wheel has no libclang.dll" }
}

# -- the compiler ---------------------------------------------------------
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path $vswhere)) { Fail "Visual Studio is not installed (no vswhere.exe)" }
$vs = & $vswhere -latest -products * -property installationPath
$msvc = Get-ChildItem "$vs\VC\Tools\MSVC" -Directory |
    Where-Object { (Test-Path "$($_.FullName)\bin\Hostx64\x64\cl.exe") -and (Test-Path "$($_.FullName)\lib\x64") } |
    Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
if (-not $msvc) { Fail "no MSVC x64 toolset under $vs; add 'Desktop development with C++' in the Visual Studio Installer" }
$msvc = $msvc.FullName

# -- the Windows SDK: a Kits folder that has its libraries ----------------
$roots = @()
$registered = (Get-ItemProperty "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows Kits\Installed Roots" -ErrorAction SilentlyContinue).KitsRoot10
if ($registered) { $roots += $registered.TrimEnd("\") }
foreach ($drive in Get-PSDrive -PSProvider FileSystem) {
    $roots += "$($drive.Root)Program Files (x86)\Windows Kits\10"
}
$sdk = $null
foreach ($root in $roots | Select-Object -Unique) {
    $version = Get-ChildItem "$root\Lib" -Directory -ErrorAction SilentlyContinue |
        Where-Object {
            (Test-Path "$($_.FullName)\um\x64\kernel32.lib") -and
            (Test-Path "$root\Include\$($_.Name)\ucrt") -and
            (Test-Path "$root\bin\$($_.Name)\x64\rc.exe")
        } | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
    if ($version) { $sdk = @{ Root = $root; Version = $version.Name }; break }
}
if (-not $sdk) { Fail "no complete Windows SDK found; add a 'Windows 11 SDK' in the Visual Studio Installer" }
$kit = $sdk.Root
$ver = $sdk.Version

# -- cmake and Ninja ------------------------------------------------------
$tools = @()
if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    $cmake = "$vs\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin"
    if (-not (Test-Path "$cmake\cmake.exe")) { Fail "no cmake on PATH or in Visual Studio" }
    $tools += $cmake
}
if (-not (Get-Command ninja -ErrorAction SilentlyContinue)) {
    $ninja = "$vs\Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja"
    if (-not (Test-Path "$ninja\ninja.exe")) { Fail "no ninja on PATH or in Visual Studio" }
    $tools += $ninja
}

# -- the environment ------------------------------------------------------
$env:INCLUDE = @("$msvc\include") + (@("ucrt", "um", "shared", "winrt", "cppwinrt") | ForEach-Object { "$kit\Include\$ver\$_" }) -join ";"
$env:LIB = "$msvc\lib\x64;$kit\Lib\$ver\ucrt\x64;$kit\Lib\$ver\um\x64"
# Once only, so dot-sourcing twice does not grow PATH.
if ($env:CLIPCUT_DEV_ENV -ne $repo) {
    $env:PATH = (@("$msvc\bin\Hostx64\x64", "$kit\bin\$ver\x64") + $tools + @("$ffmpeg\bin", "$ort\lib", $env:PATH)) -join ";"
    $env:CLIPCUT_DEV_ENV = $repo
}
# What a Visual Studio prompt sets, which the cc and cmake crates read to
# take the compiler from the environment instead of the registry.
$env:VCINSTALLDIR = "$vs\VC\"
$env:VSCMD_ARG_TGT_ARCH = "x64"
$env:VSCMD_ARG_HOST_ARCH = "x64"
$env:WindowsSdkDir = "$kit\"
$env:WindowsSDKVersion = "$ver\"
$env:CMAKE_GENERATOR = "Ninja"
# The resource compiler that embeds the .exe's icon: the winresource crate
# reads the SDK's place from the registry too, unless told.
$env:RC_PATH = "$kit\bin\$ver\x64\rc.exe"

$env:FFMPEG_DIR = $ffmpeg
$env:LIBCLANG_PATH = $libclang
$env:ORT_LIB_LOCATION = "$ort\lib"
$env:ORT_PREFER_DYNAMIC_LINK = "1"

Write-Host "ClipCut build environment:"
Write-Host "  MSVC      $msvc"
Write-Host "  SDK       $kit ($ver)"
Write-Host "  FFmpeg    $ffmpeg"
Write-Host "  ONNX      $ort"
Write-Host "  libclang  $libclang"
Write-Host "Now: cd src; cargo dev"
