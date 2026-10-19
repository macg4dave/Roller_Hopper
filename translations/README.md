# Translations

All user-visible text lives in this folder, in gettext `.po` files. The `.slint` files under `ui/`
contain no wording, only message keys such as `@tr("header.scan")`. The Rust code sends data
(names, numbers, enums), never wording. English is a translation like any other language.

```text
translations/
  en/LC_MESSAGES/network-mapper.po      English: the reference for every key
  <lang>/LC_MESSAGES/network-mapper.po  one per extra language, e.g. de, fr, pt-BR
```

The files are bundled into the binary at build time. At start-up the app picks the language that
matches the system locale (exact match first, such as `pt-BR`, then the base language, `pt`). If
none matches, it uses English.

## Change some text

Edit the `msgstr` in the language's `.po` file, then run `cargo build`. Don't change the `msgid`
lines: they are the keys the UI looks up.

## Add a language

1. Copy `en/LC_MESSAGES/network-mapper.po` to `<lang>/LC_MESSAGES/network-mapper.po`, e.g.
   `de/LC_MESSAGES/network-mapper.po`.
2. In the header, set `Language:` and the `Plural-Forms:` line for that language. Poedit fills
   these in, or see the GNU gettext manual's plural forms table.
3. Translate every `msgstr`. For plural entries, give one `msgstr[i]` per plural form.
4. Run `cargo build`.

Placeholders must stay in the translated text. Use `{}` or `{0}`, `{1}` for values in order, and
`{n}` for the count in plural entries. The comments in the English file say what each one holds.

## Add or rename UI text (developers)

1. In the `.slint` file, use a key: `@tr("area.name")`. Keys are lower case with dots and hyphens.
2. Add the key and its English text to `en/LC_MESSAGES/network-mapper.po`, and to the other
   languages if you can.
3. Run `cargo test`. It fails if a key used in the UI has no English text, if an English entry is
   no longer used, or if `@tr()` is given wording instead of a key.

A language file that misses a key shows the raw key for that text, so keep every language up to
date with the English file.

## Not translated

Device names, notes, custom roles, manufacturers, models and service names (`SSH`, `HTTP`) are
data, so they are shown as entered.
