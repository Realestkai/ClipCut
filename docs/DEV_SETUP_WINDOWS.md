# Building ClipCut on Windows

How to build and run the editor on a Windows PC, so a change can be tried
in minutes instead of waiting for GitHub Actions.

## What you need installed

- **Rust**, through [rustup](https://rustup.rs). The toolchain the project
  pins (`src/rust-toolchain.toml`) installs itself on the first build.
- **Visual Studio 2022 or newer** with the *Desktop development with C++*
  workload: the MSVC compiler, a Windows SDK, and the cmake and Ninja that
  come with it.
- **Git for Windows** and **Python 3**.

Nothing else is installed. FFmpeg, ONNX Runtime and libclang are downloaded
into the repository's `vendor\` folder, which git ignores.

## Build and run

In PowerShell, from the repository's folder:

```powershell
. .\scripts\dev-windows.ps1
cd src
cargo dev
```

The dot and the space at the start of the first line matter: the script
sets variables for the PowerShell window it runs in. Run it once in every
new window; `cargo dev` can then be run as often as you like.

`cargo dev` is an alias (`src/.cargo/config.toml`) for
`cargo run --bin clipcut --profile quick`. On the PC this was written on
(an i5-6500 with 16 GB) the first build took about 30 minutes, and a
rebuild after a change to the window's Rust or `.slint` files about five:
the window's crate is one large generated module. Still an hour less than
waiting for CI.

## What the script does

`scripts/dev-windows.ps1` does for one PC what the Windows steps of
`.github/workflows/build-app.yml` do on CI:

| Needed | Where it comes from | Variable |
| --- | --- | --- |
| FFmpeg 8.1 development build | `ffmpeg-win64` in `scripts/ci/deps.lock`, fetched and checked by `scripts/ci/fetch.sh` into `vendor\ffmpeg` | `FFMPEG_DIR` |
| ONNX Runtime 1.28.0 | `onnxruntime-win-x64` in `deps.lock`, into `vendor\onnxruntime` | `ORT_LIB_LOCATION`, `ORT_PREFER_DYNAMIC_LINK=1` |
| libclang, for bindgen | the `libclang` wheel from PyPI, unpacked into `vendor\libclang` (or an LLVM already installed) | `LIBCLANG_PATH` |
| MSVC and the Windows SDK | found on disk, see below | `INCLUDE`, `LIB`, `PATH`, `RC_PATH` |
| cmake and Ninja, for whisper.cpp | Visual Studio's own copies | `PATH`, `CMAKE_GENERATOR=Ninja` |

It also puts FFmpeg's and ONNX Runtime's DLLs on `PATH`, so the program
that was built starts.

Two differences from CI, both on purpose:

- CI installs LLVM 21 with its installer. The wheel is the same library
  without an installer; if you would rather have LLVM installed, install it
  to `C:\Program Files\LLVM` and the script uses that instead.
- CI builds whisper's CPU kernels as one DLL per processor generation
  (`GGML_BACKEND_DL`). A local build leaves that off: the kernels are built
  into the program, for the processor of the PC that builds it.

## If it does not work

**`linker link.exe not found`, with Visual Studio installed.** The Windows
SDK is where the registry does not say it is. On the PC this was written
on, the registry named `C:\Program Files (x86)\Windows Kits\10`, which had
no `Lib` or `Include` folder, and the real SDK was on `D:`. The script does
not trust the registry: it looks on every drive for a Windows Kits folder
that has its libraries, and sets `INCLUDE` and `LIB` itself. Run cargo from
a window the script was dot-sourced in, not from a plain one.

**`no complete Windows SDK found`.** Open the Visual Studio Installer,
choose Modify, and add a *Windows 11 SDK* under Individual components.

**The download is refused with "does not match deps.lock".** The file
upstream changed or the download was cut short. Delete `vendor\dl` and run
the script again.

**The window does not open, with a missing DLL.** The program was started
from a window the script was not run in. Dot-source the script first.

## Checks before pushing

CI fails on these two, and both run in seconds:

```powershell
cd src; cargo fmt --check; cd ..
python scripts/locales.py --check
```

To check the window compiles without building all of it:
`cargo check -p clipcut` from `src`.
