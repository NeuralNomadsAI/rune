//! Android's system fonts, from `/system/etc/fonts.xml`: the named families (`sans-serif`,
//! `monospace`, their aliases) and the fallback families per language and script, each a list
//! of files in `/system/fonts`. Read once, when a text first needs a system font.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Family {
    /// `sans-serif`, `monospace`...; none for a fallback family.
    pub(super) name: Option<String>,
    /// BCP 47 tags the family is for (`und-Arab`, `zh-Hans`, `ja`, `und-Zsye`).
    pub(super) langs: Vec<String>,
    pub(super) fonts: Vec<Font>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Font {
    pub(super) file: String,
    pub(super) weight: u16,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Fonts {
    pub(super) families: Vec<Family>,
    /// `(alias, family)`, both lowercase.
    pub(super) aliases: Vec<(String, String)>,
}

/// The value of attribute `name` in the inside of a tag.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    while let Some(at) = rest.find(name) {
        let (before, after) = (&rest[..at], rest[at + name.len()..].trim_start());
        rest = &rest[at + name.len()..];
        if !before.ends_with(char::is_whitespace) { continue }
        let Some(value) = after.strip_prefix('=').map(str::trim_start) else { continue };
        let quote = value.chars().next().filter(|q| matches!(q, '"' | '\''))?;
        return value[1..].split(quote).next();
    }
    None
}

/// Parses `fonts.xml`. Lenient: unknown tags and attributes are skipped, and a file that isn't
/// one gives no families.
pub(super) fn parse(xml: &str) -> Fonts {
    let mut out = Fonts::default();
    let mut family: Option<Family> = None;
    // The `<font>` being read: its weight, and the text seen so far (the file name).
    let mut font: Option<(u16, String)> = None;
    let mut rest = xml;
    while let Some(open) = rest.find('<') {
        if let Some((_, text)) = &mut font { text.push_str(&rest[..open]) }
        rest = &rest[open..];
        if rest.starts_with("<!--") {
            rest = rest.find("-->").map_or("", |end| &rest[end + 3..]);
            continue;
        }
        let Some(close) = rest.find('>') else { break };
        let tag = rest[1..close].trim();
        rest = &rest[close + 1..];
        let name = tag.trim_start_matches('/').split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("");
        match (name, tag.starts_with('/')) {
            ("family", false) => {
                let new = Family {
                    name: attr(tag, "name").map(str::to_lowercase),
                    langs: attr(tag, "lang").map(|l| l.split([',', ' ']).filter(|t| !t.is_empty()).map(str::to_owned).collect()).unwrap_or_default(),
                    fonts: Vec::new(),
                };
                if tag.ends_with('/') { out.families.push(new) } else { family = Some(new) }
            }
            ("family", true) => out.families.extend(family.take()),
            ("font", false) => font = Some((attr(tag, "weight").and_then(|w| w.parse().ok()).unwrap_or(400), String::new())),
            ("font", true) => {
                if let (Some((weight, text)), Some(family)) = (font.take(), &mut family) {
                    let file = text.trim();
                    if !file.is_empty() { family.fonts.push(Font { file: file.to_owned(), weight }) }
                }
            }
            ("alias", false) => {
                // An alias with a weight names one face of the family; the family's files still serve.
                if let (Some(name), Some(to)) = (attr(tag, "name"), attr(tag, "to")) {
                    out.aliases.push((name.to_lowercase(), to.to_lowercase()));
                }
            }
            _ => {}
        }
    }
    out
}

/// The scripts a `lang` tag's family is for: its script subtag, or the scripts of a language
/// written without one.
fn lang_scripts(lang: &str) -> &'static [&'static str] {
    const KNOWN: &[(&str, &[&str])] = &[
        ("ja", &["Jpan", "Hira", "Kana", "Hani"]),
        ("ko", &["Kore", "Hang", "Hani"]),
        ("zh-Hans", &["Hans", "Hani"]),
        ("zh-Hant", &["Hant", "Hani", "Bopo"]),
        ("zh-Bopo", &["Bopo"]),
    ];
    KNOWN.iter().find(|(tag, _)| *tag == lang).map_or(&[], |(_, scripts)| scripts)
}

fn lang_has_script(lang: &str, script: &str) -> bool {
    lang.split('-').skip(1).any(|part| part == script) || lang_scripts(lang).contains(&script)
}

/// Lowercase letters and digits of `s`: "Noto Color Emoji" and "NotoColorEmoji" compare equal.
fn squash(s: &str) -> String {
    s.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
}

impl Fonts {
    fn named(&self, name: &str) -> Option<&Family> {
        let name = name.to_lowercase();
        let target = self.aliases.iter().find(|(alias, _)| *alias == name).map_or(name.as_str(), |(_, to)| to.as_str());
        self.families.iter().find(|f| f.name.as_deref() == Some(target))
    }

    /// The files of family `name`: a named family or alias (`monospace`), else every file
    /// whose name starts with the family's (`Noto Color Emoji` finds `NotoColorEmoji.ttf`).
    pub(super) fn family_files(&self, name: &str) -> Vec<&str> {
        if let Some(family) = self.named(name) {
            return family.fonts.iter().map(|f| f.file.as_str()).collect();
        }
        let want = squash(name);
        if want.is_empty() { return Vec::new() }
        let mut out: Vec<&str> = Vec::new();
        for font in self.families.iter().flat_map(|f| &f.fonts) {
            let stem = font.file.split(['-', '.']).next().unwrap_or("");
            if squash(stem) == want && !out.contains(&font.file.as_str()) { out.push(&font.file) }
        }
        out
    }

    /// The files of the first fallback families for `script` (an ISO 15924 code), the one in
    /// `locale`'s language first: Han in a Japanese locale draws with the Japanese font.
    pub(super) fn script_files(&self, script: &str, locale: Option<&str>) -> Vec<&str> {
        let language = locale.and_then(|l| l.split(['-', '_']).next()).unwrap_or("");
        let mut families: Vec<&Family> = self.families.iter()
            .filter(|f| f.name.is_none() && f.langs.iter().any(|lang| lang_has_script(lang, script)))
            .collect();
        families.sort_by_key(|f| !f.langs.iter().any(|lang| !language.is_empty() && lang.split('-').next() == Some(language)));
        families.iter().take(2).flat_map(|f| &f.fonts).map(|f| f.file.as_str()).collect()
    }
}

const CONFIG: &str = "/system/etc/fonts.xml";
const FONT_DIR: &str = "/system/fonts";

fn system() -> &'static Fonts {
    static FONTS: std::sync::OnceLock<Fonts> = std::sync::OnceLock::new();
    FONTS.get_or_init(|| std::fs::read_to_string(CONFIG).map(|xml| parse(&xml)).unwrap_or_default())
}

fn existing<'a>(files: impl IntoIterator<Item = &'a str>, out: &mut Vec<PathBuf>) {
    for file in files {
        let path = Path::new(FONT_DIR).join(file);
        if !out.contains(&path) && path.is_file() { out.push(path) }
    }
}

pub(super) fn family_files(names: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for name in names { existing(system().family_files(name), &mut out) }
    out
}

pub(super) fn fallback_family_files(chars: &[char], locale: Option<&str>) -> Vec<PathBuf> {
    use icu_properties::{CodePointMapData, PropertyNamesShort, props::Script};
    let names = PropertyNamesShort::<Script>::new();
    let mut scripts: Vec<String> = Vec::new();
    for &ch in chars {
        let script = CodePointMapData::<Script>::new().get(ch);
        if matches!(script, Script::Common | Script::Inherited | Script::Unknown) { continue }
        let Some(tag) = names.get_locale_script(script) else { continue };
        if !scripts.iter().any(|s| s == tag.as_str()) { scripts.push(tag.as_str().to_owned()) }
    }
    let mut out = Vec::new();
    for script in &scripts { existing(system().script_files(script, locale), &mut out) }
    out
}

pub(super) fn installed_families() -> Vec<super::installed::RawFamily> {
    system().families.iter().filter_map(|f| {
        let name = f.name.clone()?;
        let mut weights: Vec<u16> = f.fonts.iter().map(|font| font.weight).collect();
        weights.sort_unstable();
        weights.dedup();
        Some(super::installed::RawFamily { monospace: name.contains("mono"), name, weights })
    }).collect()
}

/// The families are read when a text first needs one: nothing to build ahead.
pub(super) fn prewarm() {}
