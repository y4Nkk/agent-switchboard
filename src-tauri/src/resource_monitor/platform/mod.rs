#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
pub use windows::Reader;
#[cfg(target_os = "windows")]
pub use windows::{image_path, process_images, process_key};
#[cfg(target_os = "macos")]
pub use macos::{image_path, process_images, Reader};
#[cfg(target_os = "linux")]
pub use linux::{image_path, process_images, Reader};
