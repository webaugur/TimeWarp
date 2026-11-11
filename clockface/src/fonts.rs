use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};

/// Register Liberation Serif under the family name `"serif"`.
///
/// Call once on the egui context before [`crate::paint`]. egui's built-in
/// faces are sans-serif, and the numerals need a serif.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "LiberationSerif".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../fonts/LiberationSerif-Regular.ttf"
        ))),
    );
    fonts
        .families
        .entry(FontFamily::Name("serif".into()))
        .or_default()
        .insert(0, "LiberationSerif".to_owned());
    ctx.set_fonts(fonts);
}
