//! PDF Export options dialog — 三十三轮 Phase 2b.
//!
//! Floating window that lets the user toggle `PdfExportOptions` before
//! firing the save-file dialog.  Layout mirrors `svg_export_dialog`
//! (toolbar + scrollable form) so the two export dialogs feel consistent.

use crate::app::{Message, PdfExportDialogField};
use crate::io::pdf_export::{PdfExportOptions, PdfFontChoice};
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Background, Border, Color, Element, Fill, Theme};

// ── Palette (deliberately identical to svg_export_dialog so the two
//     option windows share a visual language). ────────────────────────────

const TB:     Color = Color { r: 0.13, g: 0.13, b: 0.13, a: 1.0 };
const BG:     Color = Color { r: 0.15, g: 0.15, b: 0.15, a: 1.0 };
const BORDER: Color = Color { r: 0.35, g: 0.35, b: 0.35, a: 1.0 };
const TEXT:   Color = Color { r: 0.88, g: 0.88, b: 0.88, a: 1.0 };
const DIM:    Color = Color { r: 0.55, g: 0.55, b: 0.55, a: 1.0 };
const ACCENT: Color = Color { r: 0.25, g: 0.50, b: 0.85, a: 1.0 };
const ACTIVE: Color = Color { r: 0.20, g: 0.40, b: 0.70, a: 1.0 };
const FIELD:  Color = Color { r: 0.10, g: 0.10, b: 0.10, a: 1.0 };

fn btn(accent: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_: &Theme, st| button::Style {
        background: Some(Background::Color(match (accent, st) {
            (true,  button::Status::Hovered | button::Status::Pressed) => Color { r: 0.20, g: 0.42, b: 0.72, a: 1.0 },
            (false, button::Status::Hovered | button::Status::Pressed) => Color { r: 0.28, g: 0.28, b: 0.28, a: 1.0 },
            (true,  _) => ACCENT,
            _ => Color { r: 0.22, g: 0.22, b: 0.22, a: 1.0 },
        })),
        text_color: TEXT,
        border: Border { color: BORDER, width: 1.0, radius: 4.0.into() },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

fn pill(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_: &Theme, st| button::Style {
        background: Some(Background::Color(match (active, st) {
            (true,  _) => ACTIVE,
            (false, button::Status::Hovered | button::Status::Pressed) => Color { r: 0.28, g: 0.28, b: 0.28, a: 1.0 },
            _ => Color { r: 0.20, g: 0.20, b: 0.20, a: 1.0 },
        })),
        text_color: TEXT,
        border: Border { color: BORDER, width: 1.0, radius: 3.0.into() },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

fn field_style(_: &Theme, _: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: Background::Color(FIELD),
        border: Border { color: BORDER, width: 1.0, radius: 3.0.into() },
        icon: TEXT, placeholder: DIM, value: TEXT, selection: ACCENT,
    }
}

fn hdivider<'a>() -> Element<'a, Message> {
    container(Space::new().width(Fill).height(1))
        .width(Fill).height(1)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(BORDER)),
            ..Default::default()
        })
        .into()
}

fn section_label<'a>(s: &'static str) -> Element<'a, Message> {
    text(s).size(11).color(DIM).into()
}

fn toggle<'a>(
    label: &'static str,
    active: bool,
    field: PdfExportDialogField,
) -> Element<'a, Message> {
    let prefix = if active { "✓ " } else { "  " };
    button(text(format!("{prefix}{label}")).size(11))
        .on_press(Message::PdfExportDialogToggle(field))
        .style(pill(active))
        .padding([4, 10])
        .width(Fill)
        .into()
}

fn font_pill<'a>(
    label: &'static str,
    choice: PdfFontChoice,
    active: bool,
) -> Element<'a, Message> {
    button(text(label).size(11))
        .on_press(Message::PdfExportDialogSelectFont(choice))
        .style(pill(active))
        .padding([4, 10])
        .width(Fill)
        .into()
}

pub fn view_window<'a>(
    opts: &'a PdfExportOptions,
    font_size_buf: &'a str,
) -> Element<'a, Message> {
    // ── Toolbar ───────────────────────────────────────────────────────────
    let toolbar = container(
        row![
            button(text("Cancel").size(12))
                .on_press(Message::PdfExportDialogClose)
                .style(btn(false))
                .padding([4, 14]),
            Space::new().width(Fill),
            button(text("Export…").size(12))
                .on_press(Message::PdfExportDialogCommit)
                .style(btn(true))
                .padding([4, 20]),
        ]
        .align_y(iced::Center)
    )
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(TB)),
        ..Default::default()
    })
    .width(Fill)
    .padding([5, 10]);

    let lbl = |s: &'static str| text(s).size(11).color(DIM).width(150);

    use PdfExportDialogField as F;

    let stroke_section = column![
        section_label("Color & Strokes"),
        toggle("Monochrome (force black)", opts.monochrome, F::Monochrome),
    ].spacing(8);

    let text_section = column![
        section_label("Text"),
        toggle(
            "Tessellate text (fallback to wires)",
            opts.text_as_geometry,
            F::TextAsGeometry,
        ),
        toggle(
            "Native dimension text",
            opts.native_dimension_text,
            F::NativeDimensionText,
        ),
        row![
            lbl("Built-in font"),
            font_pill("Helvetica", PdfFontChoice::Helvetica, opts.font_family == PdfFontChoice::Helvetica),
            font_pill("Times",     PdfFontChoice::TimesRoman, opts.font_family == PdfFontChoice::TimesRoman),
            font_pill("Courier",   PdfFontChoice::Courier,    opts.font_family == PdfFontChoice::Courier),
        ]
        .spacing(6)
        .align_y(iced::Center),
        row![
            lbl("Font size scale"),
            text_input("0.8", font_size_buf)
                .on_input(|s| Message::PdfExportDialogEdit(F::FontSizeScale, s))
                .on_submit(Message::PdfExportDialogCommit)
                .style(field_style)
                .size(12)
                .width(100),
        ].spacing(8).align_y(iced::Center),
    ].spacing(8);

    let geometry_section = column![
        section_label("Geometry"),
        toggle("Include hatch fills",                opts.include_hatches, F::IncludeHatches),
        toggle("Emit hatch patterns (line family)",  opts.hatch_patterns,  F::HatchPatterns),
        toggle("Emit native curves (circle/arc)",    opts.native_curves,   F::NativeCurves),
    ].spacing(6);

    let images_section = column![
        section_label("Images"),
        toggle("Include raster images",  opts.include_images, F::IncludeImages),
        toggle("Embed image bytes",      opts.embed_images,   F::EmbedImages),
    ].spacing(6);

    // ── Main scrollable form ──────────────────────────────────────────────
    let form = column![
        stroke_section,
        hdivider(),
        text_section,
        hdivider(),
        geometry_section,
        hdivider(),
        images_section,
    ]
    .spacing(12)
    .padding(16)
    .width(Fill);

    let content = scrollable(form).width(Fill).height(Fill);

    container(
        column![toolbar, hdivider(), content].spacing(0)
    )
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(BG)),
        ..Default::default()
    })
    .width(Fill)
    .height(Fill)
    .into()
}

/// Apply one `PdfExportDialogField` toggle to a mutable `PdfExportOptions`.
/// Pulled out as a free function so the toggle semantics can be exercised
/// from a unit test without constructing a full `H7CAD` state.
pub fn apply_toggle(opts: &mut PdfExportOptions, field: PdfExportDialogField) {
    use PdfExportDialogField as F;
    match field {
        F::Monochrome           => opts.monochrome            = !opts.monochrome,
        F::TextAsGeometry       => opts.text_as_geometry      = !opts.text_as_geometry,
        F::IncludeHatches       => opts.include_hatches       = !opts.include_hatches,
        F::HatchPatterns        => opts.hatch_patterns        = !opts.hatch_patterns,
        F::NativeCurves         => opts.native_curves         = !opts.native_curves,
        F::IncludeImages        => opts.include_images        = !opts.include_images,
        F::EmbedImages          => opts.embed_images          = !opts.embed_images,
        F::NativeDimensionText  => opts.native_dimension_text = !opts.native_dimension_text,
        F::FontSizeScale => {}
    }
}

/// Apply a font choice selection.
pub fn apply_font_choice(opts: &mut PdfExportOptions, choice: PdfFontChoice) {
    opts.font_family = choice;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_flips_each_boolean_field() {
        let mut o = PdfExportOptions::default();
        let (mono0, tag0, hat0, pat0, cur0, img0, emb0, dim0) = (
            o.monochrome, o.text_as_geometry, o.include_hatches, o.hatch_patterns,
            o.native_curves, o.include_images, o.embed_images, o.native_dimension_text,
        );
        apply_toggle(&mut o, PdfExportDialogField::Monochrome);
        apply_toggle(&mut o, PdfExportDialogField::TextAsGeometry);
        apply_toggle(&mut o, PdfExportDialogField::IncludeHatches);
        apply_toggle(&mut o, PdfExportDialogField::HatchPatterns);
        apply_toggle(&mut o, PdfExportDialogField::NativeCurves);
        apply_toggle(&mut o, PdfExportDialogField::IncludeImages);
        apply_toggle(&mut o, PdfExportDialogField::EmbedImages);
        apply_toggle(&mut o, PdfExportDialogField::NativeDimensionText);

        assert_ne!(o.monochrome,            mono0);
        assert_ne!(o.text_as_geometry,      tag0);
        assert_ne!(o.include_hatches,       hat0);
        assert_ne!(o.hatch_patterns,        pat0);
        assert_ne!(o.native_curves,         cur0);
        assert_ne!(o.include_images,        img0);
        assert_ne!(o.embed_images,          emb0);
        assert_ne!(o.native_dimension_text, dim0);
    }

    #[test]
    fn font_choice_updates_options() {
        let mut o = PdfExportOptions::default();
        assert_eq!(o.font_family, PdfFontChoice::Helvetica);
        apply_font_choice(&mut o, PdfFontChoice::TimesRoman);
        assert_eq!(o.font_family, PdfFontChoice::TimesRoman);
        apply_font_choice(&mut o, PdfFontChoice::Courier);
        assert_eq!(o.font_family, PdfFontChoice::Courier);
    }

    #[test]
    fn font_size_scale_field_does_not_flip_on_toggle_path() {
        // Ensures typed-field enum variant is a no-op for the boolean toggle
        // path (it only participates in `Message::PdfExportDialogEdit`).
        let mut o = PdfExportOptions::default();
        let before = o.font_size_scale;
        apply_toggle(&mut o, PdfExportDialogField::FontSizeScale);
        assert_eq!(o.font_size_scale, before);
    }
}
