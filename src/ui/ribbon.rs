// Ribbon — tab bar + 3-row tool area.
//
// Button sizes:
//   LargeTool / LargeDropdown  — full ribbon height (3 rows), icon + label [+ ▾]
//   Tool / Dropdown            — 1-row height, icon only [+ ▾ on right]
//
// Dropdown items within a group are collected into columns of 3 rows.

use std::collections::HashMap;

use std::time::Duration;

use acadrust::types::{Color as AcadColor, LineWeight};
use iced::widget::tooltip::Position as TipPos;
use iced::widget::{button, column, container, mouse_area, row, svg, text, tooltip};
use iced::{Background, Border, Color, Element, Fill, Length, Padding, Theme};

use crate::modules::registry;
use crate::modules::{CadModule, IconKind, ModuleEvent, RibbonItem, ToolDef};
use crate::ui::properties::{LwItem, LinetypeItem, acad_color_display, color_picker_dropdown, lw_options};
use crate::app::Message;

// ── Layout constants ───────────────────────────────────────────────────────
//
// ROW_H is the single source of truth for ribbon sizing.
// Change it and the entire ribbon scales proportionally.

/// Height of one ribbon row (px). Imported from ui — single source of truth.
use super::ROW_H;

// ── Derived from ROW_H ────────────────────────────────────────────────────
/// Icon size inside a 3-row (large) button.
const LARGE_ICON: f32 = ROW_H * 1.5;      // 39 px at ROW_H = 26
/// Icon size inside a 1-row (small) button.
const SMALL_ICON: f32 = ROW_H * 0.7;      // 18.2 px at ROW_H = 26
/// Width of a 3-row (large) button.
const LARGE_W: f32 = ROW_H * 2.2;         // 57.2 px at ROW_H = 26
/// Width of a 1-row (small) button.
const SMALL_W: f32 = ROW_H;               // 26 px
/// Width of the ▾ strip on a small dropdown.
const ARROW_W: f32 = ROW_H * 0.4;         // 10.4 px
/// Height of the ▾ strip at the bottom of a large dropdown.
const LARGE_ARR: f32 = ROW_H * 0.55;      // 14.3 px
/// Total ribbon tool-area height = 3 × ROW_H + 6 px v-padding + 12 px group-label.
const TOOL_BAR_H: f32 = 3.0 * ROW_H + 18.0;

// ── Tab-bar constants (not tied to ROW_H) ────────────────────────────────
const TOP_ARR_W: f32 = 12.0;
const TOP_HIST_W: f32 = 28.0;
const TOP_HIST_GAP: f32 = 4.0;
const UNDO_HISTORY_ID: &str = "UNDO_HISTORY";
const REDO_HISTORY_ID: &str = "REDO_HISTORY";
const LAYER_COMBO_ID: &str = "LAYER_COMBO";
const PROP_COLOR_ID: &str = "PROP_COLOR";
const PROP_LINETYPE_ID: &str = "PROP_LINETYPE";
const PROP_LW_ID: &str = "PROP_LW";

// ── Ribbon state ───────────────────────────────────────────────────────────

pub struct Ribbon {
    modules: Vec<Box<dyn CadModule>>,
    active: usize,
    active_tool: Option<String>,
    pub wireframe: bool,
    pub ortho_mode: bool,
    pub open_dropdown: Option<String>,
    last_cmd: HashMap<&'static str, &'static str>,
    pub layer_names: Vec<String>,
    pub active_layer: String,
    pub layer_infos: Vec<LayerInfo>,
    /// Active object color — ACI / ByLayer / ByBlock.
    pub active_color: AcadColor,
    /// Active linetype override ("ByLayer", "Continuous", …).
    pub active_linetype: String,
    /// Active lineweight.
    pub active_lineweight: LineWeight,
    /// Linetypes loaded from the current document (with ASCII art).
    pub available_linetypes: Vec<LinetypeItem>,
    /// Whether the full ACI palette is expanded inside the color picker overlay.
    pub prop_color_palette_open: bool,
}

/// Per-layer display data shown in the ribbon layer dropdown.
#[derive(Clone, Debug)]
pub struct LayerInfo {
    pub name: String,
    pub color: Color,
    pub visible: bool,
    pub frozen: bool,
    pub locked: bool,
}

impl Ribbon {
    pub fn new() -> Self {
        Self {
            modules: registry::all_modules(),
            active: 0,
            active_tool: None,
            wireframe: false,
            ortho_mode: true,
            open_dropdown: None,
            last_cmd: HashMap::new(),
            layer_names: vec!["0".to_string()],
            active_layer: "0".to_string(),
            layer_infos: vec![LayerInfo {
                name: "0".to_string(),
                color: Color::WHITE,
                visible: true,
                frozen: false,
                locked: false,
            }],
            active_color: AcadColor::ByLayer,
            active_linetype: "ByLayer".to_string(),
            active_lineweight: LineWeight::ByLayer,
            available_linetypes: vec![LinetypeItem { name: "Continuous".to_string(), art: String::new() }],
            prop_color_palette_open: false,
        }
    }

    pub fn set_layers(&mut self, infos: Vec<LayerInfo>, active: &str) {
        self.active_layer = active.to_string();
        self.layer_names = infos.iter().map(|l| l.name.clone()).collect();
        self.layer_infos = infos;
    }

    pub fn set_available_linetypes(&mut self, items: Vec<LinetypeItem>) {
        self.available_linetypes = items;
    }

    pub fn select(&mut self, index: usize) {
        if index < self.modules.len() {
            self.active = index;
        }
    }
    pub fn activate_tool(&mut self, id: &str) {
        self.active_tool = Some(id.to_string());
    }
    pub fn set_wireframe(&mut self, w: bool) {
        self.wireframe = w;
    }
    pub fn set_ortho(&mut self, ortho: bool) {
        self.ortho_mode = ortho;
    }

    pub fn toggle_dropdown(&mut self, id: &str) {
        if self.open_dropdown.as_deref() == Some(id) {
            self.open_dropdown = None;
        } else {
            self.open_dropdown = Some(id.to_string());
        }
    }
    pub fn close_dropdown(&mut self) {
        self.open_dropdown = None;
    }

    /// Returns the index of the Layout module in the modules list.
    pub fn layout_module_index(&self) -> Option<usize> {
        self.modules.iter().position(|m| m.id() == "layout")
    }

    /// Returns true if the currently active tab is the Layout module.
    pub fn active_is_layout(&self) -> bool {
        self.modules
            .get(self.active)
            .map(|m| m.id() == "layout")
            .unwrap_or(false)
    }

    pub fn select_dropdown_item(&mut self, dropdown_id: &'static str, cmd: &'static str) {
        self.last_cmd.insert(dropdown_id, cmd);
        self.open_dropdown = None;
    }

    #[allow(dead_code)]
    pub fn last_dropdown_cmd(
        &self,
        dropdown_id: &'static str,
        default: &'static str,
    ) -> &'static str {
        self.last_cmd.get(dropdown_id).copied().unwrap_or(default)
    }

    // ── View ──────────────────────────────────────────────────────────────

    pub fn view(&self, is_paper: bool, undo_count: usize, redo_count: usize) -> Element<'_, Message> {
        // ── Logo ──────────────────────────────────────────────────────────
        let logo_svg = {
            let handle = svg::Handle::from_memory(include_bytes!("../../assets/logo.svg"));
            svg(handle).width(30).height(28)
        };
        let logo = button(logo_svg)
            .on_press(Message::ToggleAppMenu)
            .style(|_: &Theme, status| button::Style {
                background: Some(Background::Color(match status {
                    button::Status::Hovered | button::Status::Pressed => Color {
                        r: 0.80,
                        g: 0.25,
                        b: 0.15,
                        a: 1.0,
                    },
                    _ => LOGO_RED,
                })),
                border: Border {
                    radius: 2.0.into(),
                    ..Default::default()
                },
                shadow: iced::Shadow::default(),
                snap: false,
                ..Default::default()
            })
            .padding([0, 4]);

        // ── Tab buttons ───────────────────────────────────────────────────
        let history_controls = row![
            render_history_control("↶", "Undo", UNDO_HISTORY_ID, undo_count, &self.open_dropdown),
            render_history_control("↷", "Redo", REDO_HISTORY_ID, redo_count, &self.open_dropdown),
        ]
        .spacing(TOP_HIST_GAP)
        .align_y(iced::Center);

        let tab_buttons = self.modules.iter().enumerate().fold(
            row![logo, history_controls].align_y(iced::Center).spacing(6),
            |row_acc, (i, module)| {
                // Hide the Layout tab when in model space.
                if module.id() == "layout" && !is_paper {
                    return row_acc;
                }

                let is_active = i == self.active;
                let is_contextual = module.id() == "layout";
                let accent = if is_contextual {
                    ACCENT_GOLD
                } else {
                    ACCENT_BLUE
                };
                let text_inactive = if is_contextual {
                    Color {
                        r: 0.90,
                        g: 0.72,
                        b: 0.30,
                        a: 1.0,
                    }
                } else {
                    Color {
                        r: 0.75,
                        g: 0.75,
                        b: 0.75,
                        a: 1.0,
                    }
                };
                let hover_bg = if is_contextual {
                    Color {
                        r: 0.28,
                        g: 0.24,
                        b: 0.12,
                        a: 1.0,
                    }
                } else {
                    Color {
                        r: 0.25,
                        g: 0.25,
                        b: 0.25,
                        a: 1.0,
                    }
                };
                let btn = container(
                    button(text(module.title()).size(12))
                        .on_press(Message::RibbonSelectTab(i))
                        .style(move |_: &Theme, status| button::Style {
                            background: Some(Background::Color(match (is_active, status) {
                                (true, _) => RIBBON_BG,
                                (false, button::Status::Hovered) => hover_bg,
                                _ => Color::TRANSPARENT,
                            })),
                            text_color: if is_active {
                                Color::WHITE
                            } else {
                                text_inactive
                            },
                            border: Border {
                                color: if is_active {
                                    accent
                                } else {
                                    Color::TRANSPARENT
                                },
                                width: if is_active { 2.0 } else { 0.0 },
                                radius: 0.0.into(),
                            },
                            shadow: iced::Shadow::default(),
                            snap: false,
                        })
                        .padding([5, 14]),
                )
                .style(move |_: &Theme| container::Style {
                    border: Border {
                        color: if is_active {
                            accent
                        } else {
                            Color::TRANSPARENT
                        },
                        width: if is_active { 2.0 } else { 0.0 },
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                });
                row_acc.push(btn)
            },
        );

        let tab_bar = container(tab_buttons)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(TOPBAR_BG)),
                ..Default::default()
            })
            .width(Length::Fill)
            .height(28);

        // ── Tool area ─────────────────────────────────────────────────────
        // If Layout tab is active but we're in model space, fall back to Home (index 0).
        let effective_active = if !is_paper
            && self
                .modules
                .get(self.active)
                .map(|m| m.id() == "layout")
                .unwrap_or(false)
        {
            0
        } else {
            self.active
        };
        let tool_area: Element<'_, Message> =
            if let Some(module) = self.modules.get(effective_active) {
                let groups = module.ribbon_groups();
                let wireframe = self.wireframe;
                let ortho_mode = self.ortho_mode;
                let active_tool = self.active_tool.clone();
                let open_dd = self.open_dropdown.clone();
                let last_cmd = &self.last_cmd;
                let layer_infos = &self.layer_infos;
                let active_layer = &self.active_layer;
                let active_color = self.active_color;
                let active_linetype = &self.active_linetype;
                let active_lineweight = self.active_lineweight;

                let mut widgets: Vec<Element<Message>> = Vec::new();
                let mut first_group = true;

                for group in groups {
                    // Separator between groups.
                    if !first_group {
                        widgets.push(
                            container(text(""))
                                .width(1)
                                .height(Fill)
                                .style(|_: &Theme| container::Style {
                                    background: Some(Background::Color(BORDER_DARK)),
                                    ..Default::default()
                                })
                                .into(),
                        );
                    }
                    first_group = false;

                    // Build group content: large items side-by-side, smalls stack 3-per-column.
                    let mut items_row: Vec<Element<Message>> = Vec::new();
                    let mut small_buf: Vec<Element<Message>> = Vec::new();

                    for item in group.tools {
                        let is_large = matches!(
                            &item,
                            RibbonItem::LargeTool(_)
                            | RibbonItem::LargeDropdown { .. }
                            | RibbonItem::LayerComboGroup { .. }
                            | RibbonItem::PropertiesGroup { .. }
                        );

                        if is_large {
                            // Flush any pending small items first.
                            flush_small_col(&mut small_buf, &mut items_row);
                            items_row.push(render_large(
                                item,
                                &active_tool,
                                &open_dd,
                                last_cmd,
                                wireframe,
                                ortho_mode,
                                layer_infos,
                                active_layer,
                                active_color,
                                active_linetype,
                                active_lineweight,
                            ));
                        } else {
                            small_buf.push(render_small(
                                item,
                                &active_tool,
                                &open_dd,
                                last_cmd,
                                wireframe,
                                ortho_mode,
                            ));
                            if small_buf.len() == 3 {
                                flush_small_col(&mut small_buf, &mut items_row);
                            }
                        }
                    }
                    flush_small_col(&mut small_buf, &mut items_row);

                    let tools_el = items_row
                        .into_iter()
                        .fold(row![].spacing(2).height(Fill).align_y(iced::Top), |r, e| {
                            r.push(e)
                        });

                    widgets.push(
                        column![
                            tools_el,
                            container(text(group.title).size(9).color(GROUP_LABEL)).padding([1, 4]),
                        ]
                        .spacing(0)
                        .padding([3u16, 4])
                        .height(Length::Fill)
                        .into(),
                    );
                }

                widgets
                    .into_iter()
                    .fold(row![].spacing(0).height(Length::Fill), |r, w| r.push(w))
                    .into()
            } else {
                text("").into()
            };

        let tool_bar = container(tool_area)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(RIBBON_BG)),
                border: Border {
                    color: BORDER_DARK,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .width(Length::Fill)
            .height(TOOL_BAR_H);

        column![tab_bar, tool_bar].into()
    }

    // ── Dropdown overlay ──────────────────────────────────────────────────

    pub fn dropdown_overlay(
        &self,
        undo_labels: &[String],
        redo_labels: &[String],
    ) -> Option<Element<'_, Message>> {
        let open_id = self.open_dropdown.as_deref()?;

        if open_id == UNDO_HISTORY_ID || open_id == REDO_HISTORY_ID {
            let is_undo = open_id == UNDO_HISTORY_ID;
            let labels = if is_undo { undo_labels } else { redo_labels };
            if labels.is_empty() {
                return None;
            }

            let rows: Vec<Element<Message>> = labels
                .iter()
                .enumerate()
                .map(|(idx, label)| {
                    let step = idx + 1;
                    button(text(label.clone()).size(11).color(LABEL_ON))
                        .on_press(if is_undo {
                            Message::UndoMany(step)
                        } else {
                            Message::RedoMany(step)
                        })
                        .style(|_: &Theme, status| button::Style {
                            background: Some(Background::Color(match status {
                                button::Status::Hovered | button::Status::Pressed => ROW_HOVER,
                                _ => Color::TRANSPARENT,
                            })),
                            ..Default::default()
                        })
                        .width(Fill)
                        .padding([5, 10])
                        .into()
                })
                .collect();

            let panel = container(column(rows))
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(PANEL_BG)),
                    border: Border {
                        color: PANEL_BORDER,
                        width: 1.0,
                        radius: 3.0.into(),
                    },
                    ..Default::default()
                })
                .width(Length::Fixed(170.0));

            let positioned = container(panel)
                .align_left(Fill)
                .align_top(Fill)
                .padding(Padding {
                    top: 28.0,
                    left: compute_history_dropdown_left(open_id),
                    ..Default::default()
                })
                .width(Fill)
                .height(Fill);

            return Some(
                mouse_area(positioned)
                    .on_press(Message::CloseRibbonDropdown)
                    .into(),
            );
        }

        if open_id == LAYER_COMBO_ID {
            return self.layer_combo_overlay();
        }

        if open_id == PROP_COLOR_ID {
            return self.prop_color_overlay();
        }
        if open_id == PROP_LINETYPE_ID {
            return self.prop_linetype_overlay();
        }
        if open_id == PROP_LW_ID {
            return self.prop_lw_overlay();
        }

        let module = self.modules.get(self.active)?;
        let groups = module.ribbon_groups();
        let mut items_list: Option<Vec<(&'static str, &'static str, IconKind)>> = None;
        let mut dd_default = "";
        let mut dd_id: &'static str = "";

        'outer: for group in &groups {
            for item in &group.tools {
                let (id, items, default) = match item {
                    RibbonItem::Dropdown {
                        id, items, default, ..
                    } => (id, items, default),
                    RibbonItem::LargeDropdown {
                        id, items, default, ..
                    } => (id, items, default),
                    _ => continue,
                };
                if *id == open_id {
                    items_list = Some(items.clone());
                    dd_default = default;
                    dd_id = id;
                    break 'outer;
                }
            }
        }
        let items = items_list?;
        let last_cmd = self.last_cmd.get(dd_id).copied().unwrap_or(dd_default);

        let rows: Vec<Element<Message>> = items
            .iter()
            .map(|(cmd, label, item_icon)| {
                let is_current = *cmd == last_cmd;
                let checkmark = text(if is_current { "✓" } else { "  " })
                    .size(11)
                    .color(if is_current {
                        CHECK_COLOR
                    } else {
                        Color::TRANSPARENT
                    })
                    .width(Length::Fixed(14.0));
                let icon_el: Element<Message> = match *item_icon {
                    IconKind::Glyph(s) => text(s)
                        .size(13)
                        .color(ICON_COLOR)
                        .width(Length::Fixed(20.0))
                        .into(),
                    IconKind::Svg(bytes) => {
                        let handle = svg::Handle::from_memory(bytes);
                        svg(handle).width(20).height(20).into()
                    }
                };
                let label_el =
                    text(*label)
                        .size(11)
                        .color(if is_current { LABEL_ON } else { LABEL_OFF });

                button(
                    row![checkmark, icon_el, label_el]
                        .spacing(4)
                        .align_y(iced::Center),
                )
                .on_press(Message::DropdownSelectItem {
                    dropdown_id: dd_id,
                    cmd: *cmd,
                })
                .style(|_: &Theme, status| button::Style {
                    background: Some(Background::Color(match status {
                        button::Status::Hovered | button::Status::Pressed => ROW_HOVER,
                        _ => Color::TRANSPARENT,
                    })),
                    ..Default::default()
                })
                .width(Fill)
                .padding([4, 10])
                .into()
            })
            .collect();

        let panel = container(column(rows))
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(PANEL_BG)),
                border: Border {
                    color: PANEL_BORDER,
                    width: 1.0,
                    radius: 3.0.into(),
                },
                ..Default::default()
            })
            .width(Length::Fixed(190.0));

        let top_offset = 28.0 + TOOL_BAR_H;
        let left_offset = compute_dropdown_left(&groups, open_id);
        let positioned = container(panel)
            .align_left(Fill)
            .align_top(Fill)
            .padding(Padding {
                top: top_offset,
                left: left_offset,
                ..Default::default()
            })
            .width(Fill)
            .height(Fill);

        Some(
            mouse_area(positioned)
                .on_press(Message::CloseRibbonDropdown)
                .into(),
        )
    }

    fn layer_combo_overlay(&self) -> Option<Element<'_, Message>> {
        let rows: Vec<Element<Message>> = self
            .layer_infos
            .iter()
            .map(|info| {
                let is_active = info.name == self.active_layer;
                let lc = info.color;
                let lv = info.visible;
                let lf = info.frozen;
                let ll = info.locked;
                let name = info.name.clone();

                let swatch = container(text(""))
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(lc)),
                        border: Border {
                            color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.5 },
                            width: 1.0,
                            radius: 1.0.into(),
                        },
                        ..Default::default()
                    })
                    .width(12)
                    .height(12);

                let vis = text(if lv { "●" } else { "○" })
                    .size(10)
                    .color(if lv {
                        Color { r: 0.95, g: 0.85, b: 0.20, a: 1.0 }
                    } else {
                        Color { r: 0.45, g: 0.45, b: 0.45, a: 1.0 }
                    });
                let freeze = text("✱").size(10).color(if lf {
                    Color { r: 0.40, g: 0.80, b: 1.00, a: 1.0 }
                } else {
                    Color { r: 0.95, g: 0.85, b: 0.20, a: 1.0 }
                });
                let lock = text(if ll { "🔒" } else { "🔓" }).size(10).color(
                    if ll {
                        Color { r: 0.95, g: 0.70, b: 0.20, a: 1.0 }
                    } else {
                        Color { r: 0.55, g: 0.55, b: 0.55, a: 1.0 }
                    },
                );
                let checkmark = text(if is_active { "✓" } else { "  " })
                    .size(11)
                    .color(if is_active { CHECK_COLOR } else { Color::TRANSPARENT })
                    .width(Length::Fixed(14.0));
                let label = text(&info.name)
                    .size(11)
                    .color(if is_active { LABEL_ON } else { LABEL_OFF });

                button(
                    row![checkmark, vis, freeze, lock, swatch, label]
                        .spacing(5)
                        .align_y(iced::Center),
                )
                .on_press(Message::RibbonLayerChanged(name))
                .style(|_: &Theme, status| button::Style {
                    background: Some(Background::Color(match status {
                        button::Status::Hovered | button::Status::Pressed => ROW_HOVER,
                        _ => Color::TRANSPARENT,
                    })),
                    ..Default::default()
                })
                .width(Fill)
                .padding([4, 8])
                .into()
            })
            .collect();

        let panel = container(column(rows))
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(PANEL_BG)),
                border: Border { color: PANEL_BORDER, width: 1.0, radius: 3.0.into() },
                ..Default::default()
            })
            .width(Length::Fixed(220.0));

        // Position below the ribbon layers group area
        let top_offset = 28.0 + TOOL_BAR_H;
        let groups = self.modules.get(self.active)?.ribbon_groups();
        let left_offset = compute_layer_combo_left(&groups);
        let positioned = container(panel)
            .align_left(Fill)
            .align_top(Fill)
            .padding(Padding { top: top_offset, left: left_offset, ..Default::default() })
            .width(Fill)
            .height(Fill);

        Some(mouse_area(positioned).on_press(Message::CloseRibbonDropdown).into())
    }

    fn prop_color_overlay(&self) -> Option<Element<'_, Message>> {
        let picker = color_picker_dropdown(
            self.prop_color_palette_open,
            Message::RibbonColorPaletteToggle,
            Some(Message::RibbonColorChanged(AcadColor::ByLayer)),
            Some(Message::RibbonColorChanged(AcadColor::ByBlock)),
            |aci| Message::RibbonColorChanged(AcadColor::Index(aci)),
        );

        let panel = container(picker)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(PANEL_BG)),
                border: Border { color: PANEL_BORDER, width: 1.0, radius: 3.0.into() },
                ..Default::default()
            })
            .width(Length::Fixed(200.0));

        let top_offset = 28.0 + TOOL_BAR_H;
        let groups = self.modules.get(self.active)?.ribbon_groups();
        let left_offset = compute_prop_combo_left(&groups, PROP_COLOR_ID);
        let positioned = container(panel)
            .align_left(Fill)
            .align_top(Fill)
            .padding(Padding { top: top_offset, left: left_offset, ..Default::default() })
            .width(Fill)
            .height(Fill);

        Some(mouse_area(positioned).on_press(Message::CloseRibbonDropdown).into())
    }

    fn prop_linetype_overlay(&self) -> Option<Element<'_, Message>> {
        let active_lt = &self.active_linetype;

        // Prepend ByLayer/ByBlock, then all document linetypes (deduped).
        let mut items: Vec<LinetypeItem> = vec![
            LinetypeItem { name: "ByLayer".to_string(), art: String::new() },
            LinetypeItem { name: "ByBlock".to_string(), art: String::new() },
        ];
        for lt in &self.available_linetypes {
            if lt.name != "ByLayer" && lt.name != "ByBlock" {
                items.push(lt.clone());
            }
        }

        let rows: Vec<Element<Message>> = items.into_iter().map(|lt| {
            let is_cur = lt.name == *active_lt;
            let check = text(if is_cur { "✓" } else { "  " })
                .size(11).color(if is_cur { CHECK_COLOR } else { Color::TRANSPARENT })
                .width(Length::Fixed(14.0));
            let name_col = text(lt.name.clone())
                .size(11).color(if is_cur { LABEL_ON } else { LABEL_OFF })
                .width(Length::Fixed(90.0));
            let art_col = text(lt.art.clone())
                .size(9).color(Color { r: 0.55, g: 0.55, b: 0.55, a: 1.0 });
            let name = lt.name.clone();
            button(row![check, name_col, art_col].spacing(4).align_y(iced::Center))
                .on_press(Message::RibbonLinetypeChanged(name))
                .style(|_: &Theme, status| button::Style {
                    background: Some(Background::Color(match status {
                        button::Status::Hovered | button::Status::Pressed => ROW_HOVER,
                        _ => Color::TRANSPARENT,
                    })),
                    ..Default::default()
                })
                .width(Fill).padding([4, 6]).into()
        }).collect();

        // Wrap in a scrollable so long lists don't get clipped.
        use iced::widget::scrollable;
        let list = container(scrollable(column(rows)).height(Length::Fixed(200.0)))
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(PANEL_BG)),
                border: Border { color: PANEL_BORDER, width: 1.0, radius: 3.0.into() },
                ..Default::default()
            })
            .width(Length::Fixed(220.0));

        let top_offset = 28.0 + TOOL_BAR_H;
        let groups = self.modules.get(self.active)?.ribbon_groups();
        let left_offset = compute_prop_combo_left(&groups, PROP_LINETYPE_ID);
        let positioned = container(list)
            .align_left(Fill)
            .align_top(Fill)
            .padding(Padding { top: top_offset, left: left_offset, ..Default::default() })
            .width(Fill)
            .height(Fill);

        Some(mouse_area(positioned).on_press(Message::CloseRibbonDropdown).into())
    }

    fn prop_lw_overlay(&self) -> Option<Element<'_, Message>> {
        let active_lw = self.active_lineweight;
        let rows: Vec<Element<Message>> = lw_options().into_iter().map(|item| {
            let is_cur = item.0 == active_lw;
            let label = item.to_string();
            let check = text(if is_cur { "✓" } else { "  " })
                .size(11).color(if is_cur { CHECK_COLOR } else { Color::TRANSPARENT })
                .width(Length::Fixed(14.0));
            button(row![check, text(label).size(11).color(if is_cur { LABEL_ON } else { LABEL_OFF })].spacing(5).align_y(iced::Center))
                .on_press(Message::RibbonLineweightChanged(item.0))
                .style(|_: &Theme, status| button::Style {
                    background: Some(Background::Color(match status {
                        button::Status::Hovered | button::Status::Pressed => ROW_HOVER,
                        _ => Color::TRANSPARENT,
                    })),
                    ..Default::default()
                })
                .width(Fill).padding([4, 8]).into()
        }).collect();

        self.prop_overlay_positioned(rows, PROP_LW_ID, 140.0)
    }

    fn prop_overlay_positioned<'a>(&'a self, rows: Vec<Element<'a, Message>>, dd_id: &str, width: f32) -> Option<Element<'a, Message>> {
        let panel = container(column(rows))
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(PANEL_BG)),
                border: Border { color: PANEL_BORDER, width: 1.0, radius: 3.0.into() },
                ..Default::default()
            })
            .width(Length::Fixed(width));

        let top_offset = 28.0 + TOOL_BAR_H;
        let groups = self.modules.get(self.active)?.ribbon_groups();
        let left_offset = compute_prop_combo_left(&groups, dd_id);
        let positioned = container(panel)
            .align_left(Fill)
            .align_top(Fill)
            .padding(Padding { top: top_offset, left: left_offset, ..Default::default() })
            .width(Fill)
            .height(Fill);

        Some(mouse_area(positioned).on_press(Message::CloseRibbonDropdown).into())
    }
}

impl Default for Ribbon {
    fn default() -> Self {
        Self::new()
    }
}

// ── Rendering helpers ──────────────────────────────────────────────────────

/// Flush up-to-3 small items as a vertical column into the group row.
fn flush_small_col<'a>(buf: &mut Vec<Element<'a, Message>>, out: &mut Vec<Element<'a, Message>>) {
    if buf.is_empty() {
        return;
    }
    let col = column(std::mem::take(buf)).spacing(1);
    out.push(col.into());
}

fn make_icon(icon: IconKind, size: f32) -> Element<'static, Message> {
    match icon {
        IconKind::Glyph(s) => text(s).size(size * 0.7).color(Color::WHITE).into(),
        IconKind::Svg(bytes) => {
            let handle = svg::Handle::from_memory(bytes);
            svg(handle).width(size).height(size).into()
        }
    }
}

fn is_active_tool(
    id: &str,
    active_tool: &Option<String>,
    wireframe: bool,
    ortho_mode: bool,
) -> bool {
    match id {
        "WIREFRAME" => wireframe,
        "SOLID" => !wireframe,
        "ORTHO" => ortho_mode,
        "PERSP" => !ortho_mode,
        id => active_tool.as_deref() == Some(id),
    }
}

/// Render a 1-row small button (Tool or Dropdown).
fn render_small<'a>(
    item: RibbonItem,
    active_tool: &Option<String>,
    open_dd: &Option<String>,
    last_cmd: &HashMap<&'static str, &'static str>,
    wireframe: bool,
    ortho_mode: bool,
) -> Element<'a, Message> {
    match item {
        RibbonItem::Tool(t) => {
            let active = is_active_tool(t.id, active_tool, wireframe, ortho_mode);
            let event = t.event.clone();
            let tool_id = t.id.to_string();
            let tip_text = format!("{}\nCommand: {}", t.label, t.id);
            let btn = button(make_icon(t.icon, SMALL_ICON))
                .on_press(Message::RibbonToolClick { tool_id, event })
                .style(move |_: &Theme, status| tool_btn_style(active, status))
                .width(Length::Fixed(SMALL_W))
                .height(ROW_H)
                .padding([4, 4]);
            tooltip(btn, make_tip(tip_text), TipPos::Bottom)
                .gap(6.0)
                .delay(Duration::from_millis(400))
                .style(tip_style)
                .into()
        }

        RibbonItem::Dropdown {
            id,
            icon,
            items,
            default,
            ..
        } => {
            let active = active_tool.as_deref() == Some(id)
                || items
                    .iter()
                    .any(|(cmd, _, _)| active_tool.as_deref() == Some(cmd));
            let dd_open = open_dd.as_deref() == Some(id);
            let last = last_cmd.get(id).copied().unwrap_or(default);
            // First launch: first item's icon. After selection: selected item's icon.
            let cur_icon = last_cmd
                .get(id)
                .copied()
                .and_then(|cmd| {
                    items
                        .iter()
                        .find(|(c, _, _)| *c == cmd)
                        .map(|(_, _, ik)| *ik)
                })
                .or_else(|| items.first().map(|(_, _, ik)| *ik))
                .unwrap_or(icon);

            // Tooltip: show current selection.
            let cur_label = last_cmd
                .get(id)
                .copied()
                .and_then(|cmd| {
                    items
                        .iter()
                        .find(|(c, _, _)| *c == cmd)
                        .map(|(_, lbl, _)| *lbl)
                })
                .or_else(|| items.first().map(|(_, lbl, _)| *lbl))
                .unwrap_or(id);
            let tip_text = format!("{}\nCommand: {}", cur_label, last);

            // Left: icon button → fires last-used command.
            let icon_btn = button(make_icon(cur_icon, SMALL_ICON))
                .on_press(Message::Command(last.to_string()))
                .style(move |_: &Theme, status| tool_btn_style(active, status))
                .width(Length::Fixed(SMALL_W))
                .height(ROW_H)
                .padding([4, 4]);

            // Right: ▾ button → opens dropdown.
            let arr_tip = format!("{} seçenekleri", cur_label);
            let arr_btn = button(
                container(text("▾").size(7).color(ARROW_COLOR))
                    .width(Fill)
                    .height(Fill)
                    .align_x(iced::Center)
                    .align_y(iced::Center),
            )
            .on_press(Message::ToggleRibbonDropdown(id.to_string()))
            .style(move |_: &Theme, status| button::Style {
                background: Some(Background::Color(match status {
                    button::Status::Hovered | button::Status::Pressed => TOOL_HOVER,
                    _ if dd_open => TOOL_ACTIVE,
                    _ => Color::TRANSPARENT,
                })),
                border: Border {
                    radius: 2.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .width(Length::Fixed(ARROW_W))
            .height(ROW_H)
            .padding(0);

            let icon_with_tip = tooltip(icon_btn, make_tip(tip_text), TipPos::Bottom)
                .gap(6.0)
                .delay(Duration::from_millis(400))
                .style(tip_style);
            let arr_with_tip = tooltip(arr_btn, make_tip(arr_tip), TipPos::Bottom)
                .gap(6.0)
                .delay(Duration::from_millis(400))
                .style(tip_style);

            row![icon_with_tip, arr_with_tip]
                .spacing(0)
                .height(ROW_H)
                .into()
        }

        // LargeTool / LargeDropdown should never reach render_small — handled upstream.
        _ => text("").into(),
    }
}

/// Render a full-height large button (LargeTool, LargeDropdown, or LayerCombo).
fn render_large<'a>(
    item: RibbonItem,
    active_tool: &Option<String>,
    open_dd: &Option<String>,
    last_cmd: &HashMap<&'static str, &'static str>,
    wireframe: bool,
    ortho_mode: bool,
    layer_infos: &'a [LayerInfo],
    active_layer: &'a str,
    active_color: AcadColor,
    active_linetype: &'a str,
    active_lineweight: LineWeight,
) -> Element<'a, Message> {
    match item {
        RibbonItem::LargeTool(t) => {
            let active = is_active_tool(t.id, active_tool, wireframe, ortho_mode);
            let event = t.event.clone();
            let tool_id = t.id.to_string();
            let tip_text = format!("{}\nCommand: {}", t.label, t.id);
            let btn = button(
                column![
                    make_icon(t.icon, LARGE_ICON),
                    text(t.label).size(10).color(LABEL_COLOR),
                ]
                .align_x(iced::Center)
                .spacing(3),
            )
            .on_press(Message::RibbonToolClick { tool_id, event })
            .style(move |_: &Theme, status| tool_btn_style(active, status))
            .width(Length::Fixed(LARGE_W))
            .height(Fill)
            .padding(Padding {
                top: 6.0,
                right: 4.0,
                bottom: 4.0,
                left: 4.0,
            });
            tooltip(btn, make_tip(tip_text), TipPos::Bottom)
                .gap(6.0)
                .delay(Duration::from_millis(400))
                .style(tip_style)
                .into()
        }

        RibbonItem::LargeDropdown {
            id,
            label,
            icon,
            items,
            default,
        } => {
            let active = active_tool.as_deref() == Some(id)
                || items
                    .iter()
                    .any(|(cmd, _, _)| active_tool.as_deref() == Some(cmd));
            let dd_open = open_dd.as_deref() == Some(id);
            let last = last_cmd.get(id).copied().unwrap_or(default);
            // First launch: first item's icon. After selection: selected item's icon.
            let cur_icon = last_cmd
                .get(id)
                .copied()
                .and_then(|cmd| {
                    items
                        .iter()
                        .find(|(c, _, _)| *c == cmd)
                        .map(|(_, _, ik)| *ik)
                })
                .or_else(|| items.first().map(|(_, _, ik)| *ik))
                .unwrap_or(icon);

            // Tooltip: show current variant label + command id.
            let cur_label = last_cmd
                .get(id)
                .copied()
                .and_then(|cmd| {
                    items
                        .iter()
                        .find(|(c, _, _)| *c == cmd)
                        .map(|(_, lbl, _)| *lbl)
                })
                .or_else(|| items.first().map(|(_, lbl, _)| *lbl))
                .unwrap_or(label);
            let tip_text = format!("{}\nCommand: {}", cur_label, last);
            let arr_tip = format!("{} seçenekleri", label);

            // Top part: icon + label → fires last-used command.
            let top_btn = button(
                column![
                    make_icon(cur_icon, LARGE_ICON),
                    text(label).size(10).color(LABEL_COLOR),
                ]
                .align_x(iced::Center)
                .spacing(3),
            )
            .on_press(Message::Command(last.to_string()))
            .style(move |_: &Theme, status| tool_btn_style(active, status))
            .width(Length::Fixed(LARGE_W))
            .height(Fill)
            .padding(Padding {
                top: 6.0,
                right: 4.0,
                bottom: 2.0,
                left: 4.0,
            });

            // Bottom strip: ▾ → opens dropdown.
            let arr_btn = button(
                container(text("▾").size(9).color(ARROW_COLOR))
                    .width(Fill)
                    .height(Fill)
                    .align_x(iced::Center)
                    .align_y(iced::Center),
            )
            .on_press(Message::ToggleRibbonDropdown(id.to_string()))
            .style(move |_: &Theme, status| button::Style {
                background: Some(Background::Color(match status {
                    button::Status::Hovered | button::Status::Pressed => TOOL_HOVER,
                    _ if dd_open => TOOL_ACTIVE,
                    _ => Color::TRANSPARENT,
                })),
                border: Border {
                    radius: 3.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .width(Length::Fixed(LARGE_W))
            .height(LARGE_ARR)
            .padding(0);

            let top_with_tip = tooltip(top_btn, make_tip(tip_text), TipPos::Bottom)
                .gap(6.0)
                .delay(Duration::from_millis(400))
                .style(tip_style);
            let arr_with_tip = tooltip(arr_btn, make_tip(arr_tip), TipPos::Bottom)
                .gap(6.0)
                .delay(Duration::from_millis(400))
                .style(tip_style);

            column![top_with_tip, arr_with_tip]
                .spacing(0)
                .width(Length::Fixed(LARGE_W))
                .height(Fill)
                .into()
        }

        RibbonItem::LayerComboGroup { row2, row3 } => {
            const COMBO_W: f32 = LARGE_W * 2.5;

            // ── Top: layer combo button ───────────────────────────────────
            let info = layer_infos.iter().find(|l| l.name == active_layer);
            let lc = info.map(|l| l.color).unwrap_or(Color::WHITE);
            let lv = info.map(|l| l.visible).unwrap_or(true);
            let lf = info.map(|l| l.frozen).unwrap_or(false);
            let ll = info.map(|l| l.locked).unwrap_or(false);
            let is_open = open_dd.as_deref() == Some(LAYER_COMBO_ID);

            let vis_icon = text(if lv { "●" } else { "○" }).size(10).color(
                if lv { Color { r: 0.95, g: 0.85, b: 0.20, a: 1.0 } }
                else   { Color { r: 0.45, g: 0.45, b: 0.45, a: 1.0 } },
            );
            let freeze_icon = text("✱").size(10).color(
                if lf { Color { r: 0.40, g: 0.80, b: 1.00, a: 1.0 } }
                else  { Color { r: 0.95, g: 0.85, b: 0.20, a: 1.0 } },
            );
            let lock_icon = text(if ll { "🔒" } else { "🔓" }).size(10).color(
                if ll { Color { r: 0.95, g: 0.70, b: 0.20, a: 1.0 } }
                else  { Color { r: 0.65, g: 0.65, b: 0.65, a: 1.0 } },
            );
            let swatch = container(text(""))
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(lc)),
                    border: Border {
                        color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.5 },
                        width: 1.0,
                        radius: 1.0.into(),
                    },
                    ..Default::default()
                })
                .width(12)
                .height(12);

            // Clip layer name to available space.
            const ICONS_USED: f32 = 10.0 + 10.0 + 10.0 + 12.0 + 10.0 + 5.0 * 4.0 + 16.0 + 16.0;
            let name_w = (COMBO_W - ICONS_USED).max(40.0);

            let combo_btn = button(
                row![
                    vis_icon, freeze_icon, lock_icon, swatch,
                    container(text(active_layer).size(11).color(Color::WHITE))
                        .width(name_w).clip(true),
                    text("▾").size(9).color(Color { r: 0.7, g: 0.7, b: 0.7, a: 1.0 }),
                ]
                .spacing(4)
                .align_y(iced::Center),
            )
            .on_press(Message::ToggleRibbonDropdown(LAYER_COMBO_ID.to_string()))
            .style(move |_: &Theme, status| button::Style {
                background: Some(Background::Color(match (is_open, status) {
                    (true, _) => Color { r: 0.14, g: 0.14, b: 0.14, a: 1.0 },
                    (_, button::Status::Hovered) => Color { r: 0.26, g: 0.26, b: 0.26, a: 1.0 },
                    _ => Color { r: 0.18, g: 0.18, b: 0.18, a: 1.0 },
                })),
                border: Border {
                    radius: 3.0.into(),
                    width: 1.0,
                    color: Color { r: 0.35, g: 0.35, b: 0.35, a: 1.0 },
                },
                ..Default::default()
            })
            .padding([3, 8])
            .width(Fill);

            // ── Helper: build a row of small tool buttons ─────────────────
            let make_tool_row = |tools: Vec<ToolDef>| -> Element<Message> {
                let btns: Vec<Element<Message>> = tools
                    .into_iter()
                    .map(|t| {
                        let is_active = active_tool.as_deref() == Some(t.id);
                        let tip = t.label;
                        let event = t.event.clone();
                        let icon_el: Element<Message> = match t.icon {
                            IconKind::Glyph(g) => text(g).size(13).color(Color::WHITE).into(),
                            IconKind::Svg(bytes) => iced::widget::svg(
                                iced::widget::svg::Handle::from_memory(bytes),
                            )
                            .width(16)
                            .height(16)
                            .into(),
                        };
                        let msg = module_event_to_message(event);
                        tooltip(
                            button(icon_el)
                                .on_press(msg)
                                .style(move |_: &Theme, status| tool_btn_style(is_active, status))
                                .padding([2, 5]),
                            make_tip(tip.to_string()),
                            TipPos::Bottom,
                        )
                        .gap(4.0)
                        .delay(Duration::from_millis(400))
                        .style(tip_style)
                        .into()
                    })
                    .collect();
                row(btns).spacing(2).align_y(iced::Center).into()
            };

            let tools_row2 = make_tool_row(row2);
            let tools_row3 = make_tool_row(row3);

            container(
                column![combo_btn, tools_row2, tools_row3]
                    .spacing(3)
                    .align_x(iced::Left),
            )
            .width(Length::Fixed(COMBO_W))
            .height(Fill)
            .align_y(iced::Center)
            .padding(Padding { top: 4.0, bottom: 4.0, left: 4.0, right: 4.0 })
            .into()
        }

        RibbonItem::PropertiesGroup { match_prop } => {
            // Left: Match Properties large button.
            let mp_active = is_active_tool(match_prop.id, active_tool, wireframe, ortho_mode);
            let mp_event = match_prop.event.clone();
            let mp_id = match_prop.id.to_string();
            let mp_tip = format!("{}\nCommand: {}", match_prop.label, match_prop.id);
            let mp_btn = button(
                column![
                    make_icon(match_prop.icon, LARGE_ICON),
                    text(match_prop.label).size(10).color(LABEL_COLOR),
                ]
                .align_x(iced::Center)
                .spacing(3),
            )
            .on_press(Message::RibbonToolClick { tool_id: mp_id, event: mp_event })
            .style(move |_: &Theme, status| tool_btn_style(mp_active, status))
            .width(Length::Fixed(LARGE_W))
            .height(Fill)
            .padding(Padding { top: 6.0, right: 4.0, bottom: 4.0, left: 4.0 });
            let mp_el = tooltip(mp_btn, make_tip(mp_tip), TipPos::Bottom)
                .gap(6.0)
                .delay(Duration::from_millis(400))
                .style(tip_style);

            // Right: three property combo rows.
            const PROP_W: f32 = 130.0;

            // Helper: build one property combo row.
            let prop_row = |label: String, dd_id: &'static str, swatch: Option<Color>| {
                let is_open = open_dd.as_deref() == Some(dd_id);
                let swatch_el: Element<'a, Message> = if let Some(c) = swatch {
                    container(text(""))
                        .style(move |_: &Theme| container::Style {
                            background: Some(Background::Color(c)),
                            border: Border {
                                color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.5 },
                                width: 1.0,
                                radius: 1.0.into(),
                            },
                            ..Default::default()
                        })
                        .width(12)
                        .height(12)
                        .into()
                } else {
                    iced::widget::Space::new().width(0).into()
                };
                button(
                    row![
                        swatch_el,
                        container(text(label).size(10).color(Color::WHITE))
                            .width(Fill)
                            .clip(true),
                        text(if is_open { "▲" } else { "▼" })
                            .size(7)
                            .color(Color { r: 0.6, g: 0.6, b: 0.6, a: 1.0 }),
                    ]
                    .spacing(4)
                    .align_y(iced::Center),
                )
                .on_press(Message::ToggleRibbonDropdown(dd_id.to_string()))
                .style(move |_: &Theme, status| button::Style {
                    background: Some(Background::Color(match (is_open, status) {
                        (true, _) => Color { r: 0.14, g: 0.14, b: 0.14, a: 1.0 },
                        (_, button::Status::Hovered) => Color { r: 0.26, g: 0.26, b: 0.26, a: 1.0 },
                        _ => Color { r: 0.18, g: 0.18, b: 0.18, a: 1.0 },
                    })),
                    border: Border {
                        radius: 2.0.into(),
                        width: 1.0,
                        color: if is_open {
                            Color { r: 0.45, g: 0.65, b: 0.90, a: 1.0 }
                        } else {
                            Color { r: 0.35, g: 0.35, b: 0.35, a: 1.0 }
                        },
                    },
                    ..Default::default()
                })
                .padding([3, 8])
                .width(Length::Fixed(PROP_W))
            };

            // Color swatch — use shared acad_color_display helper.
            let (color_swatch, color_label) = acad_color_display(active_color);

            let color_row = prop_row(color_label.to_string(), PROP_COLOR_ID, Some(color_swatch));
            let lt_row    = prop_row(active_linetype.to_string(), PROP_LINETYPE_ID, None);
            let lw_row    = prop_row(LwItem(active_lineweight).to_string(), PROP_LW_ID, None);

            let combos = container(
                column![color_row, lt_row, lw_row].spacing(2).align_x(iced::Left),
            )
            .height(Fill)
            .align_y(iced::Center)
            .padding(Padding { top: 4.0, bottom: 4.0, left: 0.0, right: 4.0 });

            row![mp_el, combos]
                .spacing(4)
                .align_y(iced::Center)
                .height(Fill)
                .into()
        }

        // Small items should never reach render_large — handled upstream.
        _ => text("").into(),
    }
}

// ── Button style ───────────────────────────────────────────────────────────

fn tool_btn_style(is_active: bool, status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(match (is_active, status) {
            (true, _) => TOOL_ACTIVE,
            (_, button::Status::Hovered) => TOOL_HOVER,
            (_, button::Status::Pressed) => TOOL_ACTIVE,
            _ => Color::TRANSPARENT,
        })),
        text_color: Color::WHITE,
        border: Border {
            radius: 3.0.into(),
            color: Color::TRANSPARENT,
            width: 0.0,
        },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

// ── Colors ────────────────────────────────────────────────────────────────

const LOGO_RED: Color = Color {
    r: 0.75,
    g: 0.10,
    b: 0.10,
    a: 1.0,
};
const TOPBAR_BG: Color = Color {
    r: 0.17,
    g: 0.17,
    b: 0.17,
    a: 1.0,
};
const RIBBON_BG: Color = Color {
    r: 0.22,
    g: 0.22,
    b: 0.22,
    a: 1.0,
};
const BORDER_DARK: Color = Color {
    r: 0.12,
    g: 0.12,
    b: 0.12,
    a: 1.0,
};
const ACCENT_BLUE: Color = Color {
    r: 0.20,
    g: 0.55,
    b: 0.90,
    a: 1.0,
};
const ACCENT_GOLD: Color = Color {
    r: 0.90,
    g: 0.65,
    b: 0.10,
    a: 1.0,
}; // contextual tab accent
const LABEL_COLOR: Color = Color {
    r: 0.82,
    g: 0.82,
    b: 0.82,
    a: 1.0,
};
const GROUP_LABEL: Color = Color {
    r: 0.50,
    g: 0.50,
    b: 0.50,
    a: 1.0,
};
const TOOL_HOVER: Color = Color {
    r: 0.32,
    g: 0.32,
    b: 0.32,
    a: 1.0,
};
const TOOL_ACTIVE: Color = Color {
    r: 0.18,
    g: 0.42,
    b: 0.70,
    a: 1.0,
};
const ARROW_COLOR: Color = Color {
    r: 0.65,
    g: 0.65,
    b: 0.65,
    a: 1.0,
};
const PANEL_BG: Color = Color {
    r: 0.16,
    g: 0.16,
    b: 0.16,
    a: 0.98,
};
const PANEL_BORDER: Color = Color {
    r: 0.32,
    g: 0.32,
    b: 0.32,
    a: 1.0,
};
const ROW_HOVER: Color = Color {
    r: 0.24,
    g: 0.24,
    b: 0.24,
    a: 1.0,
};
const CHECK_COLOR: Color = Color {
    r: 0.20,
    g: 0.75,
    b: 0.35,
    a: 1.0,
};
const ICON_COLOR: Color = Color {
    r: 0.25,
    g: 0.75,
    b: 0.45,
    a: 1.0,
};
const LABEL_ON: Color = Color {
    r: 0.92,
    g: 0.92,
    b: 0.92,
    a: 1.0,
};
const LABEL_OFF: Color = Color {
    r: 0.72,
    g: 0.72,
    b: 0.72,
    a: 1.0,
};

// ── Tooltip helpers ────────────────────────────────────────────────────────

/// Build the tooltip label element (multiline text).
fn make_tip(tip: String) -> Element<'static, Message> {
    text(tip).size(11).color(Color::WHITE).into()
}

/// Style the tooltip container — dark panel matching the ribbon's dropdown panels.
fn tip_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color {
            r: 0.13,
            g: 0.13,
            b: 0.13,
            a: 0.97,
        })),
        border: Border {
            color: Color {
                r: 0.35,
                g: 0.35,
                b: 0.35,
                a: 1.0,
            },
            width: 1.0,
            radius: 3.0.into(),
        },
        text_color: Some(Color::WHITE),
        ..Default::default()
    }
}

/// Calculate the left pixel offset of a dropdown button inside the ribbon tool area.
/// Iterates through groups and items, summing widths using the same layout constants
/// used by render_large / render_small, to align the panel with the button's left edge.
fn compute_dropdown_left(groups: &[crate::modules::RibbonGroup], open_id: &str) -> f32 {
    let sum_with_spacing = |widths: &[f32]| -> f32 {
        widths
            .iter()
            .enumerate()
            .map(|(i, &w)| if i == 0 { w } else { 2.0 + w })
            .sum::<f32>()
    };
    let next_item_x = |widths: &[f32]| -> f32 {
        if widths.is_empty() {
            0.0
        } else {
            sum_with_spacing(widths) + 2.0
        }
    };

    let mut x = 0.0f32;

    for (g_idx, group) in groups.iter().enumerate() {
        if g_idx > 0 {
            x += 1.0;
        } // separator between groups
        x += 4.0; // group column padding-left

        let mut row_widths: Vec<f32> = Vec::new();
        let mut small_col_w: f32 = 0.0;
        let mut small_col_n: usize = 0;

        for item in &group.tools {
            let is_large = matches!(
                item,
                RibbonItem::LargeTool(_)
                | RibbonItem::LargeDropdown { .. }
                | RibbonItem::LayerComboGroup { .. }
                | RibbonItem::PropertiesGroup { .. }
            );
            let id: &str = match item {
                RibbonItem::LargeTool(t) => t.id,
                RibbonItem::LargeDropdown { id, .. } => *id,
                RibbonItem::Tool(t) => t.id,
                RibbonItem::Dropdown { id, .. } => *id,
                RibbonItem::LayerComboGroup { .. } => LAYER_COMBO_ID,
                RibbonItem::PropertiesGroup { match_prop } => match_prop.id,
            };
            let item_w = match item {
                RibbonItem::LargeTool(_) | RibbonItem::LargeDropdown { .. } => LARGE_W,
                RibbonItem::LayerComboGroup { .. } => LARGE_W * 2.5,
                RibbonItem::PropertiesGroup { .. } => LARGE_W + 4.0 + 130.0, // match_prop + combos
                RibbonItem::Dropdown { .. } => SMALL_W + ARROW_W,
                _ => SMALL_W,
            };

            if is_large {
                // Flush pending small column
                if small_col_n > 0 {
                    row_widths.push(small_col_w);
                    small_col_w = 0.0;
                    small_col_n = 0;
                }
                if id == open_id {
                    return x + next_item_x(&row_widths);
                }
                row_widths.push(item_w);
            } else {
                // Small items share a column; the column's x = next_item_x before it started
                if id == open_id {
                    return x + next_item_x(&row_widths);
                }
                small_col_w = small_col_w.max(item_w);
                small_col_n += 1;
                if small_col_n == 3 {
                    row_widths.push(small_col_w);
                    small_col_w = 0.0;
                    small_col_n = 0;
                }
            }
        }

        // Not in this group — advance x past the full group width + right padding
        if small_col_n > 0 {
            row_widths.push(small_col_w);
        }
        x += sum_with_spacing(&row_widths) + 4.0;
    }

    60.0 // fallback
}

fn compute_layer_combo_left(groups: &[crate::modules::RibbonGroup]) -> f32 {
    compute_dropdown_left(groups, LAYER_COMBO_ID)
}

/// Left offset for a Properties combo dropdown (color/lt/lw sit after the large Match Prop btn).
fn compute_prop_combo_left(groups: &[crate::modules::RibbonGroup], _dd_id: &str) -> f32 {
    // Find the PropertiesGroup and return its x + match_prop button width + spacing.
    let base = compute_dropdown_left(groups, "MATCHPROP");
    base + LARGE_W + 4.0
}

#[allow(dead_code)]
pub fn module_event_to_message(event: ModuleEvent) -> Message {
    match event {
        ModuleEvent::Command(cmd) => Message::Command(cmd),
        ModuleEvent::OpenFileDialog => Message::OpenFile,
        ModuleEvent::ClearModels => Message::ClearScene,
        ModuleEvent::SetWireframe(w) => Message::SetWireframe(w),
        ModuleEvent::ToggleLayers => Message::ToggleLayers,
    }
}


fn render_history_control<'a>(
    glyph: &'static str,
    label: &'static str,
    dropdown_id: &'static str,
    count: usize,
    open_dropdown: &Option<String>,
) -> Element<'a, Message> {
    let dd_open = open_dropdown.as_deref() == Some(dropdown_id);
    let active = count > 0;

    let main_btn = {
        let btn = button(text(glyph).size(14).color(if active { Color::WHITE } else { LABEL_OFF }))
            .style(move |_: &Theme, status| top_hist_btn_style(active, dd_open, status))
            .width(Length::Fixed(TOP_HIST_W))
            .height(24)
            .padding([2, 0]);
        let btn = if active {
            if dropdown_id == UNDO_HISTORY_ID {
                btn.on_press(Message::Undo)
            } else {
                btn.on_press(Message::Redo)
            }
        } else {
            btn
        };
        tooltip(
            btn,
            make_tip(format!("{label}\n{count} steps available")),
            TipPos::Bottom,
        )
        .gap(6.0)
        .delay(Duration::from_millis(400))
        .style(tip_style)
    };

    let arrow_btn = {
        let btn = button(
            container(text("▾").size(7).color(if active { ARROW_COLOR } else { LABEL_OFF }))
                .width(Fill)
                .height(Fill)
                .align_x(iced::Center)
                .align_y(iced::Center),
        )
        .style(move |_: &Theme, status| top_hist_btn_style(active, dd_open, status))
        .width(Length::Fixed(TOP_ARR_W))
        .height(24)
        .padding(0);
        let btn = if active {
            btn.on_press(Message::ToggleRibbonDropdown(dropdown_id.to_string()))
        } else {
            btn
        };
        tooltip(
            btn,
            make_tip(format!("Choose {label} history")),
            TipPos::Bottom,
        )
        .gap(6.0)
        .delay(Duration::from_millis(400))
        .style(tip_style)
    };

    row![main_btn, arrow_btn].spacing(0).into()
}

fn top_hist_btn_style(active: bool, open: bool, status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(match (active, open, status) {
            (false, _, _) => Color {
                r: 0.20,
                g: 0.20,
                b: 0.20,
                a: 1.0,
            },
            (_, true, _) => TOOL_ACTIVE,
            (_, _, button::Status::Hovered) => TOOL_HOVER,
            (_, _, button::Status::Pressed) => TOOL_ACTIVE,
            _ => Color::TRANSPARENT,
        })),
        text_color: Color::WHITE,
        border: Border {
            radius: 3.0.into(),
            color: Color::TRANSPARENT,
            width: 0.0,
        },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

fn compute_history_dropdown_left(open_id: &str) -> f32 {
    let logo_w = 38.0;
    let leading_gap = 6.0;
    let ctrl_w = TOP_HIST_W + TOP_ARR_W;

    match open_id {
        UNDO_HISTORY_ID => logo_w + leading_gap,
        REDO_HISTORY_ID => logo_w + leading_gap + ctrl_w + TOP_HIST_GAP,
        _ => logo_w + leading_gap,
    }
}
