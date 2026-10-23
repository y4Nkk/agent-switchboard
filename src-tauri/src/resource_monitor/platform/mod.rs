#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
pub use windows::Reader;
#[cfg(target_os = "windows")]
pub use windows::process_key;
#[cfg(target_os = "macos")]
pub use macos::Reader;
#[cfg(target_os = "linux")]
pub use linux::Reader;
