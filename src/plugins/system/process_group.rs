#[cfg(unix)]
#[path = "process_unix.rs"]
mod platform;
#[cfg(windows)]
#[path = "process_windows.rs"]
mod platform;

pub(super) use platform::ProcessGroup;
