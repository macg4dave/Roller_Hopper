//! Language selection. All UI text lives in `translations/<lang>/LC_MESSAGES/*.po`; the `.slint`
//! files only hold message keys, so English is a translation like any other.

use slint::ComponentHandle;

use crate::ui::{AppWindow, Labels};

/// Slint picks the bundled language that matches the system locale. When none matches it would
/// show the raw message keys, so fall back to English, which is always bundled.
pub(crate) fn select_language(ui: &AppWindow) {
    if !ui.global::<Labels<'_>>().invoke_has_language() {
        // An error only means no translations were bundled at all. build.rs always bundles
        // them, and the tests below check that English covers every key, so it cannot occur.
        slint::select_bundled_translation("en").ok();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    const ENGLISH: &str = "translations/en/LC_MESSAGES/network-mapper.po";

    /// Every message key (and plural key) passed to `@tr(...)` in the UI.
    fn keys_in_ui() -> BTreeSet<String> {
        let mut keys = BTreeSet::new();
        for dir in ["ui", "ui/components"] {
            for entry in fs::read_dir(Path::new(dir)).unwrap().flatten() {
                if entry.path().extension().is_some_and(|e| e == "slint") {
                    collect_keys(&fs::read_to_string(entry.path()).unwrap(), &mut keys);
                }
            }
        }
        keys
    }

    fn collect_keys(source: &str, keys: &mut BTreeSet<String>) {
        for call in source.split("@tr(\"").skip(1) {
            let (key, rest) = call.split_once('"').unwrap();
            keys.insert(key.to_owned());
            if let Some(plural) = rest.strip_prefix(" | \"") {
                keys.insert(plural.split_once('"').unwrap().0.to_owned());
            }
        }
    }

    /// Every `msgid` and `msgid_plural` in a .po file, except the empty header entry.
    fn keys_in_po(path: &str) -> BTreeSet<String> {
        let po = fs::read_to_string(path).unwrap();
        po.lines()
            .filter_map(|l| {
                l.strip_prefix("msgid \"")
                    .or_else(|| l.strip_prefix("msgid_plural \""))
            })
            .filter_map(|rest| rest.strip_suffix('"'))
            .filter(|key| !key.is_empty())
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn english_has_text_for_every_key_and_no_stale_entries() {
        let ui = keys_in_ui();
        let english = keys_in_po(ENGLISH);
        let missing: Vec<_> = ui.difference(&english).collect();
        let unused: Vec<_> = english.difference(&ui).collect();
        assert!(missing.is_empty(), "keys with no English text: {missing:?}");
        assert!(unused.is_empty(), "English entries no UI uses: {unused:?}");
    }

    #[test]
    fn ui_uses_keys_rather_than_wording() {
        let wording: Vec<_> = keys_in_ui()
            .into_iter()
            .filter(|key| {
                !key.split('.').all(|part| {
                    !part.is_empty() && part.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                })
            })
            .collect();
        assert!(
            wording.is_empty(),
            "@tr() should take a key such as \"header.scan\": {wording:?}"
        );
    }

    #[test]
    fn key_scanner_reads_plural_forms() {
        let mut keys = BTreeSet::new();
        collect_keys(
            r#"x: @tr("a.one" | "a.other" % n); y: @tr("b.c", z);"#,
            &mut keys,
        );
        assert_eq!(
            keys.into_iter().collect::<Vec<_>>(),
            ["a.one", "a.other", "b.c"]
        );
    }
}
