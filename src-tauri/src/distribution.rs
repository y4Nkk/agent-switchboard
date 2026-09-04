use serde::Serialize;

const ERROR_INSUFFICIENT_BUFFER: i32 = 122;

/// The installation channel owns application updates: direct installs use the
/// signed GitHub updater, while an MSIX installation is updated by the Store.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) enum UpdateChannel {
    #[serde(rename = "github")]
    GitHub,
    #[serde(rename = "microsoftStore")]
    MicrosoftStore,
}

#[tauri::command]
pub(crate) fn update_channel() -> UpdateChannel {
    update_channel_from_package_status(package_status())
}

fn update_channel_from_package_status(status: i32) -> UpdateChannel {
    if status == ERROR_INSUFFICIENT_BUFFER {
        UpdateChannel::MicrosoftStore
    } else {
        UpdateChannel::GitHub
    }
}

#[cfg(windows)]
fn package_status() -> i32 {
    unsafe extern "system" {
        fn GetCurrentPackageFullName(
            package_full_name_length: *mut u32,
            package_full_name: *mut u16,
        ) -> i32;
    }

    let mut length = 0;
    // Passing no buffer only distinguishes an installed MSIX package
    // (insufficient-buffer) from an unpackaged executable.
    unsafe { GetCurrentPackageFullName(&mut length, std::ptr::null_mut()) }
}

#[cfg(not(windows))]
fn package_status() -> i32 {
    0
}

#[cfg(test)]
mod tests {
    use super::{update_channel_from_package_status, UpdateChannel, ERROR_INSUFFICIENT_BUFFER};

    #[test]
    fn only_a_packaged_process_uses_the_store_update_channel() {
        assert_eq!(
            update_channel_from_package_status(ERROR_INSUFFICIENT_BUFFER),
            UpdateChannel::MicrosoftStore
        );
        assert_eq!(update_channel_from_package_status(0), UpdateChannel::GitHub);
        assert_eq!(update_channel_from_package_status(15_700), UpdateChannel::GitHub);
    }
}
