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
            visible_families(db.faces().flat_map(|face| {
                face.families
                    .iter()
                    .map(|(family, _language)| family.clone())
            }))
        })
        .clone()
}

fn visible_families(families: impl IntoIterator<Item = String>) -> Vec<String> {
    families
        .into_iter()
        .filter(|family| !family.starts_with('@'))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_families_sort_deduplicate_and_skip_vertical_names() {
        let families = visible_families([
            "Noto Sans SC".to_string(),
            "@Microsoft YaHei UI".to_string(),
            "Arial".to_string(),
            "Noto Sans SC".to_string(),
        ]);

        assert_eq!(families, ["Arial", "Noto Sans SC"]);
    }
}
