//! ClipCut's updater: the small dark window that puts a downloaded update in
//! place on Windows and opens the new version, so an update from
//! Settings › Version never shows the setup wizard.
//!
//! The app downloads the new `setup.exe`, checks it against the release's
//! digest, copies this program out of its install folder and starts it:
//!
//! ```text
//! clipcut-updater --setup <setup.exe> --wait <pid> --app <clipcut.exe> --version 0.2.13
//! ```
//!
//! and quits. This waits for the app to be gone, runs the setup without its
//! wizard (`/VERYSILENT`), and starts the app again. Should the quiet
//! install fail, it opens the setup's own wizard instead, so an update is
//! never lost. `--demo` shows the window for a few seconds without
//! installing anything, for a look at it.
//!
//! macOS needs none of this: the app swaps its own bundle and restarts.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod window;

fn main() {
    #[cfg(windows)]
    std::process::exit(window::run(std::env::args_os().skip(1).collect()));
    #[cfg(not(windows))]
    eprintln!("clipcut-updater is for Windows; on macOS ClipCut updates itself in place.");
}
