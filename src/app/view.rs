use super::{H7CAD, Message};
use super::document::DocumentTab;
use super::history::history_dropdown_labels;
use super::helpers::grid_plane_from_camera;
use crate::scene::{VIEWCUBE_DRAW_PX, VIEWCUBE_PAD};
use crate::scene::grip::grips_to_screen;
use crate::ui::overlay;
use iced::widget::{button, column, container, mouse_area, row, shader, stack, text, text_input, Row, Space};
use iced::window;
use iced::{keyboard, Background, Border, Color, Element, Fill, Subscription, Task, Theme};

const VIEWCUBE_HIT_SIZE: f32 = VIEWCUBE_DRAW_PX;

impl H7CAD {
    pub fn view(&self, window_id: window::Id) -> Element<'_, Message> {
        // ── Layer Properties Manager window ───────────────────────────────
        if Some(window_id) == self.layer_window {
            let tab = &self.tabs[self.active_tab];
            return tab.layers.view_window();
        }

        let i = self.active_tab;
        let tab = &self.tabs[i];
        let is_paper = tab.scene.current_layout != "Model";
        let viewport_3d = shader(&tab.scene).width(Fill).height(Fill);

        let selection_overlay = {
            let sel = tab.scene.selection.borrow().clone();
            let snap_info = tab.snap_result.map(|s| (s.screen, s.snap_type));

            let grips: Vec<overlay::GripMarker> =
                if tab.active_cmd.is_none() && !tab.selected_grips.is_empty() {
                    let (vw, vh) = tab.scene.selection.borrow().vp_size;
                    let bounds = iced::Rectangle {
                        x: 0.0, y: 0.0, width: vw, height: vh,
                    };
                    let vp_mat = tab.scene.camera.borrow().view_proj(bounds);
                    let sel_h = tab.selected_handle;
                    grips_to_screen(&tab.selected_grips, vp_mat, bounds)
                        .into_iter()
                        .filter(|(_, screen, _, _)| {
                            screen.x.is_finite()
                                && screen.y.is_finite()
                                && screen.x >= -bounds.width
                                && screen.x <= bounds.width * 2.0
                                && screen.y >= -bounds.height
                                && screen.y <= bounds.height * 2.0
                        })
                        .map(|(grip_id, screen, _is_midpoint, shape)| {
                            let is_hot = tab
                                .active_grip
                                .as_ref()
                                .map_or(false, |g| Some(g.handle) == sel_h && g.grip_id == grip_id);
                            overlay::GripMarker { pos: screen, shape, is_hot }
                        })
                        .collect()
                } else {
                    vec![]
                };

            let (vw, vh) = tab.scene.selection.borrow().vp_size;
            let vp_bounds = iced::Rectangle { x: 0.0, y: 0.0, width: vw, height: vh };

            let grid = if self.show_grid {
                let cam = tab.scene.camera.borrow();
                let plane = grid_plane_from_camera(cam.pitch, cam.yaw);
                Some(overlay::GridParams {
                    view_proj: cam.view_proj(vp_bounds),
                    bounds: vp_bounds,
                    plane,
                })
            } else {
                None
            };

            let ucs_icon = if self.show_ucs_icon && !is_paper {
                let cam = tab.scene.camera.borrow();
                Some(overlay::UcsIconParams {
                    view_proj: cam.view_proj(vp_bounds),
                    bounds: vp_bounds,
                })
            } else {
                None
            };

            overlay::selection_overlay(sel, snap_info, grips, grid, ucs_icon)
        };

        let nav = container(overlay::nav_toolbar())
            .align_right(Fill)
            .align_top(Fill)
            .padding(iced::Padding { top: 148.0, right: 8.0, bottom: 0.0, left: 0.0 });

        let info = container(overlay::info_bar(
            if is_paper { &tab.scene.current_layout } else { "Custom View" },
            &tab.visual_style,
        ))
        .padding([4, 6]);

        let viewport_mouse = mouse_area(container(
            iced::widget::Space::new().width(Fill).height(Fill),
        ))
        .on_move(Message::ViewportMove)
        .on_press(Message::ViewportLeftPress)
        .on_release(Message::ViewportLeftRelease)
        .on_right_press(Message::ViewportRightPress)
        .on_right_release(Message::ViewportRightRelease)
        .on_middle_press(Message::ViewportMiddlePress)
        .on_middle_release(Message::ViewportMiddleRelease)
        .on_scroll(Message::ViewportScroll)
        .on_exit(Message::ViewportExit);

        let cube_click = mouse_area(container(
            iced::widget::Space::new()
                .width(iced::Length::Fixed(VIEWCUBE_HIT_SIZE))
                .height(iced::Length::Fixed(VIEWCUBE_HIT_SIZE)),
        ))
        .on_move(Message::CursorMoved)
        .on_press(Message::ViewportClick);

        let cube_click = container(cube_click)
            .align_right(Fill)
            .align_top(Fill)
            .padding(iced::Padding { top: VIEWCUBE_PAD, right: VIEWCUBE_PAD, bottom: 0.0, left: 0.0 })
            .width(Fill)
            .height(Fill);

        let bg_color = if is_paper {
            Color { r: 0.22, g: 0.24, b: 0.28, a: 1.0 }
        } else {
            Color { r: 0.11, g: 0.11, b: 0.11, a: 1.0 }
        };

        let viewport_stack = stack![
            container(viewport_3d)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(bg_color)),
                    ..Default::default()
                })
                .width(Fill)
                .height(Fill),
            container(info).width(Fill).height(Fill),
            selection_overlay,
            viewport_mouse,
            nav,
            cube_click,
        ]
        .width(Fill)
        .height(Fill);

        let center_stack = iced::widget::stack![
            row![tab.properties.view(), viewport_stack]
                .width(Fill)
                .height(Fill),
        ]
        .width(Fill)
        .height(Fill);

        let tab_bar = doc_tab_bar(&self.tabs, self.active_tab);

        let main_ui = container(
            column![
                self.ribbon.view(
                    is_paper,
                    self.tabs[self.active_tab].history.undo_stack.len(),
                    self.tabs[self.active_tab].history.redo_stack.len(),
                ),
                tab_bar,
                center_stack,
                self.command_line.view(),
                self.status_bar.view(
                    &self.snapper,
                    self.snap_popup_open,
                    self.ortho_mode,
                    self.polar_mode,
                    self.show_grid,
                    tab.scene.layout_names(),
                    tab.scene.current_layout.clone(),
                    self.layout_rename_state.as_ref(),
                    tab.scene.first_viewport_scale(),
                    tab.scene.viewport_count(),
                )
            ]
            .width(Fill)
            .height(Fill),
        )
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color { r: 0.11, g: 0.11, b: 0.11, a: 1.0 })),
            ..Default::default()
        })
        .width(Fill)
        .height(Fill);

        let snap_layer: Element<'_, Message> = if self.snap_popup_open {
            crate::ui::snap_popup::snap_popup_overlay(&self.snapper, 4.0)
        } else {
            iced::widget::Space::new().width(0).height(0).into()
        };

        let dropdown_layer: Element<'_, Message> = self
            .ribbon
            .dropdown_overlay(
                &history_dropdown_labels(&self.tabs[self.active_tab].history.undo_stack),
                &history_dropdown_labels(&self.tabs[self.active_tab].history.redo_stack),
            )
            .unwrap_or_else(|| iced::widget::Space::new().width(0).height(0).into());

        let layout_ctx_layer: Element<'_, Message> =
            if let Some(name) = &self.layout_context_menu {
                layout_context_menu_overlay(name)
            } else {
                iced::widget::Space::new().width(0).height(0).into()
            };

        let page_setup_layer: Element<'_, Message> = if self.page_setup_open {
            page_setup_overlay(
                &self.page_setup_w,
                &self.page_setup_h,
                &self.page_setup_plot_area,
                self.page_setup_center,
                &self.page_setup_offset_x,
                &self.page_setup_offset_y,
                &self.page_setup_rotation,
                &self.page_setup_scale,
            )
        } else {
            iced::widget::Space::new().width(0).height(0).into()
        };

        stack![main_ui, self.app_menu.view(), snap_layer, dropdown_layer, layout_ctx_layer, page_setup_layer].into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        use iced::event;
        iced::Subscription::batch([
            window::frames().map(Message::Tick),
            event::listen_with(|ev, status, win_id| {
                use iced::event::Status;
                match ev {
                    iced::Event::Window(window::Event::Closed) => {
                        Some(Message::OsWindowClosed(win_id))
                    }
                    iced::Event::Window(window::Event::Resized(sz)) => {
                        Some(Message::WindowResized(sz.width as f32, sz.height as f32))
                    }
                    iced::Event::Keyboard(keyboard::Event::KeyPressed {
                        key, modifiers, ..
                    }) => {
                        let ctrl = modifiers.control();
                        let shift = modifiers.shift();
                        match key {
                            keyboard::Key::Named(keyboard::key::Named::Enter)
                            | keyboard::Key::Named(keyboard::key::Named::Space)
                                if status == Status::Ignored =>
                            {
                                Some(Message::CommandFinalize)
                            }
                            keyboard::Key::Named(keyboard::key::Named::Escape) => {
                                Some(Message::CommandEscape)
                            }
                            keyboard::Key::Named(keyboard::key::Named::Delete)
                                if status == Status::Ignored =>
                            {
                                Some(Message::DeleteSelected)
                            }
                            keyboard::Key::Named(keyboard::key::Named::F3) => {
                                Some(Message::ToggleSnapEnabled)
                            }
                            keyboard::Key::Named(keyboard::key::Named::F7) => {
                                Some(Message::ToggleGrid)
                            }
                            keyboard::Key::Named(keyboard::key::Named::F8) => {
                                Some(Message::ToggleOrtho)
                            }
                            keyboard::Key::Named(keyboard::key::Named::F9) => {
                                Some(Message::ToggleGridSnap)
                            }
                            keyboard::Key::Named(keyboard::key::Named::F10) => {
                                Some(Message::TogglePolar)
                            }
                            keyboard::Key::Character(c) if ctrl => match c.as_str() {
                                "n" => Some(Message::ClearScene),
                                "o" => Some(Message::OpenFile),
                                "s" if !shift => Some(Message::SaveFile),
                                "s" if shift => Some(Message::SaveAs),
                                "z" if !shift => Some(Message::Undo),
                                "z" if shift => Some(Message::Redo),
                                "y" => Some(Message::Redo),
                                "c" => Some(Message::Command("COPYCLIP".to_string())),
                                "x" => Some(Message::Command("CUTCLIP".to_string())),
                                "v" => Some(Message::Command("PASTECLIP".to_string())),
                                _ => None,
                            },
                            _ => None,
                        }
                    }
                    _ => None,
                }
            }),
        ])
    }

    pub(super) fn focus_cmd_input(&self) -> Task<Message> {
        iced::widget::operation::focus(iced::widget::Id::new(crate::ui::command_line::CMD_INPUT_ID))
    }

    pub(super) fn blur_cmd_input(&self) -> Task<Message> {
        let op = iced::advanced::widget::operation::focusable::unfocus::<Message>();
        iced::advanced::widget::operate(op)
    }
}

// ── Document tab bar ───────────────────────────────────────────────────────

pub(super) fn doc_tab_bar<'a>(tabs: &'a [DocumentTab], active_tab: usize) -> Element<'a, Message> {
    const BAR_BG: Color = Color { r: 0.13, g: 0.13, b: 0.13, a: 1.0 };
    const TAB_ACTIVE: Color = Color { r: 0.22, g: 0.22, b: 0.22, a: 1.0 };
    const TAB_HOVER: Color = Color { r: 0.18, g: 0.18, b: 0.18, a: 1.0 };
    const TAB_INACTIVE: Color = Color { r: 0.13, g: 0.13, b: 0.13, a: 1.0 };
    const ACCENT: Color = Color { r: 0.20, g: 0.55, b: 0.90, a: 1.0 };
    const TEXT_ACTIVE: Color = Color::WHITE;
    const TEXT_INACTIVE: Color = Color { r: 0.60, g: 0.60, b: 0.60, a: 1.0 };
    const CLOSE_HOVER: Color = Color { r: 0.70, g: 0.22, b: 0.22, a: 1.0 };
    const BORDER_COLOR: Color = Color { r: 0.25, g: 0.25, b: 0.25, a: 1.0 };

    let mut bar = Row::new().spacing(0).align_y(iced::Center);

    for (idx, tab) in tabs.iter().enumerate() {
        let is_active = idx == active_tab;
        let name = tab.tab_display_name();
        let label = if tab.dirty { format!("● {}", name) } else { name };

        let title_btn = button(text(label).size(12))
            .on_press(Message::TabSwitch(idx))
            .padding([5, 12])
            .style(move |_: &Theme, status| button::Style {
                background: Some(Background::Color(match (is_active, status) {
                    (true, _) => TAB_ACTIVE,
                    (false, button::Status::Hovered) => TAB_HOVER,
                    _ => TAB_INACTIVE,
                })),
                text_color: if is_active { TEXT_ACTIVE } else { TEXT_INACTIVE },
                border: Border {
                    color: if is_active { ACCENT } else { Color::TRANSPARENT },
                    width: if is_active { 1.0 } else { 0.0 },
                    radius: 0.0.into(),
                },
                shadow: iced::Shadow::default(),
                snap: false,
            });

        let close_btn = button(text("×").size(11).color(Color { r: 0.55, g: 0.55, b: 0.55, a: 1.0 }))
            .on_press(Message::TabClose(idx))
            .padding([3, 5])
            .style(move |_: &Theme, status| button::Style {
                background: Some(Background::Color(match status {
                    button::Status::Hovered => CLOSE_HOVER,
                    _ => if is_active { TAB_ACTIVE } else { TAB_INACTIVE },
                })),
                border: Border { radius: 3.0.into(), ..Default::default() },
                ..Default::default()
            });

        bar = bar.push(
            container(row![title_btn, close_btn].spacing(0).align_y(iced::Center))
                .style(move |_: &Theme| container::Style {
                    border: Border {
                        color: if is_active { BORDER_COLOR } else { Color::TRANSPARENT },
                        width: if is_active { 1.0 } else { 0.0 },
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                }),
        );
    }

    let new_btn = button(text("+").size(14).color(Color { r: 0.65, g: 0.65, b: 0.65, a: 1.0 }))
        .on_press(Message::TabNew)
        .padding([4, 10])
        .style(|_: &Theme, status| button::Style {
            background: Some(Background::Color(match status {
                button::Status::Hovered => TAB_HOVER,
                _ => Color::TRANSPARENT,
            })),
            border: Border { radius: 0.0.into(), ..Default::default() },
            ..Default::default()
        });

    bar = bar.push(new_btn);
    bar = bar.push(iced::widget::Space::new().width(Fill));

    container(bar)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(BAR_BG)),
            border: Border {
                color: BORDER_COLOR,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .height(30)
        .width(Fill)
        .padding([0, 2])
        .into()
}

// ── Layout context-menu overlay ────────────────────────────────────────────

/// A small right-click context menu rendered above the status bar.
/// The `name` is the layout tab that was right-clicked.
fn layout_context_menu_overlay(name: &str) -> Element<'_, Message> {
    const MENU_BG: Color = Color { r: 0.17, g: 0.17, b: 0.17, a: 1.0 };
    const MENU_BORDER: Color = Color { r: 0.35, g: 0.35, b: 0.35, a: 1.0 };
    const ITEM_HOVER: Color = Color { r: 0.25, g: 0.45, b: 0.70, a: 1.0 };
    const TEXT_COLOR: Color = Color { r: 0.88, g: 0.88, b: 0.88, a: 1.0 };

    let item = |label: &'static str, msg: Message| {
        button(text(label).size(12).color(TEXT_COLOR))
            .on_press(msg)
            .style(|_: &Theme, status| button::Style {
                background: Some(Background::Color(match status {
                    button::Status::Hovered | button::Status::Pressed => ITEM_HOVER,
                    _ => Color::TRANSPARENT,
                })),
                text_color: TEXT_COLOR,
                border: Border::default(),
                shadow: iced::Shadow::default(),
                snap: false,
            })
            .padding([4, 12])
            .width(Fill)
    };

    let rename_name = name.to_string();
    let delete_name = name.to_string();

    let menu = container(
        column![
            item("Rename", Message::LayoutRenameStart(rename_name)),
            item("Delete", Message::LayoutDelete(delete_name)),
        ]
        .spacing(0)
        .width(160),
    )
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(MENU_BG)),
        border: Border {
            color: MENU_BORDER,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    })
    .padding([4, 0]);

    // Click-catcher fills the whole screen to close the menu when clicking outside.
    let catcher = mouse_area(
        container(iced::widget::Space::new().width(Fill).height(Fill))
            .width(Fill)
            .height(Fill),
    )
    .on_press(Message::LayoutContextMenuClose)
    .on_right_press(Message::LayoutContextMenuClose);

    // Position the menu above the status bar at the left.
    let positioned = container(menu)
        .align_bottom(Fill)
        .align_left(Fill)
        .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 30.0, left: 8.0 });

    stack![catcher, positioned].into()
}

// ── Page Setup overlay ──────────────────────────────────────────────────────

/// Modal panel for editing paper width / height of the current layout.
// ── Paper size presets ────────────────────────────────────────────────────

#[allow(dead_code)]
const PAPER_PRESETS: &[(&str, f64, f64)] = &[
    ("A4 Portrait",   210.0, 297.0),
    ("A4 Landscape",  297.0, 210.0),
    ("A3 Portrait",   297.0, 420.0),
    ("A3 Landscape",  420.0, 297.0),
    ("A2 Portrait",   420.0, 594.0),
    ("A2 Landscape",  594.0, 420.0),
    ("A1 Portrait",   594.0, 841.0),
    ("A1 Landscape",  841.0, 594.0),
    ("A0 Portrait",   841.0, 1189.0),
    ("A0 Landscape",  1189.0, 841.0),
    ("Letter Portrait",  215.9, 279.4),
    ("Letter Landscape", 279.4, 215.9),
    ("Custom",           0.0,   0.0),
];

fn page_setup_overlay<'a>(
    w_buf: &'a str,
    h_buf: &'a str,
    plot_area: &'a str,
    center: bool,
    offset_x: &'a str,
    offset_y: &'a str,
    rotation: &'a str,
    scale: &'a str,
) -> Element<'a, Message> {
    const PANEL_BG: Color  = Color { r: 0.15, g: 0.15, b: 0.15, a: 1.0 };
    const BORDER_COL: Color = Color { r: 0.35, g: 0.35, b: 0.35, a: 1.0 };
    const TEXT_COLOR: Color = Color { r: 0.88, g: 0.88, b: 0.88, a: 1.0 };
    const DIM_COLOR: Color  = Color { r: 0.55, g: 0.55, b: 0.55, a: 1.0 };
    const ACCENT: Color     = Color { r: 0.25, g: 0.50, b: 0.85, a: 1.0 };
    const ACTIVE_BG: Color  = Color { r: 0.20, g: 0.40, b: 0.70, a: 1.0 };

    let lbl = |s: &'static str| text(s).size(11).color(DIM_COLOR).width(110);

    let field_style = |_: &Theme, _: text_input::Status| text_input::Style {
        background: Background::Color(Color { r: 0.10, g: 0.10, b: 0.10, a: 1.0 }),
        border: Border { color: BORDER_COL, width: 1.0, radius: 3.0.into() },
        icon: TEXT_COLOR,
        placeholder: Color { r: 0.45, g: 0.45, b: 0.45, a: 1.0 },
        value: TEXT_COLOR,
        selection: ACCENT,
    };

    let btn_style = |accent: bool| {
        move |_: &Theme, status: button::Status| button::Style {
            background: Some(Background::Color(match status {
                button::Status::Hovered | button::Status::Pressed if accent => {
                    Color { r: 0.20, g: 0.42, b: 0.72, a: 1.0 }
                }
                button::Status::Hovered | button::Status::Pressed => {
                    Color { r: 0.28, g: 0.28, b: 0.28, a: 1.0 }
                }
                _ if accent => ACCENT,
                _ => Color { r: 0.22, g: 0.22, b: 0.22, a: 1.0 },
            })),
            text_color: TEXT_COLOR,
            border: Border { color: BORDER_COL, width: 1.0, radius: 4.0.into() },
            shadow: iced::Shadow::default(),
            snap: false,
        }
    };

    let pill_style = |active: bool| {
        move |_: &Theme, status: button::Status| button::Style {
            background: Some(Background::Color(match (active, status) {
                (true,  _) => ACTIVE_BG,
                (false, button::Status::Hovered | button::Status::Pressed) => {
                    Color { r: 0.28, g: 0.28, b: 0.28, a: 1.0 }
                }
                _ => Color { r: 0.20, g: 0.20, b: 0.20, a: 1.0 },
            })),
            text_color: TEXT_COLOR,
            border: Border { color: BORDER_COL, width: 1.0, radius: 3.0.into() },
            shadow: iced::Shadow::default(),
            snap: false,
        }
    };

    let divider = || container(Space::new().width(Fill).height(1))
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(BORDER_COL)),
            ..Default::default()
        })
        .width(Fill)
        .height(1);

    // ── Paper size presets ────────────────────────────────────────────────
    let preset_row1 = row![
        button(text("A4 P").size(10).color(TEXT_COLOR))
            .on_press(Message::PageSetupPreset("A4 Portrait".into()))
            .style(pill_style(false)).padding([3, 6]),
        button(text("A4 L").size(10).color(TEXT_COLOR))
            .on_press(Message::PageSetupPreset("A4 Landscape".into()))
            .style(pill_style(false)).padding([3, 6]),
        button(text("A3 P").size(10).color(TEXT_COLOR))
            .on_press(Message::PageSetupPreset("A3 Portrait".into()))
            .style(pill_style(false)).padding([3, 6]),
        button(text("A3 L").size(10).color(TEXT_COLOR))
            .on_press(Message::PageSetupPreset("A3 Landscape".into()))
            .style(pill_style(false)).padding([3, 6]),
    ].spacing(4);

    let preset_row2 = row![
        button(text("A2 L").size(10).color(TEXT_COLOR))
            .on_press(Message::PageSetupPreset("A2 Landscape".into()))
            .style(pill_style(false)).padding([3, 6]),
        button(text("A1 L").size(10).color(TEXT_COLOR))
            .on_press(Message::PageSetupPreset("A1 Landscape".into()))
            .style(pill_style(false)).padding([3, 6]),
        button(text("A0 L").size(10).color(TEXT_COLOR))
            .on_press(Message::PageSetupPreset("A0 Landscape".into()))
            .style(pill_style(false)).padding([3, 6]),
        button(text("Letter").size(10).color(TEXT_COLOR))
            .on_press(Message::PageSetupPreset("Letter Landscape".into()))
            .style(pill_style(false)).padding([3, 6]),
    ].spacing(4);

    // ── Plot area buttons ─────────────────────────────────────────────────
    let area_row = row![
        button(text("Layout").size(10).color(TEXT_COLOR))
            .on_press(Message::PageSetupPlotArea("Layout".into()))
            .style(pill_style(plot_area == "Layout")).padding([3, 8]),
        button(text("Extents").size(10).color(TEXT_COLOR))
            .on_press(Message::PageSetupPlotArea("Extents".into()))
            .style(pill_style(plot_area == "Extents")).padding([3, 8]),
    ].spacing(6);

    // ── Center toggle ─────────────────────────────────────────────────────
    let center_btn = button(
        text(if center { "✓ Center on page" } else { "  Center on page" }).size(11).color(TEXT_COLOR)
    )
    .on_press(Message::PageSetupCenterToggle)
    .style(pill_style(center))
    .padding([4, 10]);

    // ── Rotation buttons ──────────────────────────────────────────────────
    let rot_row = row![
        button(text("0°").size(10).color(TEXT_COLOR))
            .on_press(Message::PageSetupRotation("0".into()))
            .style(pill_style(rotation == "0")).padding([3, 8]),
        button(text("90°").size(10).color(TEXT_COLOR))
            .on_press(Message::PageSetupRotation("90".into()))
            .style(pill_style(rotation == "90")).padding([3, 8]),
        button(text("180°").size(10).color(TEXT_COLOR))
            .on_press(Message::PageSetupRotation("180".into()))
            .style(pill_style(rotation == "180")).padding([3, 8]),
        button(text("270°").size(10).color(TEXT_COLOR))
            .on_press(Message::PageSetupRotation("270".into()))
            .style(pill_style(rotation == "270")).padding([3, 8]),
    ].spacing(4);

    let panel = container(
        column![
            text("Page Setup").size(14).color(TEXT_COLOR),
            divider(),
            // Paper size
            text("Paper Size").size(11).color(DIM_COLOR),
            preset_row1,
            preset_row2,
            row![
                lbl("Width (mm)"),
                text_input("297", w_buf)
                    .on_input(Message::PageSetupWidthEdit)
                    .on_submit(Message::PageSetupCommit)
                    .style(field_style)
                    .width(80).size(12),
            ].spacing(6).align_y(iced::Alignment::Center),
            row![
                lbl("Height (mm)"),
                text_input("210", h_buf)
                    .on_input(Message::PageSetupHeightEdit)
                    .on_submit(Message::PageSetupCommit)
                    .style(field_style)
                    .width(80).size(12),
            ].spacing(6).align_y(iced::Alignment::Center),
            divider(),
            // Plot area
            text("Plot Area").size(11).color(DIM_COLOR),
            area_row,
            divider(),
            // Position
            text("Position").size(11).color(DIM_COLOR),
            center_btn,
            row![
                lbl("Offset X (mm)"),
                text_input("0", offset_x)
                    .on_input(Message::PageSetupOffsetXEdit)
                    .style(field_style)
                    .width(80).size(12),
            ].spacing(6).align_y(iced::Alignment::Center),
            row![
                lbl("Offset Y (mm)"),
                text_input("0", offset_y)
                    .on_input(Message::PageSetupOffsetYEdit)
                    .style(field_style)
                    .width(80).size(12),
            ].spacing(6).align_y(iced::Alignment::Center),
            divider(),
            // Rotation
            text("Rotation").size(11).color(DIM_COLOR),
            rot_row,
            divider(),
            // Plot Scale
            text("Plot Scale").size(11).color(DIM_COLOR),
            row![
                button(text("Fit").size(10).color(TEXT_COLOR))
                    .on_press(Message::PageSetupScale("Fit".into()))
                    .style(pill_style(scale == "Fit")).padding([3, 8]),
                button(text("1:1").size(10).color(TEXT_COLOR))
                    .on_press(Message::PageSetupScale("1:1".into()))
                    .style(pill_style(scale == "1:1")).padding([3, 8]),
                button(text("1:2").size(10).color(TEXT_COLOR))
                    .on_press(Message::PageSetupScale("1:2".into()))
                    .style(pill_style(scale == "1:2")).padding([3, 8]),
                button(text("1:5").size(10).color(TEXT_COLOR))
                    .on_press(Message::PageSetupScale("1:5".into()))
                    .style(pill_style(scale == "1:5")).padding([3, 8]),
                button(text("1:10").size(10).color(TEXT_COLOR))
                    .on_press(Message::PageSetupScale("1:10".into()))
                    .style(pill_style(scale == "1:10")).padding([3, 8]),
            ].spacing(4),
            row![
                button(text("1:20").size(10).color(TEXT_COLOR))
                    .on_press(Message::PageSetupScale("1:20".into()))
                    .style(pill_style(scale == "1:20")).padding([3, 8]),
                button(text("1:50").size(10).color(TEXT_COLOR))
                    .on_press(Message::PageSetupScale("1:50".into()))
                    .style(pill_style(scale == "1:50")).padding([3, 8]),
                button(text("1:100").size(10).color(TEXT_COLOR))
                    .on_press(Message::PageSetupScale("1:100".into()))
                    .style(pill_style(scale == "1:100")).padding([3, 8]),
                button(text("2:1").size(10).color(TEXT_COLOR))
                    .on_press(Message::PageSetupScale("2:1".into()))
                    .style(pill_style(scale == "2:1")).padding([3, 8]),
            ].spacing(4),
            divider(),
            // Buttons
            row![
                button(text("Cancel").size(12).color(TEXT_COLOR))
                    .on_press(Message::PageSetupClose)
                    .style(btn_style(false))
                    .padding([5, 14]),
                Space::new().width(Fill).height(0),
                button(text("OK").size(12).color(TEXT_COLOR))
                    .on_press(Message::PageSetupCommit)
                    .style(btn_style(true))
                    .padding([5, 20]),
            ]
            .align_y(iced::Alignment::Center),
        ]
        .spacing(8)
        .padding(16)
        .width(290),
    )
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(PANEL_BG)),
        border: Border { color: BORDER_COL, width: 1.0, radius: 6.0.into() },
        ..Default::default()
    });

    // Click-catcher to close on outside click.
    let catcher = mouse_area(
        container(Space::new().width(Fill).height(Fill))
            .width(Fill)
            .height(Fill),
    )
    .on_press(Message::PageSetupClose);

    // Center the panel on screen.
    let positioned = container(panel)
        .width(Fill)
        .height(Fill)
        .align_x(iced::Alignment::Center)
        .align_y(iced::Alignment::Center);

    stack![catcher, positioned].into()
}
