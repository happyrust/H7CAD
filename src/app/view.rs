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

            // OST tracking points → screen positions.
            let ost_points: Vec<overlay::OstTrackPoint> = if self.snapper.otrack_enabled {
                let vp_mat = tab.scene.camera.borrow().view_proj(vp_bounds);
                self.snapper.tracking_points.iter().map(|&wp| {
                    let ndc = vp_mat.project_point3(wp);
                    overlay::OstTrackPoint {
                        screen: iced::Point::new(
                            (ndc.x + 1.0) * 0.5 * vp_bounds.width,
                            (1.0 - ndc.y) * 0.5 * vp_bounds.height,
                        ),
                    }
                }).collect()
            } else {
                vec![]
            };

            overlay::selection_overlay(sel, snap_info, grips, grid, ucs_icon, ost_points, tab.last_cursor_screen)
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
            tab.paper_bg_color
                .map(|[r, g, b, a]| Color { r, g, b, a })
                .unwrap_or(Color { r: 0.22, g: 0.24, b: 0.28, a: 1.0 })
        } else {
            tab.bg_color
                .map(|[r, g, b, a]| Color { r, g, b, a })
                .unwrap_or(Color { r: 0.11, g: 0.11, b: 0.11, a: 1.0 })
        };

        // Dynamic input overlay — shown when a command is active and DYN is on.
        let dyn_input_overlay: Option<Element<'_, Message>> =
            if self.dyn_input && tab.active_cmd.is_some() {
                let w = tab.last_cursor_world;
                let label = if let Some(base) = self.last_point {
                    // Show relative distance + angle when we have a base point.
                    let dx = (w.x - base.x) as f64;
                    let dy = (w.z - base.z) as f64;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let ang = dy.atan2(dx).to_degrees();
                    format!("d={:.3}  <{:.1}°", dist, ang)
                } else {
                    format!("X:{:.3}  Y:{:.3}", w.x, w.z)
                };
                Some(overlay::dynamic_input_overlay(tab.last_cursor_screen, label))
            } else {
                None
            };

        let mut viewport_stack = stack![
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
        if let Some(dyn_ol) = dyn_input_overlay {
            viewport_stack = viewport_stack.push(dyn_ol);
        }

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
                    self.polar_increment_deg,
                    self.show_grid,
                    self.dyn_input,
                    self.snapper.otrack_enabled,
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

        let textstyle_layer: Element<'_, Message> = if self.textstyle_open {
            let tab = &self.tabs[self.active_tab];
            let styles: Vec<String> = tab.scene.document.text_styles
                .iter().map(|s| s.name.clone()).collect();
            textstyle_overlay(styles, &self.textstyle_selected, &self.textstyle_font, &self.textstyle_width, &self.textstyle_oblique)
        } else {
            iced::widget::Space::new().width(0).height(0).into()
        };

        let tablestyle_layer: Element<'_, Message> = if self.tablestyle_open {
            use acadrust::objects::ObjectType;
            let tab = &self.tabs[self.active_tab];
            let styles: Vec<String> = tab.scene.document.objects.values()
                .filter_map(|o| if let ObjectType::TableStyle(s) = o { Some(s.name.clone()) } else { None })
                .collect();
            let selected_style = tab.scene.document.objects.values()
                .find_map(|o| if let ObjectType::TableStyle(s) = o {
                    if s.name == self.tablestyle_selected { Some(s) } else { None }
                } else { None });
            tablestyle_overlay(styles, &self.tablestyle_selected, selected_style)
        } else {
            iced::widget::Space::new().width(0).height(0).into()
        };

        let mlstyle_layer: Element<'_, Message> = if self.mlstyle_open {
            use acadrust::objects::ObjectType;
            let tab = &self.tabs[self.active_tab];
            let styles: Vec<String> = tab.scene.document.objects.values()
                .filter_map(|o| if let ObjectType::MLineStyle(s) = o { Some(s.name.clone()) } else { None })
                .collect();
            let selected_style = tab.scene.document.objects.values()
                .find_map(|o| if let ObjectType::MLineStyle(s) = o {
                    if s.name == self.mlstyle_selected { Some(s) } else { None }
                } else { None });
            mlstyle_overlay(styles, &self.mlstyle_selected, selected_style, tab.scene.document.header.multiline_style.clone())
        } else {
            iced::widget::Space::new().width(0).height(0).into()
        };

        let dimstyle_layer: Element<'_, Message> = if self.dimstyle_open {
            let tab = &self.tabs[self.active_tab];
            let styles: Vec<String> = tab.scene.document.dim_styles
                .iter().map(|s| s.name.clone()).collect();
            dimstyle_overlay(
                styles,
                &self.dimstyle_selected,
                self.dimstyle_tab,
                self,
            )
        } else {
            iced::widget::Space::new().width(0).height(0).into()
        };

        stack![main_ui, self.app_menu.view(), snap_layer, dropdown_layer, layout_ctx_layer, page_setup_layer, textstyle_layer, tablestyle_layer, mlstyle_layer, dimstyle_layer].into()
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
                            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                                Some(Message::CommandHistoryPrev)
                            }
                            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                                Some(Message::CommandHistoryNext)
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
                            keyboard::Key::Named(keyboard::key::Named::F11) => {
                                Some(Message::ToggleOTrack)
                            }
                            keyboard::Key::Named(keyboard::key::Named::F12) => {
                                Some(Message::ToggleDynInput)
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

// ── DimStyle Dialog overlay ─────────────────────────────────────────────────

fn dimstyle_overlay<'a>(
    styles: Vec<String>,
    selected: &'a str,
    tab: u8,
    app: &'a super::H7CAD,
) -> Element<'a, Message> {
    use super::DsField;
    use iced::widget::checkbox;
    use iced::Length::Shrink;

    const PANEL_BG:  Color = Color { r: 0.15, g: 0.15, b: 0.15, a: 1.0 };
    const BORDER:    Color = Color { r: 0.35, g: 0.35, b: 0.35, a: 1.0 };
    const TEXT_COL:  Color = Color { r: 0.88, g: 0.88, b: 0.88, a: 1.0 };
    const DIM_COL:   Color = Color { r: 0.55, g: 0.55, b: 0.55, a: 1.0 };
    const ACCENT:    Color = Color { r: 0.25, g: 0.50, b: 0.85, a: 1.0 };
    const ACTIVE_BG: Color = Color { r: 0.20, g: 0.40, b: 0.70, a: 1.0 };

    let lbl = |s: &'static str| text(s).size(11).color(DIM_COL).width(150);

    let field_style = |_: &Theme, _: text_input::Status| text_input::Style {
        background: Background::Color(Color { r: 0.10, g: 0.10, b: 0.10, a: 1.0 }),
        border: Border { color: BORDER, width: 1.0, radius: 3.0.into() },
        icon: TEXT_COL, placeholder: DIM_COL, value: TEXT_COL, selection: ACCENT,
    };

    let mk_field = |fld: DsField, val: &'a str| -> Element<'a, Message> {
        text_input("", val)
            .on_input(move |s| Message::DsEdit(fld.clone(), s))
            .style(field_style)
            .size(11)
            .width(90)
            .into()
    };

    let btn_style = |accent: bool| move |_: &Theme, status: button::Status| button::Style {
        background: Some(Background::Color(match (accent, status) {
            (true, button::Status::Hovered | button::Status::Pressed) =>
                Color { r: 0.20, g: 0.42, b: 0.72, a: 1.0 },
            (false, button::Status::Hovered | button::Status::Pressed) =>
                Color { r: 0.28, g: 0.28, b: 0.28, a: 1.0 },
            (true, _) => ACCENT,
            _ => Color { r: 0.22, g: 0.22, b: 0.22, a: 1.0 },
        })),
        text_color: TEXT_COL,
        border: Border { color: BORDER, width: 1.0, radius: 4.0.into() },
        ..Default::default()
    };

    let tab_btn = |label: &'static str, idx: u8| {
        let active = tab == idx;
        button(text(label).size(11).color(TEXT_COL))
            .on_press(Message::DimStyleDialogTab(idx))
            .style(move |_: &Theme, st| button::Style {
                background: Some(Background::Color(match (active, st) {
                    (true, _) => ACTIVE_BG,
                    (false, button::Status::Hovered | button::Status::Pressed) =>
                        Color { r: 0.28, g: 0.28, b: 0.28, a: 1.0 },
                    _ => Color { r: 0.20, g: 0.20, b: 0.20, a: 1.0 },
                })),
                text_color: TEXT_COL,
                border: Border { color: BORDER, width: 1.0, radius: 3.0.into() },
                ..Default::default()
            })
            .padding([4, 10])
    };

    // ── Style list ────────────────────────────────────────────────────────
    let style_list: Element<'_, Message> = {
        let mut col = column![].spacing(2);
        for name in styles {
            let active = name == selected;
            col = col.push(
                button(text(name.clone()).size(11).color(TEXT_COL))
                    .on_press(Message::DimStyleDialogSelect(name))
                    .style(move |_: &Theme, st| button::Style {
                        background: Some(Background::Color(match (active, st) {
                            (true, _) => ACTIVE_BG,
                            (false, button::Status::Hovered | button::Status::Pressed) =>
                                Color { r: 0.28, g: 0.28, b: 0.28, a: 1.0 },
                            _ => Color { r: 0.18, g: 0.18, b: 0.18, a: 1.0 },
                        })),
                        text_color: TEXT_COL,
                        border: Border { color: BORDER, width: 0.0, radius: 3.0.into() },
                        ..Default::default()
                    })
                    .padding([3, 8])
                    .width(Fill)
            );
        }
        iced::widget::scrollable(col).height(160).into()
    };

    let style_panel = column![
        text("Styles").size(11).color(DIM_COL),
        container(style_list)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color { r: 0.12, g: 0.12, b: 0.12, a: 1.0 })),
                border: Border { color: BORDER, width: 1.0, radius: 3.0.into() },
                ..Default::default()
            })
            .width(150)
            .padding(2),
        row![
            button(text("New").size(10).color(TEXT_COL))
                .on_press(Message::DimStyleDialogNew)
                .style(btn_style(false)).padding([3, 8]),
            button(text("Delete").size(10).color(TEXT_COL))
                .on_press(Message::DimStyleDialogDelete)
                .style(btn_style(false)).padding([3, 8]),
        ].spacing(4),
        button(text("Set Current").size(10).color(TEXT_COL))
            .on_press(Message::DimStyleDialogSetCurrent)
            .style(btn_style(false)).padding([3, 8]).width(Fill),
    ].spacing(6).width(150);

    // ── Tab bar ───────────────────────────────────────────────────────────
    let tabs = row![
        tab_btn("Lines",       0),
        tab_btn("Arrows",      1),
        tab_btn("Text",        2),
        tab_btn("Scale/Units", 3),
        tab_btn("Tolerances",  4),
    ].spacing(2);

    // ── Checkbox helper ───────────────────────────────────────────────────
    let chk = |label: &'static str, val: bool, fld: DsField| -> Element<'a, Message> {
        checkbox(val)
            .label(label)
            .on_toggle(move |_| Message::DsToggle(fld.clone()))
            .size(14)
            .text_size(11)
            .into()
    };

    // ── Tab content ───────────────────────────────────────────────────────
    let tab_content: Element<'_, Message> = match tab {
        0 => column![
            text("Dimension Line").size(11).color(ACCENT),
            row![lbl("Extension (DIMDLE)"),   mk_field(DsField::Dimdle, &app.ds_dimdle)].spacing(8).align_y(iced::Center),
            row![lbl("Spacing (DIMDLI)"),     mk_field(DsField::Dimdli, &app.ds_dimdli)].spacing(8).align_y(iced::Center),
            row![lbl("Text gap (DIMGAP)"),    mk_field(DsField::Dimgap, &app.ds_dimgap)].spacing(8).align_y(iced::Center),
            chk("Suppress 1st line (DIMSD1)", app.ds_dimsd1, DsField::Dimsd1),
            chk("Suppress 2nd line (DIMSD2)", app.ds_dimsd2, DsField::Dimsd2),
            text("Extension Line").size(11).color(ACCENT),
            row![lbl("Extension (DIMEXE)"),   mk_field(DsField::Dimexe, &app.ds_dimexe)].spacing(8).align_y(iced::Center),
            row![lbl("Offset (DIMEXO)"),      mk_field(DsField::Dimexo, &app.ds_dimexo)].spacing(8).align_y(iced::Center),
            chk("Suppress 1st line (DIMSE1)", app.ds_dimse1, DsField::Dimse1),
            chk("Suppress 2nd line (DIMSE2)", app.ds_dimse2, DsField::Dimse2),
        ].spacing(6).into(),

        1 => column![
            text("Arrows").size(11).color(ACCENT),
            row![lbl("Arrow size (DIMASZ)"),   mk_field(DsField::Dimasz, &app.ds_dimasz)].spacing(8).align_y(iced::Center),
            row![lbl("Center mark (DIMCEN)"),  mk_field(DsField::Dimcen, &app.ds_dimcen)].spacing(8).align_y(iced::Center),
            row![lbl("Tick size (DIMTSZ)"),    mk_field(DsField::Dimtsz, &app.ds_dimtsz)].spacing(8).align_y(iced::Center),
        ].spacing(6).into(),

        2 => column![
            text("Text").size(11).color(ACCENT),
            row![lbl("Height (DIMTXT)"),         mk_field(DsField::Dimtxt,   &app.ds_dimtxt)].spacing(8).align_y(iced::Center),
            row![lbl("Style (DIMTXSTY)"),        mk_field(DsField::Dimtxsty, &app.ds_dimtxsty)].spacing(8).align_y(iced::Center),
            row![lbl("Vertical pos (DIMTAD)"),   mk_field(DsField::Dimtad,   &app.ds_dimtad)].spacing(8).align_y(iced::Center),
            chk("Horizontal inside (DIMTIH)", app.ds_dimtih, DsField::Dimtih),
            chk("Horizontal outside (DIMTOH)", app.ds_dimtoh, DsField::Dimtoh),
        ].spacing(6).into(),

        3 => column![
            text("Scale").size(11).color(ACCENT),
            row![lbl("Overall scale (DIMSCALE)"), mk_field(DsField::Dimscale, &app.ds_dimscale)].spacing(8).align_y(iced::Center),
            row![lbl("Linear factor (DIMLFAC)"),  mk_field(DsField::Dimlfac,  &app.ds_dimlfac)].spacing(8).align_y(iced::Center),
            text("Units").size(11).color(ACCENT),
            row![lbl("Format (DIMLUNIT)"),         mk_field(DsField::Dimlunit, &app.ds_dimlunit)].spacing(8).align_y(iced::Center),
            row![lbl("Decimals (DIMDEC)"),         mk_field(DsField::Dimdec,   &app.ds_dimdec)].spacing(8).align_y(iced::Center),
            row![lbl("Suffix (DIMPOST)"),          mk_field(DsField::Dimpost,  &app.ds_dimpost)].spacing(8).align_y(iced::Center),
        ].spacing(6).into(),

        _ => column![
            text("Tolerances").size(11).color(ACCENT),
            chk("Generate tolerances (DIMTOL)", app.ds_dimtol, DsField::Dimtol),
            chk("Limits generation (DIMLIM)",   app.ds_dimlim, DsField::Dimlim),
            row![lbl("Plus tolerance (DIMTP)"),    mk_field(DsField::Dimtp,   &app.ds_dimtp)].spacing(8).align_y(iced::Center),
            row![lbl("Minus tolerance (DIMTM)"),   mk_field(DsField::Dimtm,   &app.ds_dimtm)].spacing(8).align_y(iced::Center),
            row![lbl("Tol. decimals (DIMTDEC)"),   mk_field(DsField::Dimtdec, &app.ds_dimtdec)].spacing(8).align_y(iced::Center),
            row![lbl("Tol. scale (DIMTFAC)"),      mk_field(DsField::Dimtfac, &app.ds_dimtfac)].spacing(8).align_y(iced::Center),
        ].spacing(6).into(),
    };

    // ── Right panel ───────────────────────────────────────────────────────
    let right_panel = column![
        text(format!("Editing: {selected}")).size(12).color(TEXT_COL),
        tabs,
        container(
            iced::widget::scrollable(
                container(tab_content).padding(8)
            ).height(220)
        )
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color { r: 0.12, g: 0.12, b: 0.12, a: 1.0 })),
            border: Border { color: BORDER, width: 1.0, radius: 3.0.into() },
            ..Default::default()
        })
        .width(Fill),
        row![
            button(text("Apply").size(11).color(TEXT_COL))
                .on_press(Message::DimStyleDialogApply)
                .style(btn_style(true)).padding([5, 16]),
            button(text("Close").size(11).color(TEXT_COL))
                .on_press(Message::DimStyleDialogClose)
                .style(btn_style(false)).padding([5, 12]),
        ].spacing(8),
    ].spacing(8).width(Fill);

    // ── Main panel ────────────────────────────────────────────────────────
    let panel = container(
        column![
            row![
                text("Dimension Style Manager").size(14).color(TEXT_COL),
                Space::new().width(Fill),
                button(text("✕").size(12).color(DIM_COL))
                    .on_press(Message::DimStyleDialogClose)
                    .style(|_: &Theme, _| button::Style {
                        background: Some(Background::Color(Color::TRANSPARENT)),
                        text_color: DIM_COL,
                        ..Default::default()
                    })
                    .padding([2, 6]),
            ].align_y(iced::Center),
            row![style_panel, right_panel].spacing(12).align_y(iced::Top),
        ].spacing(10).padding(16)
    )
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(PANEL_BG)),
        border: Border { color: BORDER, width: 1.0, radius: 6.0.into() },
        ..Default::default()
    })
    .width(Shrink);

    let catcher = mouse_area(
        container(iced::widget::Space::new().width(Fill).height(Fill))
    ).on_press(Message::DimStyleDialogClose);

    let positioned = container(panel)
        .width(Fill).height(Fill)
        .align_x(iced::Alignment::Center)
        .align_y(iced::Alignment::Center);

    stack![catcher, positioned].into()
}

// ── TextStyle Font Browser overlay ─────────────────────────────────────────

/// Built-in CXF font file names (relative to assets/fonts/).
const BUILTIN_FONTS: &[&str] = &[
    "CourierCad.cxf", "Cursive.cxf", "GothGBT.cxf", "GothGRT.cxf", "GothITT.cxf",
    "GreekC.cxf", "GreekS.cxf", "ItalicC.cxf", "ItalicT.cxf",
    "RomanC.cxf", "RomanD.cxf", "RomanS.cxf", "RomanT.cxf",
    "SansND.cxf", "SansNS.cxf", "ScriptC.cxf", "ScriptS.cxf",
    "Standard.cxf", "Unicode.cxf", "SymbolCad.cxf",
];

fn textstyle_overlay<'a>(
    styles: Vec<String>,
    selected: &'a str,
    font_buf: &'a str,
    width_buf: &'a str,
    oblique_buf: &'a str,
) -> Element<'a, Message> {
    use iced::Length::Shrink;

    const PANEL_BG:  Color = Color { r: 0.15, g: 0.15, b: 0.15, a: 1.0 };
    const BORDER:    Color = Color { r: 0.35, g: 0.35, b: 0.35, a: 1.0 };
    const TEXT_COL:  Color = Color { r: 0.88, g: 0.88, b: 0.88, a: 1.0 };
    const DIM_COL:   Color = Color { r: 0.55, g: 0.55, b: 0.55, a: 1.0 };
    const ACCENT:    Color = Color { r: 0.25, g: 0.50, b: 0.85, a: 1.0 };
    const ACTIVE_BG: Color = Color { r: 0.20, g: 0.40, b: 0.70, a: 1.0 };

    let field_style = |_: &Theme, _: iced::widget::text_input::Status| iced::widget::text_input::Style {
        background: Background::Color(Color { r: 0.10, g: 0.10, b: 0.10, a: 1.0 }),
        border: Border { color: BORDER, width: 1.0, radius: 3.0.into() },
        icon: TEXT_COL, placeholder: DIM_COL, value: TEXT_COL, selection: ACCENT,
    };

    let btn_style = |accent: bool| move |_: &Theme, status: button::Status| button::Style {
        background: Some(Background::Color(match (accent, status) {
            (true, button::Status::Hovered | button::Status::Pressed) =>
                Color { r: 0.20, g: 0.42, b: 0.72, a: 1.0 },
            (false, button::Status::Hovered | button::Status::Pressed) =>
                Color { r: 0.28, g: 0.28, b: 0.28, a: 1.0 },
            (true, _) => ACCENT,
            _ => Color { r: 0.22, g: 0.22, b: 0.22, a: 1.0 },
        })),
        text_color: TEXT_COL,
        border: Border { color: BORDER, width: 1.0, radius: 4.0.into() },
        ..Default::default()
    };

    // Left: style list.
    let style_items: Vec<Element<'_, Message>> = styles.iter().map(|name| {
        let is_sel = name.as_str() == selected;
        button(text(name.clone()).size(11).color(TEXT_COL))
            .on_press(Message::TextStyleDialogSelect(name.clone()))
            .style(move |_: &Theme, st| button::Style {
                background: Some(Background::Color(match (is_sel, st) {
                    (true, _) => ACTIVE_BG,
                    (false, button::Status::Hovered | button::Status::Pressed) =>
                        Color { r: 0.26, g: 0.26, b: 0.26, a: 1.0 },
                    _ => Color::TRANSPARENT,
                })),
                text_color: TEXT_COL,
                ..Default::default()
            })
            .padding([3, 8])
            .width(Fill)
            .into()
    }).collect();

    let style_panel = container(
        column(style_items).spacing(2)
    )
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(Color { r: 0.12, g: 0.12, b: 0.12, a: 1.0 })),
        border: Border { color: BORDER, width: 1.0, radius: 4.0.into() },
        ..Default::default()
    })
    .padding(4)
    .width(150)
    .height(280);

    // Middle: font file list (built-in CXF fonts).
    let font_items: Vec<Element<'_, Message>> = BUILTIN_FONTS.iter().map(|&f| {
        let is_sel = font_buf == f;
        button(text(f).size(10).color(TEXT_COL))
            .on_press(Message::TextStyleFontPick(f.to_string()))
            .style(move |_: &Theme, st| button::Style {
                background: Some(Background::Color(match (is_sel, st) {
                    (true, _) => ACTIVE_BG,
                    (false, button::Status::Hovered | button::Status::Pressed) =>
                        Color { r: 0.26, g: 0.26, b: 0.26, a: 1.0 },
                    _ => Color::TRANSPARENT,
                })),
                text_color: TEXT_COL,
                ..Default::default()
            })
            .padding([3, 8])
            .width(Fill)
            .into()
    }).collect();

    let font_panel = column![
        text("Font File:").size(11).color(DIM_COL),
        container(
            iced::widget::scrollable(column(font_items).spacing(1))
        )
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color { r: 0.12, g: 0.12, b: 0.12, a: 1.0 })),
            border: Border { color: BORDER, width: 1.0, radius: 4.0.into() },
            ..Default::default()
        })
        .padding(4)
        .width(180)
        .height(160),
        text_input("font file…", font_buf)
            .on_input(|v| Message::TextStyleEdit { field: "font", value: v })
            .style(field_style)
            .size(11)
            .width(180),
    ]
    .spacing(4);

    // Right: properties + preview.
    let props = column![
        text("Properties").size(12).color(ACCENT),
        row![
            text("Width Factor:").size(11).color(DIM_COL).width(110),
            text_input("1.0", width_buf)
                .on_input(|v| Message::TextStyleEdit { field: "width", value: v })
                .style(field_style)
                .size(11)
                .width(80),
        ].spacing(6).align_y(iced::Center),
        row![
            text("Oblique (°):").size(11).color(DIM_COL).width(110),
            text_input("0.0", oblique_buf)
                .on_input(|v| Message::TextStyleEdit { field: "oblique", value: v })
                .style(field_style)
                .size(11)
                .width(80),
        ].spacing(6).align_y(iced::Center),
        Space::new().height(8),
        text("Preview:").size(11).color(DIM_COL),
        container(
            text("AaBbCc 0123").size(20).color(TEXT_COL)
        )
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color { r: 0.10, g: 0.10, b: 0.10, a: 1.0 })),
            border: Border { color: BORDER, width: 1.0, radius: 4.0.into() },
            ..Default::default()
        })
        .padding(10)
        .width(Fill),
        Space::new().height(Fill),
        row![
            button(text("Apply").size(11))
                .on_press(Message::TextStyleApply)
                .style(btn_style(true))
                .padding([5, 10]),
            button(text("Set Current").size(11))
                .on_press(Message::TextStyleDialogSetCurrent)
                .style(btn_style(false))
                .padding([5, 10]),
        ].spacing(6),
    ]
    .spacing(8)
    .width(220);

    let panel = container(
        column![
            row![
                text("Text Style Font Browser").size(13).color(TEXT_COL),
                Space::new().width(Fill),
                row![
                    button(text("New").size(11))
                        .on_press(Message::TextStyleDialogNew)
                        .style(btn_style(false))
                        .padding([3, 8]),
                    button(text("Delete").size(11))
                        .on_press(Message::TextStyleDialogDelete)
                        .style(btn_style(false))
                        .padding([3, 8]),
                    button(text("✕").size(12).color(DIM_COL))
                        .on_press(Message::TextStyleDialogClose)
                        .style(|_: &Theme, _| button::Style {
                            background: Some(Background::Color(Color::TRANSPARENT)),
                            text_color: DIM_COL,
                            ..Default::default()
                        })
                        .padding([2, 6]),
                ].spacing(4),
            ].align_y(iced::Center),
            row![style_panel, font_panel, props].spacing(12).align_y(iced::Top),
        ].spacing(10).padding(16)
    )
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(PANEL_BG)),
        border: Border { color: BORDER, width: 1.0, radius: 6.0.into() },
        ..Default::default()
    })
    .width(Shrink);

    let catcher = mouse_area(
        container(iced::widget::Space::new().width(Fill).height(Fill))
    ).on_press(Message::TextStyleDialogClose);

    let positioned = container(panel)
        .width(Fill).height(Fill)
        .align_x(iced::Alignment::Center)
        .align_y(iced::Alignment::Center);

    stack![catcher, positioned].into()
}

// ── TableStyle Dialog overlay ───────────────────────────────────────────────

fn tablestyle_overlay<'a>(
    styles: Vec<String>,
    selected: &'a str,
    selected_style: Option<&'a acadrust::objects::TableStyle>,
) -> Element<'a, Message> {
    use iced::Length::Shrink;

    const PANEL_BG:  Color = Color { r: 0.15, g: 0.15, b: 0.15, a: 1.0 };
    const BORDER:    Color = Color { r: 0.35, g: 0.35, b: 0.35, a: 1.0 };
    const TEXT_COL:  Color = Color { r: 0.88, g: 0.88, b: 0.88, a: 1.0 };
    const DIM_COL:   Color = Color { r: 0.55, g: 0.55, b: 0.55, a: 1.0 };
    const ACCENT:    Color = Color { r: 0.25, g: 0.50, b: 0.85, a: 1.0 };
    const ACTIVE_BG: Color = Color { r: 0.20, g: 0.40, b: 0.70, a: 1.0 };

    let btn_style = |accent: bool| move |_: &Theme, status: button::Status| button::Style {
        background: Some(Background::Color(match (accent, status) {
            (true, button::Status::Hovered | button::Status::Pressed) =>
                Color { r: 0.20, g: 0.42, b: 0.72, a: 1.0 },
            (false, button::Status::Hovered | button::Status::Pressed) =>
                Color { r: 0.28, g: 0.28, b: 0.28, a: 1.0 },
            (true, _) => ACCENT,
            _ => Color { r: 0.22, g: 0.22, b: 0.22, a: 1.0 },
        })),
        text_color: TEXT_COL,
        border: Border { color: BORDER, width: 1.0, radius: 4.0.into() },
        ..Default::default()
    };

    // Style list.
    let style_items: Vec<Element<'_, Message>> = styles.iter().map(|name| {
        let is_sel = name.as_str() == selected;
        button(text(name.clone()).size(11).color(TEXT_COL))
            .on_press(Message::TableStyleDialogSelect(name.clone()))
            .style(move |_: &Theme, st| button::Style {
                background: Some(Background::Color(match (is_sel, st) {
                    (true, _) => ACTIVE_BG,
                    (false, button::Status::Hovered | button::Status::Pressed) =>
                        Color { r: 0.26, g: 0.26, b: 0.26, a: 1.0 },
                    _ => Color::TRANSPARENT,
                })),
                text_color: TEXT_COL,
                ..Default::default()
            })
            .padding([3, 8])
            .width(Fill)
            .into()
    }).collect();

    let style_panel = container(
        column(style_items).spacing(2)
    )
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(Color { r: 0.12, g: 0.12, b: 0.12, a: 1.0 })),
        border: Border { color: BORDER, width: 1.0, radius: 4.0.into() },
        ..Default::default()
    })
    .padding(4)
    .width(160)
    .height(240);

    // Right panel: details.
    let details: Element<'_, Message> = if let Some(s) = selected_style {
        let info_row = |label: &'static str, val: String| -> Element<'_, Message> {
            row![
                text(label).size(11).color(DIM_COL).width(140),
                text(val).size(11).color(TEXT_COL),
            ].spacing(8).align_y(iced::Center).into()
        };
        let row_info = |row_label: &'static str, rs: &acadrust::objects::RowCellStyle| -> Element<'_, Message> {
            column![
                text(row_label).size(11).color(ACCENT),
                info_row("  Text Style:", rs.text_style_name.clone()),
                info_row("  Text Height:", format!("{:.4}", rs.text_height)),
                info_row("  Alignment:", format!("{:?}", rs.alignment)),
            ].spacing(3).into()
        };
        column![
            info_row("Name:", s.name.clone()),
            info_row("H Margin:", format!("{:.4}", s.horizontal_margin)),
            info_row("V Margin:", format!("{:.4}", s.vertical_margin)),
            info_row("Title Suppressed:", s.title_suppressed.to_string()),
            info_row("Header Suppressed:", s.header_suppressed.to_string()),
            row_info("Data Row:", &s.data_row_style),
            row_info("Header Row:", &s.header_row_style),
            row_info("Title Row:", &s.title_row_style),
        ].spacing(5).into()
    } else {
        text("No style selected.").size(11).color(DIM_COL).into()
    };

    let right_panel = column![
        details,
        Space::new().height(Fill),
        row![
            button(text("New").size(11))
                .on_press(Message::TableStyleDialogNew)
                .style(btn_style(true))
                .padding([5, 10]),
            button(text("Delete").size(11))
                .on_press(Message::TableStyleDialogDelete)
                .style(btn_style(false))
                .padding([5, 10]),
        ].spacing(6),
    ]
    .spacing(10)
    .width(280)
    .height(240);

    let panel = container(
        column![
            row![
                text("Table Style Manager").size(13).color(TEXT_COL),
                Space::new().width(Fill),
                button(text("✕").size(12).color(DIM_COL))
                    .on_press(Message::TableStyleDialogClose)
                    .style(|_: &Theme, _| button::Style {
                        background: Some(Background::Color(Color::TRANSPARENT)),
                        text_color: DIM_COL,
                        ..Default::default()
                    })
                    .padding([2, 6]),
            ].align_y(iced::Center),
            row![style_panel, right_panel].spacing(12).align_y(iced::Top),
        ].spacing(10).padding(16)
    )
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(PANEL_BG)),
        border: Border { color: BORDER, width: 1.0, radius: 6.0.into() },
        ..Default::default()
    })
    .width(Shrink);

    let catcher = mouse_area(
        container(iced::widget::Space::new().width(Fill).height(Fill))
    ).on_press(Message::TableStyleDialogClose);

    let positioned = container(panel)
        .width(Fill).height(Fill)
        .align_x(iced::Alignment::Center)
        .align_y(iced::Alignment::Center);

    stack![catcher, positioned].into()
}

// ── MLineStyle Dialog overlay ───────────────────────────────────────────────

fn mlstyle_overlay<'a>(
    styles: Vec<String>,
    selected: &'a str,
    selected_style: Option<&'a acadrust::objects::MLineStyle>,
    current_style: String,
) -> Element<'a, Message> {
    use iced::Length::Shrink;

    const PANEL_BG:  Color = Color { r: 0.15, g: 0.15, b: 0.15, a: 1.0 };
    const BORDER:    Color = Color { r: 0.35, g: 0.35, b: 0.35, a: 1.0 };
    const TEXT_COL:  Color = Color { r: 0.88, g: 0.88, b: 0.88, a: 1.0 };
    const DIM_COL:   Color = Color { r: 0.55, g: 0.55, b: 0.55, a: 1.0 };
    const ACCENT:    Color = Color { r: 0.25, g: 0.50, b: 0.85, a: 1.0 };
    const ACTIVE_BG: Color = Color { r: 0.20, g: 0.40, b: 0.70, a: 1.0 };

    let btn_style = |accent: bool| move |_: &Theme, status: button::Status| button::Style {
        background: Some(Background::Color(match (accent, status) {
            (true, button::Status::Hovered | button::Status::Pressed) =>
                Color { r: 0.20, g: 0.42, b: 0.72, a: 1.0 },
            (false, button::Status::Hovered | button::Status::Pressed) =>
                Color { r: 0.28, g: 0.28, b: 0.28, a: 1.0 },
            (true, _) => ACCENT,
            _ => Color { r: 0.22, g: 0.22, b: 0.22, a: 1.0 },
        })),
        text_color: TEXT_COL,
        border: Border { color: BORDER, width: 1.0, radius: 4.0.into() },
        ..Default::default()
    };

    // Style list.
    let style_items: Vec<Element<'_, Message>> = styles.iter().map(|name| {
        let is_sel = name.as_str() == selected;
        let is_cur = *name == current_style;
        let label = if is_cur {
            format!("{} ◀", name)
        } else {
            name.clone()
        };
        button(text(label).size(11).color(TEXT_COL))
            .on_press(Message::MlStyleDialogSelect(name.clone()))
            .style(move |_: &Theme, st| button::Style {
                background: Some(Background::Color(match (is_sel, st) {
                    (true, _) => ACTIVE_BG,
                    (false, button::Status::Hovered | button::Status::Pressed) =>
                        Color { r: 0.26, g: 0.26, b: 0.26, a: 1.0 },
                    _ => Color::TRANSPARENT,
                })),
                text_color: TEXT_COL,
                ..Default::default()
            })
            .padding([3, 8])
            .width(Fill)
            .into()
    }).collect();

    let style_panel = container(
        column(style_items).spacing(2)
    )
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(Color { r: 0.12, g: 0.12, b: 0.12, a: 1.0 })),
        border: Border { color: BORDER, width: 1.0, radius: 4.0.into() },
        ..Default::default()
    })
    .padding(4)
    .width(160)
    .height(240);

    // Right panel: details for selected style.
    let details: Element<'_, Message> = if let Some(s) = selected_style {
        let info_row = |label: &'static str, val: String| -> Element<'_, Message> {
            row![
                text(label).size(11).color(DIM_COL).width(110),
                text(val).size(11).color(TEXT_COL),
            ].spacing(8).align_y(iced::Center).into()
        };
        let elem_rows: Vec<Element<'_, Message>> = s.elements.iter().enumerate().map(|(idx, e)| {
            let color_str: String = match &e.color {
                acadrust::types::Color::ByLayer => "ByLayer".into(),
                acadrust::types::Color::ByBlock => "ByBlock".into(),
                acadrust::types::Color::Index(i) => format!("ACI {}", i),
                acadrust::types::Color::Rgb { r, g, b } => format!("#{:02X}{:02X}{:02X}", r, g, b),
            };
            let lt = if e.linetype.is_empty() { "ByLayer" } else { e.linetype.as_str() };
            row![
                text(format!("  {}:", idx)).size(10).color(DIM_COL).width(20),
                text(format!("{:+.3}", e.offset)).size(10).color(TEXT_COL).width(60),
                text(color_str).size(10).color(TEXT_COL).width(80),
                text(lt).size(10).color(TEXT_COL),
            ].spacing(4).align_y(iced::Center).into()
        }).collect();

        let mut col_items: Vec<Element<'_, Message>> = vec![
            info_row("Name:", s.name.clone()),
            info_row("Elements:", s.elements.len().to_string()),
            text("  Off   Color        Ltype").size(10).color(DIM_COL).into(),
        ];
        col_items.extend(elem_rows);
        column(col_items).spacing(6).into()
    } else {
        text("No style selected.").size(11).color(DIM_COL).into()
    };

    let right_panel = column![
        details,
        Space::new().height(Fill),
        row![
            button(text("Set Current").size(11))
                .on_press(Message::MlStyleDialogSetCurrent)
                .style(btn_style(true))
                .padding([5, 10]),
            button(text("New").size(11))
                .on_press(Message::MlStyleDialogNew)
                .style(btn_style(false))
                .padding([5, 10]),
            button(text("Delete").size(11))
                .on_press(Message::MlStyleDialogDelete)
                .style(btn_style(false))
                .padding([5, 10]),
        ].spacing(6),
    ]
    .spacing(10)
    .width(280)
    .height(240);

    let panel = container(
        column![
            row![
                text("Multiline Style Manager").size(13).color(TEXT_COL),
                Space::new().width(Fill),
                button(text("✕").size(12).color(DIM_COL))
                    .on_press(Message::MlStyleDialogClose)
                    .style(|_: &Theme, _| button::Style {
                        background: Some(Background::Color(Color::TRANSPARENT)),
                        text_color: DIM_COL,
                        ..Default::default()
                    })
                    .padding([2, 6]),
            ].align_y(iced::Center),
            row![style_panel, right_panel].spacing(12).align_y(iced::Top),
        ].spacing(10).padding(16)
    )
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(PANEL_BG)),
        border: Border { color: BORDER, width: 1.0, radius: 6.0.into() },
        ..Default::default()
    })
    .width(Shrink);

    let catcher = mouse_area(
        container(iced::widget::Space::new().width(Fill).height(Fill))
    ).on_press(Message::MlStyleDialogClose);

    let positioned = container(panel)
        .width(Fill).height(Fill)
        .align_x(iced::Alignment::Center)
        .align_y(iced::Alignment::Center);

    stack![catcher, positioned].into()
}
