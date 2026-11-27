use super::android::{Font, parse};

/// The shape of `/system/etc/fonts.xml` on Android 15, cut down.
const FONTS_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<!-- WARNING: Parsing of this file by third-party apps is not supported. -->
<familyset version="23">
    <!-- first font is default -->
    <family name="sans-serif">
        <font weight="400" style="normal">Roboto-Regular.ttf
            <axis tag="wght" stylevalue="400" />
        </font>
        <font weight="700" style="normal">Roboto-Regular.ttf
            <axis tag="wght" stylevalue="700" />
        </font>
    </family>
    <alias name="arial" to="sans-serif" />
    <alias name="sans-serif-medium" to="sans-serif" weight="500" />
    <family name="monospace">
        <font weight="400" style="normal" postScriptName="DroidSansMono">DroidSansMono.ttf</font>
    </family>
    <family lang="und-Arab" variant="elegant">
        <font weight="400" style="normal">NotoNaskhArabic-Regular.ttf</font>
        <font weight="700" style="normal">NotoNaskhArabic-Bold.ttf</font>
    </family>
    <family lang="zh-Hans">
        <font weight="400" style="normal" index="2" postScriptName="NotoSansCJKjp-Regular">NotoSansCJK-Regular.ttc</font>
    </family>
    <family lang="zh-Hant,zh-Bopo">
        <font weight="400" style="normal" index="3">NotoSansCJK-Regular.ttc</font>
    </family>
    <family lang="ja">
        <font weight="400" style="normal" index="0">NotoSansCJK-Regular.ttc</font>
        <font weight="400" style="normal" index="0" fallbackFor="serif">NotoSerifCJK-Regular.ttc</font>
    </family>
    <family lang="ko">
        <font weight="400" style="normal" index="1">NotoSansCJK-Regular.ttc</font>
    </family>
    <family lang="und-Zsye">
        <font weight="400" style="normal">NotoColorEmoji.ttf</font>
    </family>
    <family lang="und-Zsym">
        <font weight="400" style="normal">NotoSansSymbols-Regular-Subsetted.ttf</font>
    </family>
</familyset>
"#;

#[test]
fn named_families_fonts_and_aliases_are_read() {
    let fonts = parse(FONTS_XML);
    assert_eq!(fonts.families.len(), 9);
    let sans = &fonts.families[0];
    assert_eq!(sans.name.as_deref(), Some("sans-serif"));
    // The file name is the font's text; its <axis> children add nothing to it.
    assert_eq!(sans.fonts, [Font { file: "Roboto-Regular.ttf".into(), weight: 400 }, Font { file: "Roboto-Regular.ttf".into(), weight: 700 }]);
    assert_eq!(fonts.aliases, [("arial".to_owned(), "sans-serif".to_owned()), ("sans-serif-medium".to_owned(), "sans-serif".to_owned())]);
    assert_eq!(fonts.families[4].langs, ["zh-Hant", "zh-Bopo"]);
}

#[test]
fn a_family_is_found_by_name_alias_or_file_name() {
    let fonts = parse(FONTS_XML);
    assert_eq!(fonts.family_files("monospace"), ["DroidSansMono.ttf"]);
    assert_eq!(fonts.family_files("Arial"), ["Roboto-Regular.ttf", "Roboto-Regular.ttf"]);
    assert_eq!(fonts.family_files("Noto Color Emoji"), ["NotoColorEmoji.ttf"]);
    assert_eq!(fonts.family_files("Noto Sans Symbols"), ["NotoSansSymbols-Regular-Subsetted.ttf"]);
    assert_eq!(fonts.family_files("Droid Sans Mono"), ["DroidSansMono.ttf"]);
    assert!(fonts.family_files("DejaVu Sans").is_empty());
    assert!(fonts.family_files("").is_empty());
}

#[test]
fn a_script_finds_its_fallback_family() {
    let fonts = parse(FONTS_XML);
    assert_eq!(fonts.script_files("Arab", None), ["NotoNaskhArabic-Regular.ttf", "NotoNaskhArabic-Bold.ttf"]);
    assert_eq!(fonts.script_files("Hang", None), ["NotoSansCJK-Regular.ttc"]);
    assert_eq!(fonts.script_files("Hira", None), ["NotoSansCJK-Regular.ttc", "NotoSerifCJK-Regular.ttc"]);
    assert!(fonts.script_files("Deva", None).is_empty());
    // A named family is never a script's fallback.
    assert!(fonts.script_files("Latn", None).is_empty());
}

#[test]
fn han_prefers_the_locales_language() {
    let fonts = parse(FONTS_XML);
    // Simplified Chinese comes first in the file; a Japanese locale takes the Japanese family.
    assert_eq!(fonts.script_files("Hani", None)[0], "NotoSansCJK-Regular.ttc");
    assert_eq!(fonts.script_files("Hani", Some("ja-JP")), ["NotoSansCJK-Regular.ttc", "NotoSerifCJK-Regular.ttc", "NotoSansCJK-Regular.ttc"]);
    assert_eq!(fonts.script_files("Hani", Some("ko")).len(), 2);
}

#[test]
fn a_file_that_is_not_fonts_xml_gives_nothing() {
    assert!(parse("").families.is_empty());
    assert!(parse("not xml at all <").families.is_empty());
    assert!(parse("<familyset><family name='x'><font>unterminated").families.is_empty());
}
