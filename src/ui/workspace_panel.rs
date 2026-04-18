//! Workspace side panel — VS Code EXPLORER-style folder browser.
//!
//! Renders the currently loaded `Workspace` as a flat scrollable list
//! with header controls (close / refresh) and per-row icons for
//! directories, DXF, and DWG files.  Clicks dispatch `WorkspaceFileClick`
//! and `WorkspaceDirToggle` messages; the host decides whether to open a
//! tab, switch to an existing one, or expand/collapse a directory.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::app::workspace::{visible_entries, EntryKind, Workspace};
use crate::app::Message;

const PANEL_WIDTH: f32 = 240.0;
const ROW_HEIGHT: f32 = 22.0;
const HEADER_HEIGHT: f32 = 28.0;
const INDENT_PX: f32 = 12.0;

const PANEL_BG: Color = Color { r: 0.145, g: 0.145, b: 0.145, a: 1.0 };
const HEADER_BG: Color = Color { r: 0.18, g: 0.18, b: 0.18, a: 1.0 };
const ROW_HOVER: Color = Color { r: 0.22, g: 0.22, b: 0.22, a: 1.0 };
const ROW_ACTIVE: Color = Color { r: 0.20, g: 0.35, b: 0.55, a: 1.0 };
const TEXT_COLOR: Color = Color { r: 0.85, g: 0.85, b: 0.85, a: 1.0 };
const TEXT_MUTED: Color = Color { r: 0.55, g: 0.55, b: 0.55, a: 1.0 };
const BORDER_COLOR: Color = Color { r: 0.25, g: 0.25, b: 0.25, a: 1.0 };

/// Render the workspace side panel.  `active_path` is highlighted in
/// blue when it matches a file row.
pub fn view_panel<'a>(
    ws: &'a Workspace,
    active_path: Option<&'a Path>,
    expanded_dirs: &'a HashSet<PathBuf>,
) -> Element<'a, Message> {
    let header = panel_header(ws);
    let body = panel_body(ws, active_path, expanded_dirs);

    let content = column![header, body].width(Length::Fill);

    container(content)
        .width(Length::Fixed(PANEL_WIDTH))
        .height(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(PANEL_BG)),
            border: Border {
                color: BORDER_COLOR,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn panel_header<'a>(ws: &'a Workspace) -> Element<'a, Message> {
    let label = text(ws.root_label())
        .size(12)
        .color(TEXT_COLOR)
        .width(Length::Fill);

    let refresh_btn = button(text("↻").size(12).color(TEXT_COLOR))
        .on_press(Message::WorkspaceRefresh)
        .style(|_: &Theme, status| button::Style {
            background: Some(Background::Color(match status {
                button::Status::Hovered => ROW_HOVER,
                _ => Color::TRANSPARENT,
            })),
            text_color: TEXT_COLOR,
            border: Border::default(),
            ..Default::default()
        })
        .padding([2, 6]);

    let close_btn = button(text("×").size(12).color(TEXT_COLOR))
        .on_press(Message::WorkspaceClose)
        .style(|_: &Theme, status| button::Style {
            background: Some(Background::Color(match status {
                button::Status::Hovered => Color { r: 0.5, g: 0.2, b: 0.2, a: 1.0 },
                _ => Color::TRANSPARENT,
            })),
            text_color: TEXT_COLOR,
            border: Border::default(),
            ..Default::default()
        })
        .padding([2, 6]);

    container(row![label, refresh_btn, close_btn]
        .spacing(0)
        .align_y(iced::Center))
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(HEADER_BG)),
            border: Border {
                color: BORDER_COLOR,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .width(Length::Fill)
        .height(Length::Fixed(HEADER_HEIGHT))
        .padding([0, 8])
        .into()
}

fn panel_body<'a>(
    ws: &'a Workspace,
    active_path: Option<&'a Path>,
    expanded_dirs: &'a HashSet<PathBuf>,
) -> Element<'a, Message> {
    let visible = visible_entries(&ws.entries, expanded_dirs);

    if visible.is_empty() {
        let msg = text("(empty workspace)")
            .size(11)
            .color(TEXT_MUTED);
        return container(msg).padding(8).into();
    }

    let mut col = column![].spacing(0).width(Length::Fill);
    for entry in visible {
        col = col.push(row_element(entry, active_path, expanded_dirs));
    }

    scrollable(col).height(Length::Fill).into()
}

fn row_element<'a>(
    entry: &'a crate::app::workspace::WorkspaceEntry,
    active_path: Option<&'a Path>,
    expanded_dirs: &'a HashSet<PathBuf>,
) -> Element<'a, Message> {
    let depth = entry.depth.saturating_sub(1) as f32; // depth 1 = no indent
    let indent = depth * INDENT_PX;

    let (icon_str, is_dir, expanded) = match entry.kind {
        EntryKind::Directory => {
            let expanded = expanded_dirs.contains(&entry.path);
            let icon = if expanded { "▼ 📁" } else { "▶ 📁" };
            (icon, true, expanded)
        }
        EntryKind::DxfFile => ("📐", false, false),
        EntryKind::DwgFile => ("📏", false, false),
        EntryKind::Truncated => ("⋯", false, false),
    };
    let _ = expanded; // reserved for future tooltip variations

    let is_active = !is_dir
        && active_path.map(|p| p == entry.path.as_path()).unwrap_or(false);

    let label_color = match entry.kind {
        EntryKind::Truncated => TEXT_MUTED,
        _ => TEXT_COLOR,
    };

    let label = row![
        iced::widget::Space::new().width(Length::Fixed(indent)),
        text(icon_str).size(11).color(label_color),
        iced::widget::Space::new().width(Length::Fixed(4.0)),
        text(entry.name.clone()).size(11).color(label_color),
    ]
    .align_y(iced::Center);

    let msg = if is_dir {
        Message::WorkspaceDirToggle(entry.path.clone())
    } else if matches!(entry.kind, EntryKind::Truncated) {
        // Truncation marker — no-op click.
        Message::Noop
    } else {
        Message::WorkspaceFileClick(entry.path.clone())
    };

    button(label)
        .on_press(msg)
        .padding([0, 8])
        .width(Length::Fill)
        .height(Length::Fixed(ROW_HEIGHT))
        .style(move |_: &Theme, status| button::Style {
            background: Some(Background::Color(match (is_active, status) {
                (true, _) => ROW_ACTIVE,
                (false, button::Status::Hovered) => ROW_HOVER,
                _ => Color::TRANSPARENT,
            })),
            text_color: label_color,
            border: Border::default(),
            ..Default::default()
        })
        .into()
}
