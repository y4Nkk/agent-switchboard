//! Installed font-family enumeration for the interface-font setting.
//!
//! GDI `EnumFontFamiliesExW` with the default charset reports one entry per
//! family per supported charset, so families are collected into a set. The
//! resulting names are CSS font-family values exactly as Windows reports
//! them (localized on a Chinese system, English elsewhere); Chromium matches
//! either form. Vertical-layout families (prefixed `@`) are skipped.

use std::collections::BTreeSet;
use windows_sys::Win32::Foundation::LPARAM;
use windows_sys::Win32::Graphics::Gdi::{
    EnumFontFamiliesExW, GetDC, ReleaseDC, DEFAULT_CHARSET, FONTENUMPROCW, LOGFONTW,
};

unsafe extern "system" fn collect_family(
    log_font: *const LOGFONTW,
    _metrics: *const windows_sys::Win32::Graphics::Gdi::TEXTMETRICW,
    _kind: u32,
    families: LPARAM,
) -> i32 {
    let set = &mut *(families as *mut BTreeSet<String>);
    let face = (*log_font).lfFaceName;
    let length = face
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(face.len());
    let name = String::from_utf16_lossy(&face[..length]);
    if !name.starts_with('@') {
        set.insert(name);
    }
    1
}

/// Sorted, de-duplicated family names of the fonts installed for the current
/// user. Enumeration failure yields an empty list; the picker still offers
/// the bundled web font in that case.
pub fn system_font_families() -> Vec<String> {
    unsafe {
        let dc = GetDC(core::ptr::null_mut());
        if dc.is_null() {
            return Vec::new();
        }
        let mut log_font: LOGFONTW = core::mem::zeroed();
        log_font.lfCharSet = DEFAULT_CHARSET;
        let mut families = BTreeSet::new();
        let callback: FONTENUMPROCW = Some(collect_family);
        EnumFontFamiliesExW(
            dc,
            &log_font,
            callback,
            &mut families as *mut BTreeSet<String> as isize,
            0,
        );
        ReleaseDC(core::ptr::null_mut(), dc);
        families.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_families_are_unique_sorted_and_include_the_shell_font() {
        let families = system_font_families();

        assert!(families.contains(&"Segoe UI".to_string()));
        assert!(families.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(!families.iter().any(|name| name.starts_with('@')));
    }
}
