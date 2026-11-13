use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};

/// Register Liberation Serif under the family name `"serif"`.
///
/// Call once on the egui context before [`crate::paint`]. egui's built-in
/// faces are sans-serif, and the numerals need a serif. The status mark
/// U+1F5A7 is a one-glyph subset of Noto Sans Symbols 2. The calendar and
/// weather marks are a subset of Noto Emoji (both Google, SIL OFL 1.1).
/// Liberation Serif does not contain them, so they are fallbacks in the same family.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "LiberationSerif".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../fonts/LiberationSerif-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "NotoSymbols".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../fonts/NotoSansSymbols2-1F5A7.ttf"
        ))),
    );
    fonts.font_data.insert(
        "NotoEmoji".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../fonts/NotoEmoji-Clock.ttf"
        ))),
    );
    let serif = fonts
        .families
        .entry(FontFamily::Name("serif".into()))
        .or_default();
    serif.insert(0, "LiberationSerif".to_owned());
    serif.push("NotoEmoji".to_owned());
    serif.push("NotoSymbols".to_owned());
    ctx.set_fonts(fonts);
}
