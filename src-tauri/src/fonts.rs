//! Installed font-family enumeration for the interface-font setting.
//!
//! `fontdb` scans the font directories of each supported platform directly
//! (no fontconfig/CoreText system dependency). The resulting names are CSS
//! font-family values exactly as the font files report them (localized
//! names included); Chromium matches either form. Vertical-layout families
//! (prefixed `@`) are skipped. Unlike fontconfig, plain aliases such as
//! `sans-serif` never appear here — only real family names.

use std::collections::BTreeSet;
use std::sync::OnceLock;

/// Sorted, de-duplicated family names of the fonts installed for the current
/// user. Enumeration failure yields an empty list; the picker still offers
/// the bundled web font in that case. The scan parses every installed face,
/// so the result is cached for the process lifetime.
pub fn system_font_families() -> Vec<String> {
    static FONTS: OnceLock<Vec<String>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            let mut families = BTreeSet::new();
            for face in db.faces() {
                for (family, _language) in &face.families {
                    if !family.starts_with('@') {
                        families.insert(family.clone());
                    }
                }
            }
            families.into_iter().collect()
        })
        .clone()
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
