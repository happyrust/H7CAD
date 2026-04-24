//! SVG Export options dialog — Phase 6 Option D.
//!
//! A small floating window that lets the user toggle the `SvgExportOptions`
//! fields before firing the save-file dialog.  Layout mirrors `page_setup`
//! (toolbar + scrollable form) so the look-and-feel stays consistent.

use crate::app::{Message, SvgExportDialogField};
use crate::io::svg_export::SvgExportOptions;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Background, Border, Color, Element, Fill, Theme};

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

/// Render one boolean toggle as a check-mark pill.
fn toggle<'a>(
    label: &'static str,
    active: bool,
    field: SvgExportDialogField,
) -> Element<'a, Message> {
    let prefix = if active { "✓ " } else { "  " };
    button(text(format!("{prefix}{label}")).size(11))
        .on_press(Message::SvgExportDialogToggle(field))
        .style(pill(active))
        .padding([4, 10])
        .width(Fill)
        .into()
}

pub fn view_window<'a>(
    opts: &'a SvgExportOptions,
    font_size_buf: &'a str,
    min_stroke_buf: &'a str,
    lw_scale_buf: &'a str,
) -> Element<'a, Message> {
    // ── Toolbar ───────────────────────────────────────────────────────────
    let toolbar = container(
        row![
            button(text("Cancel").size(12))
                .on_press(Message::SvgExportDialogClose)
                .style(btn(false))
                .padding([4, 14]),
            Space::new().width(Fill),
            button(text("Export…").size(12))
                .on_press(Message::SvgExportDialogCommit)
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

    use SvgExportDialogField as F;

    let stroke_section = column![
        section_label("Color & Strokes"),
        toggle("Monochrome (force black)", opts.monochrome,         F::Monochrome),
        row![
            lbl("Min stroke width (mm)"),
            text_input("0.1", min_stroke_buf)
                .on_input(|s| Message::SvgExportDialogEdit(F::MinStrokeWidth, s))
                .on_submit(Message::SvgExportDialogCommit)
                .style(field_style)
                .size(12)
                .width(100),
        ].spacing(8).align_y(iced::Center),
        row![
            lbl("Line-weight scale"),
            text_input("0.2646", lw_scale_buf)
                .on_input(|s| Message::SvgExportDialogEdit(F::LineWeightScale, s))
                .on_submit(Message::SvgExportDialogCommit)
                .style(field_style)
                .size(12)
                .width(100),
        ].spacing(8).align_y(iced::Center),
    ].spacing(8);

    let text_section = column![
        section_label("Text"),
        toggle("Tessellate text (TextAsGeometry)", opts.text_as_geometry, F::TextAsGeometry),
        toggle(
            "Native dimension text",
            opts.native_dimension_text,
            F::NativeDimensionText,
        ),
        row![
            lbl("Font size scale"),
            text_input("0.8", font_size_buf)
                .on_input(|s| Message::SvgExportDialogEdit(F::FontSizeScale, s))
                .on_submit(Message::SvgExportDialogCommit)
                .style(field_style)
                .size(12)
                .width(100),
        ].spacing(8).align_y(iced::Center),
    ].spacing(8);

    let geometry_section = column![
        section_label("Geometry"),
        toggle("Include hatch fills",              opts.include_hatches, F::IncludeHatches),
        toggle("Deduplicate blocks via <defs>",    opts.use_block_defs,  F::UseBlockDefs),
        toggle("Emit native curves (circle/arc)",  opts.native_curves,   F::NativeCurves),
        toggle("Emit native splines",              opts.native_splines,  F::NativeSplines),
    ].spacing(6);

    let images_section = column![
        section_label("Images"),
        toggle("Include raster images",            opts.include_images,  F::IncludeImages),
        toggle("Embed images as base64 data URI",  opts.embed_images,    F::EmbedImages),
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

/// Apply one `SvgExportDialogField` toggle to a mutable `SvgExportOptions`.
/// Pulled out as a free function so the toggle semantics can be exercised
/// from a unit test without constructing a full `H7CAD` state.
pub fn apply_toggle(opts: &mut SvgExportOptions, field: SvgExportDialogField) {
    use SvgExportDialogField as F;
    match field {
        F::Monochrome           => opts.monochrome           = !opts.monochrome,
        F::TextAsGeometry       => opts.text_as_geometry     = !opts.text_as_geometry,
        F::IncludeHatches       => opts.include_hatches      = !opts.include_hatches,
        F::UseBlockDefs         => opts.use_block_defs       = !opts.use_block_defs,
        F::IncludeImages        => opts.include_images       = !opts.include_images,
        F::EmbedImages          => opts.embed_images         = !opts.embed_images,
        F::NativeCurves         => opts.native_curves        = !opts.native_curves,
        F::NativeSplines        => opts.native_splines       = !opts.native_splines,
        F::NativeDimensionText  => opts.native_dimension_text = !opts.native_dimension_text,
        F::FontSizeScale | F::MinStrokeWidth | F::LineWeightScale => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_flips_each_boolean_field() {
        let mut o = SvgExportOptions::default();
        let (mono0, tag0, hat0, blk0, img0, emb0, cur0, spl0, dim0) = (
            o.monochrome, o.text_as_geometry, o.include_hatches,
            o.use_block_defs, o.include_images, o.embed_images,
            o.native_curves, o.native_splines, o.native_dimension_text,
        );
        apply_toggle(&mut o, SvgExportDialogField::Monochrome);
        apply_toggle(&mut o, SvgExportDialogField::TextAsGeometry);
        apply_toggle(&mut o, SvgExportDialogField::IncludeHatches);
        apply_toggle(&mut o, SvgExportDialogField::UseBlockDefs);
        apply_toggle(&mut o, SvgExportDialogField::IncludeImages);
        apply_toggle(&mut o, SvgExportDialogField::EmbedImages);
        apply_toggle(&mut o, SvgExportDialogField::NativeCurves);
        apply_toggle(&mut o, SvgExportDialogField::NativeSplines);
        apply_toggle(&mut o, SvgExportDialogField::NativeDimensionText);

        assert_ne!(o.monochrome,            mono0);
        assert_ne!(o.text_as_geometry,      tag0);
        assert_ne!(o.include_hatches,       hat0);
        assert_ne!(o.use_block_defs,        blk0);
        assert_ne!(o.include_images,        img0);
        assert_ne!(o.embed_images,          emb0);
        assert_ne!(o.native_curves,         cur0);
        assert_ne!(o.native_splines,        spl0);
        assert_ne!(o.native_dimension_text, dim0);
    }

    #[test]
    fn numeric_fields_do_not_flip_booleans() {
        let o0 = SvgExportOptions::default();
        let mut o = o0.clone();
        apply_toggle(&mut o, SvgExportDialogField::FontSizeScale);
        apply_toggle(&mut o, SvgExportDialogField::MinStrokeWidth);
        apply_toggle(&mut o, SvgExportDialogField::LineWeightScale);
        assert_eq!(o.monochrome, o0.monochrome);
        assert_eq!(o.font_size_scale, o0.font_size_scale);
        assert_eq!(o.min_stroke_width, o0.min_stroke_width);
        assert_eq!(o.line_weight_scale, o0.line_weight_scale);
    }

    #[test]
    fn double_toggle_returns_to_original() {
        let o0 = SvgExportOptions::default();
        let mut o = o0.clone();
        apply_toggle(&mut o, SvgExportDialogField::Monochrome);
        apply_toggle(&mut o, SvgExportDialogField::Monochrome);
        assert_eq!(o.monochrome, o0.monochrome);
    }
}
