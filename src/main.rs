mod command;
mod entities;
mod io;
mod linetypes;
mod modules;
mod patterns;
mod scene;
mod snap;
mod ui;

use acadrust::types::{Color as AcadColor, LineWeight};
use acadrust::{CadDocument, EntityType, Handle};
use command::{CadCommand, CmdResult};
use modules::ModuleEvent;
use scene::dispatch;
use scene::grip::{find_hit_grip, grips_to_screen, GripEdit};
use scene::object::GripApply;
use snap::{SnapResult, Snapper};
use std::collections::HashSet;
use std::path::PathBuf;

use scene::{CubeRegion, Scene, VIEWCUBE_DRAW_PX, VIEWCUBE_PAD, VIEWCUBE_PX};
use ui::{overlay, AppMenu, CommandLine, LayerPanel, PropertiesPanel, Ribbon, StatusBar};

use iced::time::Instant;
use iced::widget::{button, column, container, mouse_area, row, shader, stack, text, Row};
use iced::window;
use iced::{mouse, Background, Border, Color, Element, Fill, Point, Subscription, Task, Theme};

const VIEWCUBE_HIT_SIZE: f32 = VIEWCUBE_DRAW_PX;
const POLY_START_DELAY_MS: u128 = 150;
const VARIES_LABEL: &str = "*VARIES*";

/// Parse a typed coordinate string into a world-space Vec3.
/// Accepts "x,y"  → Vec3(x, 0, y)  (XZ drawing plane, Y-up)
///         "x,y,z"→ Vec3(x, y, z)  (full 3D)
/// Separators: comma or semicolon; decimal point or decimal comma.
fn parse_coord(text: &str) -> Option<glam::Vec3> {
    // Split on comma or semicolon, ignore spaces.
    let parts: Vec<f32> = text
        .split(|c| c == ',' || c == ';')
        .map(|s| s.trim().replace(',', "."))
        .filter_map(|s| s.parse().ok())
        .collect();
    match parts.as_slice() {
        [x, y] => Some(glam::Vec3::new(*x, *y, 0.0)),
        [x, y, z] => Some(glam::Vec3::new(*x, *y, *z)),
        _ => None,
    }
}

fn angle_close(a: f32, b: f32, tol: f32) -> bool {
    let diff = (a - b).rem_euclid(std::f32::consts::TAU);
    let diff = if diff > std::f32::consts::PI {
        diff - std::f32::consts::TAU
    } else {
        diff
    };
    diff.abs() < tol
}

fn main() -> iced::Result {
    iced::daemon(H7CAD::boot, H7CAD::update, H7CAD::view)
        .subscription(H7CAD::subscription)
        .title(|state: &H7CAD, window_id: window::Id| {
            if Some(window_id) == state.layer_window {
                "Layer Properties Manager".to_string()
            } else if let Some(tab) = state.tabs.get(state.active_tab) {
                let dot = if tab.dirty { "● " } else { "" };
                let name = tab.tab_display_name();
                format!("{}H7CAD — {}", dot, name)
            } else {
                "H7CAD".to_string()
            }
        })
        .theme(Theme::Dark)
        .run()
}

// ── Per-document tab state ─────────────────────────────────────────────────

struct DocumentTab {
    scene: Scene,
    current_path: Option<PathBuf>,
    dirty: bool,
    tab_title: String,
    properties: PropertiesPanel,
    layers: LayerPanel,
    active_cmd: Option<Box<dyn CadCommand>>,
    last_cmd: Option<String>,
    snap_result: Option<SnapResult>,
    active_grip: Option<GripEdit>,
    selected_grips: Vec<scene::GripDef>,
    selected_handle: Option<Handle>,
    wireframe: bool,
    visual_style: String,
    last_cursor_world: glam::Vec3,
    history: HistoryState,
    active_layer: String,
}

impl DocumentTab {
    fn new_drawing(n: usize) -> Self {
        let mut scene = Scene::new();
        linetypes::populate_document(&mut scene.document);
        Self {
            scene,
            current_path: None,
            dirty: false,
            tab_title: format!("Drawing{}", n),
            properties: PropertiesPanel::empty(),
            layers: LayerPanel::default(),
            active_cmd: None,
            last_cmd: None,
            snap_result: None,
            active_grip: None,
            selected_grips: vec![],
            selected_handle: None,
            wireframe: false,
            visual_style: "Shaded".into(),
            last_cursor_world: glam::Vec3::ZERO,
            history: HistoryState::default(),
            active_layer: "0".to_string(),
        }
    }

    fn tab_display_name(&self) -> String {
        match &self.current_path {
            Some(p) => p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            None => self.tab_title.clone(),
        }
    }
}

#[derive(Clone)]
struct HistorySnapshot {
    document: CadDocument,
    current_layout: String,
    selected: Vec<Handle>,
    dirty: bool,
    label: String,
}

#[derive(Default)]
struct HistoryState {
    undo_stack: Vec<HistorySnapshot>,
    redo_stack: Vec<HistorySnapshot>,
}

// ── Application state ──────────────────────────────────────────────────────

struct H7CAD {
    start: Instant,
    tabs: Vec<DocumentTab>,
    active_tab: usize,
    tab_counter: usize,
    ribbon: Ribbon,
    app_menu: AppMenu,
    command_line: CommandLine,
    status_bar: StatusBar,
    cursor_pos: Point,
    vp_size: (f32, f32),
    snapper: Snapper,
    snap_popup_open: bool,
    /// Whether Tangent snap was enabled before a tangent-pick command started.
    pre_cmd_tangent: Option<bool>,
    /// Orthogonal drawing constraint (F8): constrains picks to 0°/90°/180°/270°.
    ortho_mode: bool,
    /// Polar tracking (F10): constrains picks to 45° angle increments.
    polar_mode: bool,
    /// Show grid lines in the viewport (F7).
    show_grid: bool,
    /// Last point committed by a drawing command — used as ortho/polar base.
    last_point: Option<glam::Vec3>,
    /// OS window Id for the floating Layer Properties Manager (None when closed).
    layer_window: Option<window::Id>,
    /// OS window Id of the primary application window.
    main_window: Option<window::Id>,
    /// In-memory clipboard: cloned entities waiting to be pasted.
    clipboard: Vec<acadrust::EntityType>,
    /// Centroid of the clipboard entities (XZ plane, Y-up).
    clipboard_centroid: glam::Vec3,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick(Instant),
    OpenFile,
    FileOpened(Result<(String, PathBuf, CadDocument), String>),
    SaveFile,
    SaveAs,
    PickedSavePath(Option<PathBuf>),
    ClearScene,
    SetWireframe(bool),
    /// Switch camera projection: true = Orthographic, false = Perspective.
    SetProjection(bool),
    /// Select a ribbon module tab by index.
    RibbonSelectTab(usize),
    /// A ribbon tool button was clicked — highlights the tool and dispatches its event.
    RibbonToolClick {
        tool_id: String,
        event: ModuleEvent,
    },
    // ── Application menu ──────────────────────────────────────────────────
    ToggleAppMenu,
    CloseAppMenu,
    /// Close the menu and immediately dispatch a CAD command.
    CloseAppMenuAndRun(String),
    AppMenuSearch(String),
    // ── Document tabs ──────────────────────────────────────────────────────
    /// Create a new empty document tab.
    TabNew,
    /// Switch to the given tab index.
    TabSwitch(usize),
    /// Close the given tab index.
    TabClose(usize),
    // ─────────────────────────────────────────────────────────────────────
    CommandInput(String),
    CommandSubmit,
    Command(String),
    ToggleLayers,
    /// Emitted when the main application window has been opened (daemon boot).
    MainWindowOpened(window::Id),
    /// Emitted when the layer window has been successfully opened.
    LayerWindowOpened(window::Id),
    LayerToggleVisible(usize),
    LayerToggleLock(usize),
    LayerToggleFreeze(usize),
    LayerNew,
    LayerDelete,
    LayerSetCurrent,
    LayerSelect(usize),
    LayerRenameStart(usize),
    LayerRenameEdit(String),
    LayerColorPickerToggle(usize),
    LayerColorMorePalette,
    LayerColorSet(u8),
    LayerLinetypeSet(String),
    LayerLineweightSet(LineWeight),
    LayerTransparencyEdit(usize, String),
    LayerRenameCommit,
    CursorMoved(Point),
    ViewportClick,
    ViewportMove(Point),
    ViewportLeftPress,
    ViewportLeftRelease,
    ViewportRightPress,
    ViewportRightRelease,
    ViewportMiddlePress,
    ViewportMiddleRelease,
    ViewportScroll(mouse::ScrollDelta),
    ViewportExit,
    ViewCubeSnap(CubeRegion),
    WindowResized(f32, f32),
    /// Enter pressed globally — finalises the active command (no text-input involvement).
    CommandFinalize,
    /// Escape pressed globally — cancels the active command.
    CommandEscape,
    /// Toggle the global snap on/off (OSNAP button body click).
    ToggleSnapEnabled,
    /// Toggle grid-snap on/off — F9 / SNAP status-bar button.
    ToggleGridSnap,
    /// Toggle grid display in the viewport — F7 / GRID status-bar button.
    ToggleGrid,
    /// Toggle orthogonal drawing constraint — F8 / ORTHO status-bar button.
    ToggleOrtho,
    /// Toggle polar-angle constraint (45° increments) — F10 / POLAR status-bar button.
    TogglePolar,
    /// Toggle an individual snap mode (from popup row click).
    ToggleSnap(snap::SnapType),
    /// Open / close the OSNAP popup (▾ arrow click).
    ToggleSnapPopup,
    /// Close the OSNAP popup (click-catcher outside the panel).
    CloseSnapPopup,
    /// Enable all snap modes.
    SnapSelectAll,
    /// Disable all snap modes.
    SnapClearAll,
    /// Toggle a ribbon dropdown open/closed.
    ToggleRibbonDropdown(String),
    /// Close any open ribbon dropdown (click-catcher outside the panel).
    CloseRibbonDropdown,
    /// User selected a specific item from a ribbon dropdown.
    DropdownSelectItem {
        dropdown_id: &'static str,
        cmd: &'static str,
    },
    /// Delete key — erase all currently selected entities.
    DeleteSelected,
    Undo,
    Redo,
    UndoMany(usize),
    RedoMany(usize),
    // ── Ribbon ────────────────────────────────────────────────────────────
    /// User selected a layer from the layer combobox in the ribbon.
    RibbonLayerChanged(String),
    /// User changed the active color in the Properties toolbar.
    RibbonColorChanged(AcadColor),
    /// Toggle the full ACI palette inside the ribbon color picker.
    RibbonColorPaletteToggle,
    /// User changed the active linetype in the Properties toolbar.
    RibbonLinetypeChanged(String),
    /// User changed the active lineweight in the Properties toolbar.
    RibbonLineweightChanged(LineWeight),

    // ── Properties panel ──────────────────────────────────────────────────
    /// User selected a layer from the layer pick_list in the Properties panel.
    PropLayerChanged(String),
    PropSelectionGroupChanged(ui::properties::SelectionGroup),
    /// User picked a color from the Properties color picker.
    PropColorChanged(AcadColor),
    /// User selected a lineweight from the Properties pick_list.
    PropLwChanged(LineWeight),
    /// User selected a linetype from the linetype pick_list.
    PropLinetypeChanged(String),
    /// User toggled a boolean property (e.g. Invisible).
    PropBoolToggle(&'static str),
    /// User selected a hatch pattern from the pattern pick_list in Properties.
    PropHatchPatternChanged(String),
    /// User selected a generic choice field in the Properties panel.
    PropGeomChoiceChanged {
        field: &'static str,
        value: String,
    },
    /// User is typing in an editable geometry field (live buffer update).
    PropGeomInput {
        field: &'static str,
        value: String,
    },
    /// User committed a geometry/common field edit (Enter pressed).
    PropGeomCommit(&'static str),
    /// Toggle the inline color picker dropdown open/closed.
    PropColorPickerToggle,
    /// Toggle the full ACI colour palette expansion.
    PropColorPaletteToggle,
    /// Switch to a named layout ("Model" or paper space layout name).
    LayoutSwitch(String),
    /// Create a new paper space layout.
    LayoutCreate,
    /// A window was closed by the OS (e.g. the user clicked the title-bar ✕).
    OsWindowClosed(window::Id),
    /// No-op — used as a fallback when a TabEvent has no host mapping.
    Noop,
}

impl H7CAD {
    fn new() -> Self {
        let first_tab = DocumentTab::new_drawing(1);
        let mut app = Self {
            start: Instant::now(),
            tabs: vec![first_tab],
            active_tab: 0,
            tab_counter: 1,
            ribbon: Ribbon::new(),
            app_menu: AppMenu::new(),
            command_line: CommandLine::new(),
            status_bar: StatusBar::new(),
            cursor_pos: Point::ORIGIN,
            vp_size: (1280.0, 720.0),
            snapper: Snapper::default(),
            snap_popup_open: false,
            pre_cmd_tangent: None,
            ortho_mode: false,
            polar_mode: false,
            show_grid: false,
            last_point: None,
            layer_window: None,
            main_window: None,
            clipboard: Vec::new(),
            clipboard_centroid: glam::Vec3::ZERO,
        };
        app.sync_ribbon_layers();
        app
    }

    /// Boot function for `iced::daemon`: returns initial state plus a task that
    /// opens the primary application window.
    fn boot() -> (Self, Task<Message>) {
        let state = Self::new();
        let (id, open_task) = window::open(window::Settings {
            maximized: true,
            icon: window::icon::from_rgba(build_window_icon(), 32, 32).ok(),
            ..Default::default()
        });
        let mut s = state;
        s.main_window = Some(id);
        let task = open_task.map(|_| Message::Noop);
        (s, task)
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Tick(t) => {
                self.tabs[self.active_tab].scene.update(t - self.start);
                Task::none()
            }

            Message::OpenFile => Task::perform(io::pick_and_open(), Message::FileOpened),

            Message::FileOpened(Ok((name, path, doc))) => {
                let entity_count = doc.entities().count();
                self.command_line
                    .push_output(&format!("Opened \"{name}\" — {entity_count} entities"));
                self.app_menu.push_recent(path.clone());

                // Eğer aktif sekme boşsa (kaydedilmemiş, yeni çizim, entity yok) dosyayı oraya yükle.
                // Aksi hâlde yeni bir sekme aç.
                let current_is_empty = {
                    let t = &self.tabs[self.active_tab];
                    t.current_path.is_none()
                        && !t.dirty
                        && self.tabs[self.active_tab].scene.document.entities().count() == 0
                };
                let i = if current_is_empty {
                    self.active_tab
                } else {
                    self.tab_counter += 1;
                    let new_tab = DocumentTab::new_drawing(self.tab_counter);
                    self.tabs.push(new_tab);
                    let idx = self.tabs.len() - 1;
                    self.active_tab = idx;
                    idx
                };

                self.tabs[i].current_path = Some(path);
                self.tabs[i].scene.document = doc;
                self.tabs[i].scene.populate_hatches_from_document();
                self.tabs[i].scene.selected = std::collections::HashSet::new();
                self.tabs[i].scene.preview_wires = vec![];
                self.tabs[i].scene.current_layout = "Model".to_string();
                // Merge standard linetypes not already present in the file.
                linetypes::populate_document(&mut self.tabs[i].scene.document);
                self.tabs[i].properties = PropertiesPanel::empty();
                let doc_layers = self.tabs[i].scene.document.layers.clone();
                self.tabs[i].layers.sync_from_doc(&doc_layers);
                self.sync_ribbon_layers();
                self.tabs[i].scene.fit_all();
                self.tabs[i].dirty = false;
                self.tabs[i].history = HistoryState::default();
                self.refresh_selected_grips();
                Task::none()
            }

            Message::FileOpened(Err(e)) => {
                if e != "Cancelled" {
                    self.command_line.push_error(&format!("Open failed: {e}"));
                }
                Task::none()
            }

            Message::SaveFile => {
                let i = self.active_tab;
                if let Some(path) = &self.tabs[i].current_path {
                    let path = path.clone();
                    match io::save(&self.tabs[i].scene.document, &path) {
                        Ok(()) => {
                            self.command_line
                                .push_output(&format!("Saved: {}", path.display()));
                            self.tabs[i].dirty = false;
                        }
                        Err(e) => self.command_line.push_error(&format!("Save failed: {e}")),
                    }
                } else {
                    return Task::perform(io::pick_save_path(), Message::PickedSavePath);
                }
                Task::none()
            }

            Message::SaveAs => Task::perform(io::pick_save_path(), Message::PickedSavePath),

            Message::PickedSavePath(Some(path)) => {
                let i = self.active_tab;
                match io::save(&self.tabs[i].scene.document, &path) {
                    Ok(()) => {
                        self.command_line
                            .push_output(&format!("Saved: {}", path.display()));
                        self.tabs[i].current_path = Some(path);
                        self.tabs[i].dirty = false;
                    }
                    Err(e) => self.command_line.push_error(&format!("Save failed: {e}")),
                }
                Task::none()
            }

            Message::PickedSavePath(None) => Task::none(),

            Message::ClearScene => {
                let i = self.active_tab;
                self.push_undo_snapshot(i, "NEW");
                self.tabs[i].scene.clear();
                linetypes::populate_document(&mut self.tabs[i].scene.document);
                self.tabs[i].properties = PropertiesPanel::empty();
                let doc_layers = self.tabs[i].scene.document.layers.clone();
                self.tabs[i].layers.sync_from_doc(&doc_layers);
                self.command_line
                    .push_output("Scene cleared. Standard linetypes loaded.");
                self.tabs[i].current_path = None;
                self.tabs[i].dirty = true;
                self.sync_ribbon_layers();
                Task::none()
            }

            Message::SetWireframe(w) => {
                let i = self.active_tab;
                self.tabs[i].wireframe = w;
                self.ribbon.set_wireframe(w);
                self.tabs[i].visual_style = if w {
                    "Wireframe".into()
                } else {
                    "Shaded".into()
                };
                self.command_line.push_output(if w {
                    "Visual style: Wireframe"
                } else {
                    "Visual style: Shaded"
                });
                Task::none()
            }

            Message::SetProjection(ortho) => {
                use crate::scene::Projection;
                let proj = if ortho {
                    Projection::Orthographic
                } else {
                    Projection::Perspective
                };
                let i = self.active_tab;
                self.tabs[i].scene.camera.borrow_mut().projection = proj;
                self.tabs[i].scene.camera_generation += 1;
                self.ribbon.set_ortho(ortho);
                self.command_line.push_output(if ortho {
                    "Projection: Orthographic"
                } else {
                    "Projection: Perspective"
                });
                Task::none()
            }

            Message::RibbonSelectTab(idx) => {
                self.ribbon.select(idx);
                Task::none()
            }

            Message::RibbonToolClick { tool_id, event } => {
                self.ribbon.activate_tool(&tool_id);
                match event {
                    ModuleEvent::Command(cmd) => return self.dispatch_command(&cmd),
                    ModuleEvent::OpenFileDialog => {
                        self.command_line
                            .push_info("Open DWG/DXF: not yet implemented.");
                    }
                    ModuleEvent::ClearModels => {
                        let i = self.active_tab;
                        self.tabs[i].scene.clear();
                        self.tabs[i].properties = PropertiesPanel::empty();
                        self.command_line.push_output("Scene cleared.");
                    }
                    ModuleEvent::SetWireframe(w) => {
                        let i = self.active_tab;
                        self.tabs[i].wireframe = w;
                        self.ribbon.set_wireframe(w);
                        self.tabs[i].visual_style = if w {
                            "Wireframe".into()
                        } else {
                            "Shaded".into()
                        };
                        self.command_line.push_output(if w {
                            "Visual style: Wireframe"
                        } else {
                            "Visual style: Shaded"
                        });
                    }
                    ModuleEvent::ToggleLayers => {
                        return Task::done(Message::ToggleLayers);
                    }
                }
                Task::none()
            }

            // ── Application menu ──────────────────────────────────────────
            Message::ToggleAppMenu => {
                self.app_menu.toggle();
                Task::none()
            }

            Message::CloseAppMenu => {
                self.app_menu.close();
                Task::none()
            }

            Message::CloseAppMenuAndRun(cmd) => {
                self.app_menu.close();
                self.dispatch_command(&cmd.clone())
            }

            Message::AppMenuSearch(s) => {
                self.app_menu.search = s;
                Task::none()
            }

            // ── Document tabs ─────────────────────────────────────────────
            Message::TabNew => {
                self.tab_counter += 1;
                let new_tab = DocumentTab::new_drawing(self.tab_counter);
                self.tabs.push(new_tab);
                self.active_tab = self.tabs.len() - 1;
                self.sync_ribbon_layers();
                Task::none()
            }

            Message::TabSwitch(idx) => {
                if idx < self.tabs.len() {
                    self.active_tab = idx;
                    self.sync_ribbon_layers();
                }
                Task::none()
            }

            Message::TabClose(idx) => {
                if self.tabs.len() == 1 {
                    // Only one tab — reset it to a fresh drawing instead of closing
                    self.tab_counter += 1;
                    self.tabs[0] = DocumentTab::new_drawing(self.tab_counter);
                    self.active_tab = 0;
                } else {
                    self.tabs.remove(idx);
                    if self.active_tab >= self.tabs.len() {
                        self.active_tab = self.tabs.len() - 1;
                    }
                }
                Task::none()
            }
            // ─────────────────────────────────────────────────────────────
            Message::CommandInput(s) => {
                self.command_line.input = s;
                Task::none()
            }
            Message::CommandSubmit => {
                let i = self.active_tab;
                if self.tabs[i].active_cmd.is_some() {
                    let text = self.command_line.input.trim().to_string();
                    self.command_line.input.clear();

                    // 1. Command explicitly owns text input (e.g. TTR radius prompt).
                    if self.tabs[i]
                        .active_cmd
                        .as_ref()
                        .map(|c| c.wants_text_input())
                        .unwrap_or(false)
                    {
                        if let Some(result) = self.tabs[i]
                            .active_cmd
                            .as_mut()
                            .and_then(|c| c.on_text_input(&text))
                        {
                            return self.apply_cmd_result(result);
                        }
                        // Step accepted but no final result yet — show updated prompt and refresh preview.
                        let prompt = self.tabs[i].active_cmd.as_ref().map(|c| c.prompt());
                        if let Some(p) = prompt {
                            self.command_line.push_info(&p);
                        }
                        let pt = self.tabs[i].last_cursor_world;
                        let previews = self.tabs[i]
                            .active_cmd
                            .as_mut()
                            .map(|c| c.on_preview_wires(pt))
                            .unwrap_or_default();
                        self.tabs[i].scene.set_preview_wires(previews);
                        return self.focus_cmd_input();
                    }

                    // 2. Empty input → Enter (finish / cancel).
                    if text.is_empty() {
                        let result = self.tabs[i].active_cmd.as_mut().map(|c| c.on_enter());
                        if let Some(r) = result {
                            return self.apply_cmd_result(r);
                        }
                        return Task::none();
                    }

                    // 3. Coordinate input "x,y" or "x,y,z" → on_point.
                    if let Some(pt) = parse_coord(&text) {
                        let result = self.tabs[i].active_cmd.as_mut().map(|c| c.on_point(pt));
                        if let Some(r) = result {
                            return self.apply_cmd_result(r);
                        }
                        return Task::none();
                    }

                    // 4. Numeric input (e.g. radius, angle, scale factor) → on_text_input.
                    if let Some(result) = self.tabs[i]
                        .active_cmd
                        .as_mut()
                        .and_then(|c| c.on_text_input(&text))
                    {
                        return self.apply_cmd_result(result);
                    }

                    // 5. Nothing matched — show hint.
                    self.command_line.push_error(&format!(
                        "Expected coordinates (x,y) or a number, got: \"{text}\""
                    ));
                    return self.focus_cmd_input();
                }
                if let Some(cmd) = self.command_line.submit() {
                    return self.dispatch_command(&cmd);
                }
                Task::none()
            }

            Message::CommandFinalize => {
                let i = self.active_tab;
                if self.tabs[i].active_cmd.is_some() {
                    let result = self.tabs[i].active_cmd.as_mut().map(|c| c.on_enter());
                    if let Some(r) = result {
                        return self.apply_cmd_result(r);
                    }
                    Task::none()
                } else if let Some(cmd) = self.tabs[i].last_cmd.clone() {
                    self.dispatch_command(&cmd)
                } else {
                    Task::none()
                }
            }

            Message::CommandEscape => {
                let i = self.active_tab;
                if self.tabs[i].active_cmd.is_some() {
                    let result = self.tabs[i].active_cmd.as_mut().map(|c| c.on_escape());
                    if let Some(r) = result {
                        return self.apply_cmd_result(r);
                    }
                } else {
                    // No active command → Escape deselects everything.
                    self.tabs[i].scene.deselect_all();
                    self.refresh_properties();
                    // Also cancel any in-progress selection box.
                    let mut sel = self.tabs[i].scene.selection.borrow_mut();
                    sel.box_anchor = None;
                    sel.box_current = None;
                    sel.box_crossing = false;
                }
                Task::none()
            }
            Message::Command(cmd) => self.dispatch_command(&cmd),

            Message::ToggleLayers => {
                if let Some(id) = self.layer_window.take() {
                    // Window already open — close it.
                    window::close(id)
                } else {
                    // Sync layer/linetype data before opening so the panel is up to date.
                    self.sync_ribbon_layers();
                    // Open a new OS window for the Layer Properties Manager.
                    let (id, task) = window::open(window::Settings {
                        size: iced::Size::new(900.0, 360.0),
                        resizable: true,
                        ..Default::default()
                    });
                    self.layer_window = Some(id);
                    task.map(|_| Message::Noop)
                }
            }

            Message::MainWindowOpened(id) => {
                self.main_window = Some(id);
                Task::none()
            }

            Message::LayerWindowOpened(_id) => Task::none(),

            Message::OsWindowClosed(id) => {
                if self.main_window == Some(id) {
                    // Main window closed — exit the process entirely.
                    return iced::exit();
                }
                if self.layer_window == Some(id) {
                    self.layer_window = None;
                }
                Task::none()
            }

            Message::LayerToggleVisible(idx) => {
                let i = self.active_tab;
                if idx < self.tabs[i].layers.layers.len() {
                    self.push_undo_snapshot(i, "LAYER OFF/ON");
                    let l = &mut self.tabs[i].layers.layers[idx];
                    l.visible = !l.visible;
                    let name = l.name.clone();
                    let on = l.visible;
                    self.tabs[i].scene.toggle_layer_visibility(&name);
                    self.command_line.push_output(&format!(
                        "Layer \"{}\" {}",
                        name,
                        if on { "on" } else { "off" }
                    ));
                }
                Task::none()
            }

            Message::LayerToggleLock(idx) => {
                let i = self.active_tab;
                if idx < self.tabs[i].layers.layers.len() {
                    self.push_undo_snapshot(i, "LAYER LOCK/UNLOCK");
                    let l = &mut self.tabs[i].layers.layers[idx];
                    l.locked = !l.locked;
                    let name = l.name.clone();
                    let locked = l.locked;
                    self.tabs[i].scene.toggle_layer_lock(&name);
                    self.command_line.push_output(&format!(
                        "Layer \"{}\" {}",
                        name,
                        if locked { "locked" } else { "unlocked" }
                    ));
                }
                Task::none()
            }

            Message::LayerToggleFreeze(idx) => {
                let i = self.active_tab;
                if idx < self.tabs[i].layers.layers.len() {
                    self.push_undo_snapshot(i, "LAYER FREEZE");
                    let l = &mut self.tabs[i].layers.layers[idx];
                    l.frozen = !l.frozen;
                    let name = l.name.clone();
                    let frozen = l.frozen;
                    if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(&name) {
                        if frozen { dl.freeze(); } else { dl.thaw(); }
                    }
                    self.tabs[i].dirty = true;
                }
                Task::none()
            }

            Message::LayerNew => {
                let i = self.active_tab;
                // Find unique name
                let mut n = 1;
                let new_name = loop {
                    let candidate = format!("Layer{}", n);
                    if !self.tabs[i].scene.document.layers.contains(&candidate) {
                        break candidate;
                    }
                    n += 1;
                };
                self.push_undo_snapshot(i, "LAYER NEW");
                use acadrust::tables::layer::Layer as DocLayer;
                let _ = self.tabs[i].scene.document.layers.add(DocLayer::new(&new_name));
                self.tabs[i].dirty = true;
                let doc_layers = self.tabs[i].scene.document.layers.clone();
                self.tabs[i].layers.sync_from_doc(&doc_layers);
                // Select and start editing the new layer
                let new_idx = self.tabs[i].layers.layers.iter()
                    .position(|l| l.name == new_name);
                if let Some(idx) = new_idx {
                    self.tabs[i].layers.selected = Some(idx);
                    self.tabs[i].layers.editing = Some(idx);
                    self.tabs[i].layers.edit_buf = new_name.clone();
                }
                self.sync_ribbon_layers();
                Task::none()
            }

            Message::LayerDelete => {
                let i = self.active_tab;
                if let Some(idx) = self.tabs[i].layers.selected {
                    let name = self.tabs[i].layers.layers.get(idx)
                        .map(|l| l.name.clone())
                        .unwrap_or_default();
                    if name == "0" { return Task::none(); }
                    self.push_undo_snapshot(i, "LAYER DELETE");
                    self.tabs[i].scene.document.layers.remove(&name);
                    self.tabs[i].dirty = true;
                    let doc_layers = self.tabs[i].scene.document.layers.clone();
                    self.tabs[i].layers.sync_from_doc(&doc_layers);
                    self.tabs[i].layers.selected = None;
                    self.sync_ribbon_layers();
                }
                Task::none()
            }

            Message::LayerSetCurrent => {
                let i = self.active_tab;
                if let Some(idx) = self.tabs[i].layers.selected {
                    if let Some(layer) = self.tabs[i].layers.layers.get(idx) {
                        let name = layer.name.clone();
                        self.tabs[i].active_layer = name.clone();
                        self.tabs[i].layers.current_layer = name.clone();
                        self.ribbon.active_layer = name;
                    }
                }
                Task::none()
            }

            Message::LayerSelect(idx) => {
                let i = self.active_tab;
                // Commit any pending rename first
                if self.tabs[i].layers.editing.is_some() {
                    return Task::done(Message::LayerRenameCommit);
                }
                self.tabs[i].layers.selected = Some(idx);
                Task::none()
            }

            Message::LayerRenameStart(idx) => {
                let i = self.active_tab;
                self.tabs[i].layers.selected = Some(idx);
                if let Some(layer) = self.tabs[i].layers.layers.get(idx) {
                    self.tabs[i].layers.edit_buf = layer.name.clone();
                }
                self.tabs[i].layers.editing = Some(idx);
                Task::none()
            }

            Message::LayerRenameEdit(s) => {
                let i = self.active_tab;
                self.tabs[i].layers.edit_buf = s;
                Task::none()
            }

            Message::LayerRenameCommit => {
                let i = self.active_tab;
                let editing_idx = self.tabs[i].layers.editing.take();
                if let Some(idx) = editing_idx {
                    let new_name = self.tabs[i].layers.edit_buf.trim().to_string();
                    let old_name = self.tabs[i].layers.layers.get(idx)
                        .map(|l| l.name.clone())
                        .unwrap_or_default();
                    if !new_name.is_empty() && new_name != old_name
                        && !self.tabs[i].scene.document.layers.contains(&new_name)
                    {
                        self.push_undo_snapshot(i, "LAYER RENAME");
                        // acadrust Table doesn't have rename; add new + remove old
                        if let Some(old_layer) = self.tabs[i].scene.document.layers.get(&old_name) {
                            use acadrust::tables::layer::Layer as DocLayer;
                            let mut nl = DocLayer::new(&new_name);
                            nl.color = old_layer.color.clone();
                            nl.flags = old_layer.flags.clone();
                            let _ = self.tabs[i].scene.document.layers.add(nl);
                        }
                        self.tabs[i].scene.document.layers.remove(&old_name);
                        // Update entities on that layer
                        for e in self.tabs[i].scene.document.entities_mut() {
                            if e.as_entity().layer() == old_name {
                                e.as_entity_mut().set_layer(new_name.clone());
                            }
                        }
                        self.tabs[i].dirty = true;
                    }
                    let doc_layers = self.tabs[i].scene.document.layers.clone();
                    self.tabs[i].layers.sync_from_doc(&doc_layers);
                    self.tabs[i].layers.edit_buf.clear();
                    self.sync_ribbon_layers();
                }
                Task::none()
            }

            Message::LayerColorPickerToggle(idx) => {
                let i = self.active_tab;
                let panel = &mut self.tabs[i].layers;
                if panel.color_picker_row == Some(idx) {
                    panel.color_picker_row = None;
                    panel.color_full_palette = false;
                } else {
                    panel.color_picker_row = Some(idx);
                    panel.color_full_palette = false;
                    panel.selected = Some(idx);
                }
                Task::none()
            }

            Message::LayerColorMorePalette => {
                let i = self.active_tab;
                self.tabs[i].layers.color_full_palette =
                    !self.tabs[i].layers.color_full_palette;
                Task::none()
            }

            Message::LayerColorSet(aci) => {
                let i = self.active_tab;
                if let Some(idx) = self.tabs[i].layers.selected {
                    if let Some(layer) = self.tabs[i].layers.layers.get(idx) {
                        let name = layer.name.clone();
                        if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(&name) {
                            dl.color = AcadColor::Index(aci);
                        }
                        use crate::ui::layers::iced_color_from_acad;
                        let new_color = iced_color_from_acad(&AcadColor::Index(aci));
                        if let Some(pl) = self.tabs[i].layers.layers.get_mut(idx) {
                            pl.color = new_color;
                        }
                        self.tabs[i].dirty = true;
                    }
                    self.tabs[i].layers.color_picker_row = None;
                    self.tabs[i].layers.color_full_palette = false;
                    self.sync_ribbon_layers();
                }
                Task::none()
            }

            Message::LayerLinetypeSet(lt) => {
                let i = self.active_tab;
                if let Some(idx) = self.tabs[i].layers.selected {
                    if let Some(layer) = self.tabs[i].layers.layers.get(idx) {
                        let name = layer.name.clone();
                        if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(&name) {
                            dl.line_type = lt.clone();
                        }
                        if let Some(pl) = self.tabs[i].layers.layers.get_mut(idx) {
                            pl.linetype = lt;
                        }
                        self.tabs[i].dirty = true;
                    }
                }
                Task::none()
            }

            Message::LayerLineweightSet(lw) => {
                let i = self.active_tab;
                if let Some(idx) = self.tabs[i].layers.selected {
                    if let Some(layer) = self.tabs[i].layers.layers.get(idx) {
                        let name = layer.name.clone();
                        if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(&name) {
                            dl.line_weight = lw;
                        }
                        if let Some(pl) = self.tabs[i].layers.layers.get_mut(idx) {
                            pl.lineweight = lw;
                        }
                        self.tabs[i].dirty = true;
                    }
                }
                Task::none()
            }

            Message::LayerTransparencyEdit(idx, s) => {
                let i = self.active_tab;
                if let Some(pl) = self.tabs[i].layers.layers.get_mut(idx) {
                    if let Ok(v) = s.parse::<i32>() {
                        pl.transparency = v.clamp(0, 90);
                    } else if s.is_empty() {
                        pl.transparency = 0;
                    }
                }
                Task::none()
            }

            Message::CursorMoved(p) => {
                let (vw, _vh) = self.tabs[self.active_tab].scene.selection.borrow().vp_size;
                self.cursor_pos = Point::new(
                    vw - VIEWCUBE_PAD - VIEWCUBE_HIT_SIZE + p.x,
                    VIEWCUBE_PAD + p.y,
                );
                Task::none()
            }

            Message::ViewportMove(p) => {
                let i = self.active_tab;
                let mut sel = self.tabs[i].scene.selection.borrow_mut();
                sel.last_move_pos = Some(p);

                if sel.left_down {
                    let press = sel.left_press_pos.unwrap_or(p);
                    let dx = p.x - press.x;
                    let dy = p.y - press.y;
                    let dist2 = dx * dx + dy * dy;
                    let elapsed_ms = sel
                        .left_press_time
                        .map(|t| Instant::now().duration_since(t).as_millis())
                        .unwrap_or(u128::MAX);
                    if !sel.left_dragging && elapsed_ms >= POLY_START_DELAY_MS && dist2 > 9.0 {
                        sel.left_dragging = true;
                        sel.poly_active = true;
                        sel.poly_crossing = p.x < press.x;
                        sel.poly_points.clear();
                        sel.poly_points.push(press);
                        sel.poly_points.push(p);
                    } else if sel.left_dragging && sel.poly_active {
                        if sel.poly_points.last().map_or(true, |lp| {
                            let ddx = p.x - lp.x;
                            let ddy = p.y - lp.y;
                            ddx * ddx + ddy * ddy > 16.0
                        }) {
                            sel.poly_points.push(p);
                        }
                    }
                } else if sel.box_anchor.is_some() {
                    sel.box_current = Some(p);
                    if let Some(a) = sel.box_anchor {
                        sel.box_crossing = p.x < a.x;
                    }
                }

                if sel.right_down {
                    if let Some(press) = sel.right_press_pos {
                        let dx = p.x - press.x;
                        let dy = p.y - press.y;
                        if !sel.right_dragging && (dx * dx + dy * dy) > 9.0 {
                            sel.right_dragging = true;
                            sel.context_menu = None;
                        }
                    }
                    if sel.right_dragging {
                        if let Some(last) = sel.right_last_pos {
                            let (dx, dy) = (p.x - last.x, p.y - last.y);
                            self.tabs[i].scene.camera.borrow_mut().orbit(dx, dy);
                        }
                        sel.right_last_pos = Some(p);
                    }
                }

                if sel.middle_down {
                    if let Some(last) = sel.middle_last_pos {
                        let (dx, dy) = (p.x - last.x, p.y - last.y);
                        self.tabs[i].scene.camera.borrow_mut().pan(dx, dy);
                    }
                    sel.middle_last_pos = Some(p);
                }

                let vp_size = sel.vp_size;
                drop(sel);

                // ── Grip drag ─────────────────────────────────────────────
                if let Some(grip) = self.tabs[i].active_grip.clone() {
                    let (vw, vh) = vp_size;
                    let bounds = iced::Rectangle {
                        x: 0.0,
                        y: 0.0,
                        width: vw,
                        height: vh,
                    };
                    let cam = self.tabs[i].scene.camera.borrow();
                    let raw = cam.pick_on_target_plane(p, bounds);
                    let vp_mat = cam.view_proj(bounds);
                    drop(cam);

                    // Exclude the entity being dragged so its own segments
                    // don't interfere with snap (avoids Nearest/Endpoint sticking).
                    let edited_name = grip.handle.value().to_string();
                    let all_wires = self.tabs[i].scene.entity_wires();
                    let snap_wires: Vec<_> = all_wires
                        .iter()
                        .filter(|w| w.name != edited_name)
                        .cloned()
                        .collect();
                    let snap_hit = self.snapper.snap(raw, p, &snap_wires, vp_mat, bounds);
                    let mut snapped = snap_hit.map(|s| s.world).unwrap_or(raw);
                    self.tabs[i].snap_result = snap_hit;

                    // Apply ortho/polar constraints relative to the grip's origin position.
                    // Only when snap did not find a geometry hit (snap takes priority).
                    if snap_hit.is_none() {
                        let base = grip.origin_world;
                        if self.ortho_mode {
                            snapped = ortho_constrain(snapped, base);
                        } else if self.polar_mode {
                            snapped = polar_constrain(snapped, base, 45.0);
                        }
                    }

                    let apply = if grip.is_translate {
                        GripApply::Translate(snapped - grip.last_world)
                    } else {
                        GripApply::Absolute(snapped)
                    };
                    self.tabs[i]
                        .scene
                        .apply_grip(grip.handle, grip.grip_id, apply);
                    self.tabs[i].dirty = true;
                    self.tabs[i].active_grip.as_mut().unwrap().last_world = snapped;
                    self.refresh_selected_grips();
                    self.refresh_properties();
                    return Task::none();
                }

                if self.tabs[i].active_cmd.is_some() {
                    let (vw, vh) = vp_size;
                    let bounds = iced::Rectangle {
                        x: 0.0,
                        y: 0.0,
                        width: vw,
                        height: vh,
                    };
                    let cam = self.tabs[i].scene.camera.borrow();
                    let cursor_world = cam.pick_on_target_plane(p, bounds);
                    let view_proj = cam.view_proj(bounds);
                    drop(cam);

                    let all_wires = self.tabs[i].scene.entity_wires();
                    let needs_tan = self.tabs[i]
                        .active_cmd
                        .as_ref()
                        .map(|c| c.needs_tangent_pick())
                        .unwrap_or(false);
                    self.tabs[i].snap_result = if needs_tan {
                        self.snapper.snap_tangent_only(
                            cursor_world,
                            p,
                            &all_wires,
                            view_proj,
                            bounds,
                        )
                    } else {
                        self.snapper
                            .snap(cursor_world, p, &all_wires, view_proj, bounds)
                    };
                    let effective = {
                        let mut pt = self.tabs[i]
                            .snap_result
                            .map(|s| s.world)
                            .unwrap_or(cursor_world);
                        if self.tabs[i].active_cmd.is_some() {
                            pt.z = 0.0;
                        }
                        if let Some(base) = self.last_point {
                            if self.ortho_mode {
                                pt = ortho_constrain(pt, base);
                            } else if self.polar_mode {
                                pt = polar_constrain(pt, base, 45.0);
                            }
                        }
                        pt
                    };
                    self.tabs[i].last_cursor_world = effective;

                    let needs_entity = self.tabs[i]
                        .active_cmd
                        .as_ref()
                        .map(|c| c.needs_entity_pick())
                        .unwrap_or(false);
                    let previews = if needs_entity {
                        let hover_handle =
                            scene::hit_test::click_hit(p, &all_wires, view_proj, bounds)
                                .and_then(|s| Scene::handle_from_wire_name(s))
                                .unwrap_or(acadrust::Handle::NULL);
                        self.tabs[i]
                            .active_cmd
                            .as_mut()
                            .map(|c| c.on_hover_entity(hover_handle, effective))
                            .unwrap_or_default()
                    } else {
                        self.tabs[i]
                            .active_cmd
                            .as_mut()
                            .map(|c| c.on_preview_wires(effective))
                            .unwrap_or_default()
                    };
                    self.tabs[i].scene.set_preview_wires(previews);
                } else {
                    self.tabs[i].snap_result = None;
                }

                Task::none()
            }

            Message::ViewportExit => {
                let i = self.active_tab;
                let mut sel = self.tabs[i].scene.selection.borrow_mut();
                sel.left_down = false;
                sel.left_press_pos = None;
                sel.left_press_time = None;
                sel.left_dragging = false;
                sel.right_down = false;
                sel.right_press_pos = None;
                sel.right_last_pos = None;
                sel.right_dragging = false;
                sel.middle_down = false;
                sel.middle_last_pos = None;
                sel.box_anchor = None;
                sel.box_current = None;
                sel.box_crossing = false;
                sel.poly_active = false;
                sel.poly_points.clear();
                sel.poly_crossing = false;
                sel.context_menu = None;
                Task::none()
            }

            Message::ViewportLeftPress => {
                let i = self.active_tab;
                let (p, vp_size) = {
                    let sel = self.tabs[i].scene.selection.borrow();
                    let p = match sel.last_move_pos {
                        Some(p) => p,
                        None => return Task::none(),
                    };
                    (p, sel.vp_size)
                };
                let (vw, vh) = vp_size;
                let bounds = iced::Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: vw,
                    height: vh,
                };

                // Viewcube check.
                if vw > 1.0 && vh > 1.0 {
                    let cam = self.tabs[i].scene.camera.borrow();
                    if scene::hit_test(p.x, p.y, vw, vh, cam.view_rotation_mat(), VIEWCUBE_PX)
                        .is_some()
                    {
                        return Task::none();
                    }
                }

                // Grip hit-test: only when no command is active and one entity selected.
                if self.tabs[i].active_cmd.is_none() && !self.tabs[i].selected_grips.is_empty() {
                    if let Some(handle) = self.tabs[i].selected_handle {
                        let vp_mat = self.tabs[i].scene.camera.borrow().view_proj(bounds);
                        let grip_hit =
                            find_hit_grip(p, &self.tabs[i].selected_grips, vp_mat, bounds);
                        if let Some((grip_id, is_translate, world)) = grip_hit {
                            self.tabs[i].active_grip = Some(GripEdit {
                                handle,
                                grip_id,
                                is_translate,
                                origin_world: world,
                                last_world: world,
                            });
                            return Task::none(); // swallow press — selection box must not start
                        }
                    }
                }

                let mut sel = self.tabs[i].scene.selection.borrow_mut();
                sel.context_menu = None;
                sel.left_down = true;
                sel.left_press_pos = Some(p);
                sel.left_press_time = Some(Instant::now());
                sel.left_dragging = false;
                Task::none()
            }

            Message::ViewportLeftRelease => {
                let i = self.active_tab;
                // Extract the state we need before any mutable borrows.
                let (p, is_click, is_down) = {
                    let sel = self.tabs[i].scene.selection.borrow();
                    let p = match sel.last_move_pos {
                        Some(p) => p,
                        None => return Task::none(),
                    };
                    (p, !sel.left_dragging, sel.left_down)
                };

                // ── Grip drag end ─────────────────────────────────────────
                if self.tabs[i].active_grip.is_some() {
                    self.tabs[i].active_grip = None;
                    self.refresh_properties();
                    return Task::none();
                }

                // Whether the active command is in selection-gathering mode.
                let is_gathering = self.tabs[i]
                    .active_cmd
                    .as_ref()
                    .map(|c| c.is_selection_gathering())
                    .unwrap_or(false);

                // ── Active command: plain click = point pick ──────────────
                // Skip this path when gathering — let clicks flow into the
                // normal selection system below.
                if is_down && is_click && self.tabs[i].active_cmd.is_some() && !is_gathering {
                    let (vw, vh) = self.tabs[i].scene.selection.borrow().vp_size;
                    let bounds = iced::Rectangle {
                        x: 0.0,
                        y: 0.0,
                        width: vw,
                        height: vh,
                    };

                    // Take snap result once; preserve tangent_obj for pick routing.
                    let snap_taken = self.tabs[i].snap_result.take();
                    let tangent_obj_at_click = snap_taken.and_then(|s| s.tangent_obj);

                    let world_pt = {
                        let raw = self.tabs[i]
                            .scene
                            .camera
                            .borrow()
                            .pick_on_target_plane(p, bounds);
                        let vp_mat = self.tabs[i].scene.camera.borrow().view_proj(bounds);
                        let all_wires = self.tabs[i].scene.entity_wires();
                        let needs_tan = self.tabs[i]
                            .active_cmd
                            .as_ref()
                            .map(|c| c.needs_tangent_pick())
                            .unwrap_or(false);
                        let snap_hit = if needs_tan {
                            self.snapper
                                .snap_tangent_only(raw, p, &all_wires, vp_mat, bounds)
                        } else {
                            self.snapper.snap(raw, p, &all_wires, vp_mat, bounds)
                        };
                        let mut pt = snap_hit.map(|s| s.world).unwrap_or(raw);
                        pt.z = 0.0;
                        if let Some(base) = self.last_point {
                            if self.ortho_mode {
                                pt = ortho_constrain(pt, base);
                            } else if self.polar_mode {
                                pt = polar_constrain(pt, base, 45.0);
                            }
                        }
                        pt
                    };

                    let result = if self.tabs[i]
                        .active_cmd
                        .as_ref()
                        .map(|c| c.needs_entity_pick())
                        .unwrap_or(false)
                    {
                        let vp_mat2 = self.tabs[i].scene.camera.borrow().view_proj(bounds);
                        let all_wires2 = self.tabs[i].scene.entity_wires();
                        let hit = scene::hit_test::click_hit(p, &all_wires2, vp_mat2, bounds)
                            .and_then(|s| Scene::handle_from_wire_name(s));
                        if let Some(handle) = hit {
                            self.tabs[i]
                                .active_cmd
                                .as_mut()
                                .map(|c| c.on_entity_pick(handle, world_pt))
                        } else {
                            self.command_line.push_info("Nothing found at that point.");
                            None
                        }
                    } else if self.tabs[i]
                        .active_cmd
                        .as_ref()
                        .map(|c| c.needs_tangent_pick())
                        .unwrap_or(false)
                    {
                        let tangent_obj = tangent_obj_at_click;
                        if let Some(obj) = tangent_obj {
                            self.tabs[i]
                                .active_cmd
                                .as_mut()
                                .map(|c| c.on_tangent_point(obj, world_pt))
                        } else {
                            self.command_line.push_info("Select a tangent object.");
                            None
                        }
                    } else {
                        self.last_point = Some(world_pt);
                        self.tabs[i]
                            .active_cmd
                            .as_mut()
                            .map(|c| c.on_point(world_pt))
                    };
                    if let Some(r) = result {
                        let task = self.apply_cmd_result(r);
                        let mut sel = self.tabs[i].scene.selection.borrow_mut();
                        sel.left_down = false;
                        sel.left_press_pos = None;
                        sel.left_press_time = None;
                        sel.left_dragging = false;
                        return task;
                    }
                    let mut sel = self.tabs[i].scene.selection.borrow_mut();
                    sel.left_down = false;
                    sel.left_press_pos = None;
                    sel.left_press_time = None;
                    sel.left_dragging = false;
                    return Task::none();
                }

                let (is_down2, is_dragging, box_anchor, box_crossing, vp_size, elapsed_ms) = {
                    let sel = self.tabs[i].scene.selection.borrow();
                    let elapsed = sel
                        .left_press_time
                        .map(|t| Instant::now().duration_since(t).as_millis())
                        .unwrap_or(u128::MAX);
                    (
                        sel.left_down,
                        sel.left_dragging,
                        sel.box_anchor,
                        sel.box_crossing,
                        sel.vp_size,
                        elapsed,
                    )
                };

                // True when a selection action fully completed this release event,
                // meaning the gathering command should fire immediately.
                let mut selection_just_completed = false;

                if is_down2 {
                    let bounds = iced::Rectangle {
                        x: 0.0,
                        y: 0.0,
                        width: vp_size.0,
                        height: vp_size.1,
                    };

                    if is_dragging {
                        // ── Drag ended ────────────────────────────────────
                        if elapsed_ms < POLY_START_DELAY_MS {
                            if let Some(a) = box_anchor {
                                let crossing = box_crossing;
                                let all_wires = self.tabs[i].scene.entity_wires();
                                let vp_mat = self.tabs[i].scene.camera.borrow().view_proj(bounds);
                                let mut handles: Vec<Handle> = scene::hit_test::box_hit(
                                    a, p, crossing, &all_wires, vp_mat, bounds,
                                )
                                .into_iter()
                                .filter_map(|s| Scene::handle_from_wire_name(s))
                                .collect();
                                handles.extend(scene::hit_test::box_hit_hatch(
                                    a,
                                    p,
                                    crossing,
                                    &self.tabs[i].scene.hatches,
                                    vp_mat,
                                    bounds,
                                ));
                                self.tabs[i].scene.deselect_all();
                                for h in &handles {
                                    self.tabs[i].scene.select_entity(*h, false);
                                }
                                self.tabs[i].scene.expand_selection_for_groups(&handles);
                                self.refresh_properties();
                                selection_just_completed = true;
                            }
                        } else {
                            // Long drag → lasso polygon selection.
                            let (poly_pts, crossing) = {
                                let sel = self.tabs[i].scene.selection.borrow();
                                (sel.poly_points.clone(), sel.poly_crossing)
                            };
                            self.tabs[i].scene.selection.borrow_mut().poly_last_crossing = crossing;

                            let all_wires = self.tabs[i].scene.entity_wires();
                            let vp_mat = self.tabs[i].scene.camera.borrow().view_proj(bounds);
                            let mut handles: Vec<Handle> = scene::hit_test::poly_hit(
                                &poly_pts, crossing, &all_wires, vp_mat, bounds,
                            )
                            .into_iter()
                            .filter_map(|s| Scene::handle_from_wire_name(s))
                            .collect();
                            handles.extend(scene::hit_test::poly_hit_hatch(
                                &poly_pts,
                                crossing,
                                &self.tabs[i].scene.hatches,
                                vp_mat,
                                bounds,
                            ));
                            self.tabs[i].scene.deselect_all();
                            for h in &handles {
                                self.tabs[i].scene.select_entity(*h, false);
                            }
                            self.tabs[i].scene.expand_selection_for_groups(&handles);
                            self.refresh_properties();
                            selection_just_completed = true;
                        }
                        let mut sel = self.tabs[i].scene.selection.borrow_mut();
                        sel.poly_active = false;
                        sel.poly_points.clear();
                        sel.poly_crossing = false;
                        sel.box_anchor = None;
                        sel.box_current = None;
                    } else {
                        // ── Non-drag click ────────────────────────────────
                        if box_anchor.is_none() {
                            let all_wires = self.tabs[i].scene.entity_wires();
                            let vp_mat = self.tabs[i].scene.camera.borrow().view_proj(bounds);
                            let hit = scene::hit_test::click_hit(p, &all_wires, vp_mat, bounds)
                                .and_then(|s| Scene::handle_from_wire_name(s))
                                .or_else(|| {
                                    scene::hit_test::click_hit_hatch(
                                        p,
                                        &self.tabs[i].scene.hatches,
                                        vp_mat,
                                        bounds,
                                    )
                                });
                            if let Some(handle) = hit {
                                self.tabs[i].scene.select_entity(handle, true);
                                self.tabs[i].scene.expand_selection_for_groups(&[handle]);
                                self.refresh_properties();
                                // Single-click hit completes the gather; miss starts a box.
                                selection_just_completed = true;
                            } else {
                                self.tabs[i].scene.deselect_all();
                                self.refresh_properties();
                                let mut sel = self.tabs[i].scene.selection.borrow_mut();
                                sel.box_anchor = Some(p);
                                sel.box_current = Some(p);
                                sel.box_crossing = false;
                            }
                        } else {
                            // Second click → complete the selection box.
                            let a = box_anchor.unwrap();
                            let crossing = box_crossing;
                            let all_wires = self.tabs[i].scene.entity_wires();
                            let vp_mat = self.tabs[i].scene.camera.borrow().view_proj(bounds);
                            let mut handles: Vec<Handle> = scene::hit_test::box_hit(
                                a, p, crossing, &all_wires, vp_mat, bounds,
                            )
                            .into_iter()
                            .filter_map(|s| Scene::handle_from_wire_name(s))
                            .collect();
                            handles.extend(scene::hit_test::box_hit_hatch(
                                a,
                                p,
                                crossing,
                                &self.tabs[i].scene.hatches,
                                vp_mat,
                                bounds,
                            ));
                            self.tabs[i].scene.deselect_all();
                            for h in &handles {
                                self.tabs[i].scene.select_entity(*h, false);
                            }
                            self.tabs[i].scene.expand_selection_for_groups(&handles);
                            self.refresh_properties();
                            let mut sel = self.tabs[i].scene.selection.borrow_mut();
                            sel.box_last = Some((a, p));
                            sel.box_last_crossing = crossing;
                            sel.box_anchor = None;
                            sel.box_current = None;
                            sel.box_crossing = false;
                            selection_just_completed = true;
                        }
                    }

                    let mut sel = self.tabs[i].scene.selection.borrow_mut();
                    sel.left_down = false;
                    sel.left_press_pos = None;
                    sel.left_press_time = None;
                    sel.left_dragging = false;
                }

                // ── Fire gathering command after selection completes ───────
                if is_gathering && selection_just_completed {
                    let handles: Vec<Handle> = self.tabs[i]
                        .scene
                        .selected_entities()
                        .into_iter()
                        .map(|(h, _)| h)
                        .collect();
                    if let Some(cmd) = self.tabs[i].active_cmd.as_mut() {
                        let result = cmd.on_selection_complete(handles);
                        return self.apply_cmd_result(result);
                    }
                }

                Task::none()
            }

            Message::ViewportRightPress => {
                let i = self.active_tab;
                let mut sel = self.tabs[i].scene.selection.borrow_mut();
                let Some(p) = sel.last_move_pos else {
                    return Task::none();
                };
                sel.context_menu = None;
                sel.right_down = true;
                sel.right_press_pos = Some(p);
                sel.right_last_pos = Some(p);
                sel.right_dragging = false;
                Task::none()
            }

            Message::ViewportRightRelease => {
                let i = self.active_tab;
                let mut sel = self.tabs[i].scene.selection.borrow_mut();
                let Some(_p) = sel.last_move_pos else {
                    return Task::none();
                };
                if sel.right_down {
                    if !sel.right_dragging {
                        sel.context_menu = sel.last_move_pos;
                    }
                    sel.right_down = false;
                    sel.right_press_pos = None;
                    sel.right_last_pos = None;
                    sel.right_dragging = false;
                }
                Task::none()
            }

            Message::ViewportMiddlePress => {
                let i = self.active_tab;
                let now = iced::time::Instant::now();
                let is_double = {
                    let sel = self.tabs[i].scene.selection.borrow();
                    sel.middle_last_press_time
                        .map(|t| now.duration_since(t).as_millis() < 300)
                        .unwrap_or(false)
                };
                {
                    let mut sel = self.tabs[i].scene.selection.borrow_mut();
                    let Some(p) = sel.last_move_pos else {
                        return Task::none();
                    };
                    sel.middle_down = true;
                    sel.middle_last_pos = Some(p);
                    sel.middle_last_press_time = Some(now);
                }
                if is_double {
                    self.tabs[i].scene.fit_all();
                    self.command_line.push_output("Zoom Extents");
                }
                Task::none()
            }

            Message::ViewportMiddleRelease => {
                let i = self.active_tab;
                let mut sel = self.tabs[i].scene.selection.borrow_mut();
                sel.middle_down = false;
                sel.middle_last_pos = None;
                Task::none()
            }

            Message::ViewportScroll(delta) => {
                let s = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => y,
                    mouse::ScrollDelta::Pixels { y, .. } => y * 0.01,
                };
                let i = self.active_tab;
                let cursor = self.tabs[i].scene.selection.borrow().last_move_pos;
                let (vw, vh) = self.tabs[i].scene.selection.borrow().vp_size;
                let bounds = iced::Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: vw,
                    height: vh,
                };

                let mut cam = self.tabs[i].scene.camera.borrow_mut();
                if let Some(cursor) = cursor {
                    cam.zoom_about_point(cursor, bounds, s);
                } else {
                    cam.zoom(s);
                }
                Task::none()
            }

            Message::ViewportClick => {
                let i = self.active_tab;
                let cam = self.tabs[i].scene.camera.borrow();
                let (vw, vh) = self.tabs[i].scene.selection.borrow().vp_size;
                if let Some(region) = scene::hit_test(
                    self.cursor_pos.x,
                    self.cursor_pos.y,
                    vw,
                    vh,
                    cam.view_rotation_mat(),
                    VIEWCUBE_PX,
                ) {
                    return Task::done(Message::ViewCubeSnap(region));
                }
                Task::none()
            }

            Message::WindowResized(w, h) => {
                self.vp_size = ((w - 440.0).max(200.0), h);
                Task::none()
            }

            Message::ViewCubeSnap(region) => {
                let i = self.active_tab;
                let mut region = region;
                {
                    let mut cam = self.tabs[i].scene.camera.borrow_mut();
                    let (target_yaw, target_pitch) = region.snap_angles();
                    if angle_close(cam.yaw, target_yaw, 0.01)
                        && angle_close(cam.pitch, target_pitch, 0.01)
                    {
                        region = region.opposite();
                    }
                    let (yaw, pitch) = region.snap_angles();
                    cam.snap_to_angles(yaw, pitch);
                }
                self.tabs[i].scene.camera_generation += 1;
                self.command_line
                    .push_output(&format!("View: {}", region.label()));
                Task::none()
            }

            Message::ToggleSnapEnabled => {
                self.snapper.toggle_global();
                Task::none()
            }
            Message::ToggleGridSnap => {
                self.snapper.toggle(snap::SnapType::Grid);
                Task::none()
            }
            Message::ToggleGrid => {
                self.show_grid ^= true;
                Task::none()
            }
            Message::ToggleOrtho => {
                self.ortho_mode ^= true;
                if self.ortho_mode {
                    self.polar_mode = false;
                }
                Task::none()
            }
            Message::TogglePolar => {
                self.polar_mode ^= true;
                if self.polar_mode {
                    self.ortho_mode = false;
                }
                Task::none()
            }
            Message::ToggleSnap(t) => {
                self.snapper.toggle(t);
                Task::none()
            }
            Message::ToggleSnapPopup => {
                self.snap_popup_open ^= true;
                Task::none()
            }
            Message::CloseSnapPopup => {
                self.snap_popup_open = false;
                Task::none()
            }
            Message::SnapSelectAll => {
                self.snapper.enable_all();
                Task::none()
            }
            Message::SnapClearAll => {
                self.snapper.disable_all();
                Task::none()
            }

            Message::ToggleRibbonDropdown(id) => {
                self.ribbon.toggle_dropdown(&id);
                Task::none()
            }
            Message::CloseRibbonDropdown => {
                self.ribbon.close_dropdown();
                Task::none()
            }
            Message::DropdownSelectItem { dropdown_id, cmd } => {
                self.ribbon.select_dropdown_item(dropdown_id, cmd);
                self.ribbon.activate_tool(cmd);
                self.dispatch_command(cmd)
            }

            Message::DeleteSelected => {
                let i = self.active_tab;
                let handles: Vec<_> = self.tabs[i].scene.selected.iter().cloned().collect();
                if !handles.is_empty() {
                    self.push_undo_snapshot(i, "ERASE");
                    self.tabs[i].scene.erase_entities(&handles);
                    self.tabs[i].dirty = true;
                    self.refresh_properties();
                }
                Task::none()
            }

            // ── Properties panel messages ─────────────────────────────────
            Message::PropSelectionGroupChanged(group) => {
                self.tabs[self.active_tab].properties.selected_group = Some(group);
                self.refresh_properties();
                Task::none()
            }

            Message::RibbonLayerChanged(layer) => {
                let i = self.active_tab;
                self.tabs[i].active_layer = layer.clone();
                self.tabs[i].layers.current_layer = layer.clone();
                self.ribbon.active_layer = layer;
                self.ribbon.close_dropdown();
                Task::none()
            }

            Message::RibbonColorChanged(color) => {
                self.ribbon.active_color = color;
                self.ribbon.prop_color_palette_open = false;
                self.ribbon.close_dropdown();
                Task::none()
            }
            Message::RibbonColorPaletteToggle => {
                self.ribbon.prop_color_palette_open ^= true;
                Task::none()
            }
            Message::RibbonLinetypeChanged(lt) => {
                self.ribbon.active_linetype = lt;
                self.ribbon.close_dropdown();
                Task::none()
            }
            Message::RibbonLineweightChanged(lw) => {
                self.ribbon.active_lineweight = lw;
                self.ribbon.close_dropdown();
                Task::none()
            }

            Message::PropLayerChanged(layer) => {
                let i = self.active_tab;
                let handles = self.property_target_handles(i);
                if !handles.is_empty() {
                    self.push_undo_snapshot(i, "CHPROP");
                    for handle in handles {
                        if let Some(entity) = self.tabs[i].scene.document.get_entity_mut(handle) {
                            dispatch::apply_common_prop(entity, "layer", &layer);
                        }
                    }
                    self.tabs[i].dirty = true;
                    self.refresh_properties();
                }
                Task::none()
            }

            Message::PropColorChanged(color) => {
                let i = self.active_tab;
                let handles = self.property_target_handles(i);
                if !handles.is_empty() {
                    self.push_undo_snapshot(i, "CHPROP");
                    for handle in handles {
                        if let Some(entity) = self.tabs[i].scene.document.get_entity_mut(handle) {
                            dispatch::apply_color(entity, color);
                        }
                    }
                    self.tabs[i].properties.color_picker_open = false;
                    self.tabs[i].properties.color_palette_open = false;
                    self.tabs[i].dirty = true;
                    self.refresh_properties();
                }
                Task::none()
            }

            Message::PropLwChanged(lw) => {
                let i = self.active_tab;
                let handles = self.property_target_handles(i);
                if !handles.is_empty() {
                    self.push_undo_snapshot(i, "CHPROP");
                    for handle in handles {
                        if let Some(entity) = self.tabs[i].scene.document.get_entity_mut(handle) {
                            dispatch::apply_line_weight(entity, lw);
                        }
                    }
                    self.tabs[i].dirty = true;
                    self.refresh_properties();
                }
                Task::none()
            }

            Message::PropLinetypeChanged(lt) => {
                let i = self.active_tab;
                let handles = self.property_target_handles(i);
                if !handles.is_empty() {
                    self.push_undo_snapshot(i, "CHPROP");
                    for handle in handles {
                        if let Some(entity) = self.tabs[i].scene.document.get_entity_mut(handle) {
                            dispatch::apply_common_prop(entity, "linetype", &lt);
                        }
                    }
                    self.tabs[i].dirty = true;
                    self.refresh_properties();
                }
                Task::none()
            }

            Message::PropHatchPatternChanged(name) => {
                let i = self.active_tab;
                let handles = self.property_target_handles(i);
                if !handles.is_empty() {
                    use crate::scene::hatch_patterns;
                    if let Some(entry) = hatch_patterns::find(&name) {
                        self.push_undo_snapshot(i, "HATCHEDIT");
                        for handle in handles {
                            if let Some(acadrust::EntityType::Hatch(dxf)) =
                                self.tabs[i].scene.document.get_entity_mut(handle)
                            {
                                dxf.pattern = hatch_patterns::build_dxf_pattern(entry);
                                dxf.is_solid = matches!(
                                    entry.gpu,
                                    crate::scene::hatch_model::HatchPattern::Solid
                                );
                            }
                            if let Some(model) = self.tabs[i].scene.hatches.get_mut(&handle) {
                                model.pattern = entry.gpu.clone();
                                model.name = name.clone();
                            }
                        }
                        self.tabs[i].dirty = true;
                        self.refresh_properties();
                    }
                }
                Task::none()
            }

            Message::PropBoolToggle(field) => {
                let i = self.active_tab;
                let handles = self.property_target_handles(i);
                if !handles.is_empty() {
                    self.push_undo_snapshot(i, "CHPROP");
                    for handle in handles {
                        if let Some(entity) = self.tabs[i].scene.document.get_entity_mut(handle) {
                            match field {
                                "invisible" => dispatch::toggle_invisible(entity),
                                _ => dispatch::apply_geom_prop(entity, field, "toggle"),
                            }
                        }
                    }
                    self.tabs[i].dirty = true;
                    self.refresh_properties();
                }
                Task::none()
            }

            Message::PropGeomChoiceChanged { field, value } => {
                let i = self.active_tab;
                let handles = self.property_target_handles(i);
                if !handles.is_empty() {
                    self.push_undo_snapshot(i, "CHPROP");
                    for handle in handles {
                        if let Some(entity) = self.tabs[i].scene.document.get_entity_mut(handle) {
                            dispatch::apply_geom_prop(entity, field, &value);
                        }
                    }
                    self.tabs[i].dirty = true;
                    self.refresh_properties();
                }
                Task::none()
            }

            Message::PropGeomInput { field, value } => {
                self.tabs[self.active_tab]
                    .properties
                    .edit_buf
                    .insert(field.to_string(), value);
                Task::none()
            }

            Message::PropGeomCommit(field) => {
                let i = self.active_tab;
                let handles = self.property_target_handles(i);
                if !handles.is_empty() {
                    if let Some(val) = self.tabs[i].properties.edit_buf.remove(field) {
                        self.push_undo_snapshot(i, "CHPROP");
                        for handle in handles {
                            if let Some(entity) = self.tabs[i].scene.document.get_entity_mut(handle)
                            {
                                match field {
                                    "linetype_scale" | "transparency" => {
                                        dispatch::apply_common_prop(entity, field, &val);
                                    }
                                    _ => {
                                        dispatch::apply_geom_prop(entity, field, &val);
                                    }
                                }
                            }
                        }
                        self.tabs[i].dirty = true;
                        self.refresh_properties();
                    }
                }
                Task::none()
            }

            Message::PropColorPickerToggle => {
                let i = self.active_tab;
                self.tabs[i].properties.color_picker_open =
                    !self.tabs[i].properties.color_picker_open;
                if self.tabs[i].properties.color_picker_open {
                    self.tabs[i].properties.color_palette_open = false;
                }
                Task::none()
            }

            Message::PropColorPaletteToggle => {
                self.tabs[self.active_tab].properties.color_palette_open =
                    !self.tabs[self.active_tab].properties.color_palette_open;
                Task::none()
            }

            Message::LayoutSwitch(name) => {
                let i = self.active_tab;
                let going_to_paper = name != "Model";
                self.tabs[i].scene.current_layout = name;
                self.tabs[i].scene.deselect_all();
                self.tabs[i].scene.fit_all();
                // Switch to Layout tab when entering paper space; back to Home when leaving.
                if going_to_paper {
                    if let Some(idx) = self.ribbon.layout_module_index() {
                        self.ribbon.select(idx);
                    }
                } else {
                    if self.ribbon.active_is_layout() {
                        self.ribbon.select(0);
                    }
                }
                Task::none()
            }

            Message::LayoutCreate => {
                let i = self.active_tab;
                let count = self.tabs[i].scene.layout_names().len(); // "Model" + existing paper
                let new_name = format!("Layout{}", count);
                self.push_undo_snapshot(i, "LAYOUT");
                match self.tabs[i].scene.document.add_layout(&new_name) {
                    Ok(_) => {
                        self.tabs[i].scene.current_layout = new_name.clone();
                        self.tabs[i].scene.deselect_all();
                        self.tabs[i].scene.fit_all();
                        if let Some(idx) = self.ribbon.layout_module_index() {
                            self.ribbon.select(idx);
                        }
                        self.command_line.push_output(&format!(
                            "Layout \"{new_name}\" oluşturuldu — MVIEW ile viewport ekleyin"
                        ));
                        self.tabs[i].dirty = true;
                    }
                    Err(e) => self
                        .command_line
                        .push_error(&format!("Layout oluşturulamadı: {e}")),
                }
                Task::none()
            }

            Message::Undo => {
                self.undo_active_tab();
                Task::none()
            }

            Message::Redo => {
                self.redo_active_tab();
                Task::none()
            }

            Message::UndoMany(steps) => {
                self.ribbon.close_dropdown();
                self.undo_steps(steps);
                Task::none()
            }

            Message::RedoMany(steps) => {
                self.ribbon.close_dropdown();
                self.redo_steps(steps);
                Task::none()
            }

            Message::Noop => Task::none(),
        }
    }

    fn dispatch_command(&mut self, cmd: &str) -> Task<Message> {
        let i = self.active_tab;
        // Cancel any running command before starting a new one.
        if self.tabs[i].active_cmd.is_some() {
            self.tabs[i].scene.clear_preview_wire();
            self.tabs[i].active_cmd = None;
        }

        if let Some(path_str) = cmd.strip_prefix("OPEN_RECENT:") {
            let path = PathBuf::from(path_str);
            return Task::perform(io::open_path(path), Message::FileOpened);
        }

        match cmd {
            "NEW"                => return Task::done(Message::ClearScene),
            "OPEN"               => return Task::done(Message::OpenFile),
            "SAVE"|"QSAVE"       => return Task::done(Message::SaveFile),
            "SAVEAS"             => return Task::done(Message::SaveAs),
            "UNDO"|"U"           => return Task::done(Message::Undo),
            "REDO"               => return Task::done(Message::Redo),
            "CLEAR"|"CLR"        => return Task::done(Message::ClearScene),
            "WIREFRAME"|"VW" => return Task::done(Message::SetWireframe(true)),
            "SOLID"|"VS"     => return Task::done(Message::SetWireframe(false)),
            "ORTHO"          => return Task::done(Message::SetProjection(true)),
            "PERSP"          => return Task::done(Message::SetProjection(false)),
            "LAYERS"|"LA"    => return Task::done(Message::ToggleLayers),

            // ── Layer object commands ──────────────────────────────────────
            // Commands that operate on the layer of selected (or picked) objects.

            "LAYOFF" => {
                let handles: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("LAYOFF");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    let layers: std::collections::HashSet<String> = self.tabs[i].scene
                        .selected_entities().into_iter()
                        .map(|(_, e)| e.common().layer.clone()).collect();
                    self.push_undo_snapshot(i, "LAYOFF");
                    for name in &layers {
                        if name == "0" { continue; }
                        if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(name) {
                            dl.turn_off();
                        }
                    }
                    self.tabs[i].dirty = true;
                    self.sync_ribbon_layers();
                    self.command_line.push_info("Layer(s) turned off.");
                }
            }

            "LAYFRZ" => {
                let handles: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("LAYFRZ");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    let layers: std::collections::HashSet<String> = self.tabs[i].scene
                        .selected_entities().into_iter()
                        .map(|(_, e)| e.common().layer.clone()).collect();
                    self.push_undo_snapshot(i, "LAYFRZ");
                    for name in &layers {
                        if name == "0" { continue; }
                        if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(name) {
                            dl.freeze();
                        }
                    }
                    self.tabs[i].dirty = true;
                    self.sync_ribbon_layers();
                    self.command_line.push_info("Layer(s) frozen.");
                }
            }

            "LAYLCK" => {
                let handles: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("LAYLCK");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    let layers: std::collections::HashSet<String> = self.tabs[i].scene
                        .selected_entities().into_iter()
                        .map(|(_, e)| e.common().layer.clone()).collect();
                    self.push_undo_snapshot(i, "LAYLCK");
                    for name in &layers {
                        if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(name) {
                            dl.lock();
                        }
                    }
                    self.tabs[i].dirty = true;
                    self.sync_ribbon_layers();
                    self.command_line.push_info("Layer(s) locked.");
                }
            }

            "LAYMCUR" => {
                let entities = self.tabs[i].scene.selected_entities();
                if entities.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("LAYMCUR");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    let layer = entities[0].1.common().layer.clone();
                    self.tabs[i].active_layer = layer.clone();
                    self.ribbon.active_layer = layer.clone();
                    self.tabs[i].layers.current_layer = layer.clone();
                    self.command_line.push_info(&format!("Current layer set to \"{layer}\"."));
                    self.sync_ribbon_layers();
                }
            }

            "LAYON" => {
                self.push_undo_snapshot(i, "LAYON");
                for name in self.tabs[i].scene.document.layers.iter()
                    .map(|l| l.name.clone()).collect::<Vec<_>>()
                {
                    if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(&name) {
                        dl.turn_on();
                    }
                }
                self.tabs[i].dirty = true;
                self.sync_ribbon_layers();
                self.command_line.push_info("All layers turned on.");
            }

            "LAYTHW" => {
                self.push_undo_snapshot(i, "LAYTHW");
                for name in self.tabs[i].scene.document.layers.iter()
                    .map(|l| l.name.clone()).collect::<Vec<_>>()
                {
                    if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(&name) {
                        dl.thaw();
                    }
                }
                self.tabs[i].dirty = true;
                self.sync_ribbon_layers();
                self.command_line.push_info("All layers thawed.");
            }

            "LAYULK" => {
                let handles: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("LAYULK");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    let layers: std::collections::HashSet<String> = self.tabs[i].scene
                        .selected_entities().into_iter()
                        .map(|(_, e)| e.common().layer.clone()).collect();
                    self.push_undo_snapshot(i, "LAYULK");
                    for name in &layers {
                        if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(name) {
                            dl.unlock();
                        }
                    }
                    self.tabs[i].dirty = true;
                    self.sync_ribbon_layers();
                    self.command_line.push_info("Layer(s) unlocked.");
                }
            }

            "LAYMATCH"|"LAYMCH" => {
                use crate::command::laymatch::LayMatchCommand;
                let dest: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().map(|(h, _)| h).collect();
                let cmd = LayMatchCommand::new(dest);
                self.command_line.push_info(&cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(cmd));
            }

            "MATCHPROP"|"MA" => {
                use crate::command::matchprop::MatchPropCommand;
                self.tabs[i].scene.deselect_all();
                let cmd = MatchPropCommand::new();
                self.command_line.push_info(&cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(cmd));
            }

            "GROUP"|"G" => {
                let handles: Vec<_> = self.tabs[i]
                    .scene
                    .selected_entities()
                    .into_iter()
                    .map(|(h, _)| h)
                    .collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("GROUP");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    let auto_name = Self::next_group_auto_name(&self.tabs[i].scene);
                    use crate::command::group::GroupCommand;
                    let cmd = GroupCommand::new(handles, auto_name);
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                }
            }

            "UNGROUP"|"UG" => {
                let handles: Vec<_> = self.tabs[i]
                    .scene
                    .selected_entities()
                    .into_iter()
                    .map(|(h, _)| h)
                    .collect();
                if handles.is_empty() {
                    use crate::command::ungroup::UngroupCommand;
                    let cmd = UngroupCommand::new();
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    self.push_undo_snapshot(i, "UNGROUP");
                    let count = self.tabs[i].scene.delete_groups_containing(&handles);
                    self.tabs[i].dirty = true;
                    if count > 0 {
                        self.command_line.push_info(&format!("{} group(s) dissolved.", count));
                    } else {
                        self.command_line.push_info("No groups found for selected objects.");
                    }
                }
            }

            "COPYCLIP"|"CC" => {
                let handles: Vec<_> = self.tabs[i]
                    .scene.selected_entities().into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("COPYCLIP");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    let entities: Vec<_> = handles.iter()
                        .filter_map(|&h| self.tabs[i].scene.document.get_entity(h).cloned())
                        .collect();
                    self.clipboard_centroid = Self::entities_centroid(
                        &self.tabs[i].scene.wire_models_for(&handles),
                    );
                    self.clipboard = entities;
                    self.command_line.push_info(
                        &format!("{} object(s) copied to clipboard.", self.clipboard.len()),
                    );
                }
            }

            "CUTCLIP"|"CX" => {
                let handles: Vec<_> = self.tabs[i]
                    .scene.selected_entities().into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("CUTCLIP");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    let entities: Vec<_> = handles.iter()
                        .filter_map(|&h| self.tabs[i].scene.document.get_entity(h).cloned())
                        .collect();
                    self.clipboard_centroid = Self::entities_centroid(
                        &self.tabs[i].scene.wire_models_for(&handles),
                    );
                    let count = entities.len();
                    self.clipboard = entities;
                    self.push_undo_snapshot(i, "CUTCLIP");
                    self.tabs[i].scene.erase_entities(&handles);
                    self.tabs[i].scene.deselect_all();
                    self.tabs[i].dirty = true;
                    self.refresh_properties();
                    self.command_line.push_info(
                        &format!("{} object(s) cut to clipboard.", count),
                    );
                }
            }

            "PASTECLIP"|"PC" => {
                if self.clipboard.is_empty() {
                    self.command_line.push_error("Clipboard is empty.");
                } else {
                    let wires = self.tabs[i].scene.wires_for_entities(&self.clipboard);
                    use crate::command::paste::PasteCommand;
                    let cmd = PasteCommand::new(wires, self.clipboard_centroid);
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                }
            }

            "BLOCK" => {
                let handles: Vec<_> = self.tabs[i]
                    .scene
                    .selected_entities()
                    .into_iter()
                    .map(|(h, _)| h)
                    .collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("BLOCK");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    use crate::modules::insert::create_block::CreateBlockCommand;
                    let cmd = CreateBlockCommand::new(handles);
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                }
            }
            "INSERT" => {
                let blocks = self.tabs[i].scene.custom_block_names();
                if blocks.is_empty() {
                    self.command_line
                        .push_error("No user-defined blocks found in this drawing.");
                } else {
                    use crate::modules::insert::insert_block::InsertBlockCommand;
                    let cmd = InsertBlockCommand::new(blocks);
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                }
            }

            "LINE"|"L" => {
                use crate::modules::home::draw::line::LineCommand;
                let new_cmd = LineCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "CIRCLE"|"C" => {
                use crate::modules::home::draw::circle::CircleCommand;
                let new_cmd = CircleCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "CIRCLE_CD" => {
                use crate::modules::home::draw::circle::CircleCDCommand;
                let new_cmd = CircleCDCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "CIRCLE_2P" => {
                use crate::modules::home::draw::circle::Circle2PCommand;
                let new_cmd = Circle2PCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "CIRCLE_3P" => {
                use crate::modules::home::draw::circle::Circle3PCommand;
                let new_cmd = Circle3PCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "CIRCLE_TTR" => {
                use crate::modules::home::draw::circle::CircleTTRCommand;
                let new_cmd = CircleTTRCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.pre_cmd_tangent = Some(self.snapper.is_on(crate::snap::SnapType::Tangent));
                self.snapper.enabled.insert(crate::snap::SnapType::Tangent);
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "CIRCLE_TTT" => {
                use crate::modules::home::draw::circle::CircleTTTCommand;
                let new_cmd = CircleTTTCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.pre_cmd_tangent = Some(self.snapper.is_on(crate::snap::SnapType::Tangent));
                self.snapper.enabled.insert(crate::snap::SnapType::Tangent);
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "ARC"|"A" => {
                use crate::modules::home::draw::arc::ArcCommand;
                let new_cmd = ArcCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "ARC_3P" => {
                use crate::modules::home::draw::arc::Arc3PCommand;
                let new_cmd = Arc3PCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "ARC_SCE" => {
                use crate::modules::home::draw::arc::ArcSCECommand;
                let new_cmd = ArcSCECommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "ARC_SCA" => {
                use crate::modules::home::draw::arc::ArcSCACommand;
                let new_cmd = ArcSCACommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "ARC_SCL" => {
                use crate::modules::home::draw::arc::ArcSCLCommand;
                let new_cmd = ArcSCLCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "ARC_SEA" => {
                use crate::modules::home::draw::arc::ArcSEACommand;
                let new_cmd = ArcSEACommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "ARC_SER" => {
                use crate::modules::home::draw::arc::ArcSERCommand;
                let new_cmd = ArcSERCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "ARC_SED" => {
                use crate::modules::home::draw::arc::ArcSEDCommand;
                let new_cmd = ArcSEDCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "ARC_CSA" => {
                use crate::modules::home::draw::arc::ArcCSACommand;
                let new_cmd = ArcCSACommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "ARC_CSL" => {
                use crate::modules::home::draw::arc::ArcCSLCommand;
                let new_cmd = ArcCSLCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "RECT" => {
                use crate::modules::home::draw::shapes::RectCommand;
                let new_cmd = RectCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "RECT_ROT" => {
                use crate::modules::home::draw::shapes::RectRotCommand;
                let new_cmd = RectRotCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "RECT_CEN" => {
                use crate::modules::home::draw::shapes::RectCenCommand;
                let new_cmd = RectCenCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "POLY" => {
                use crate::modules::home::draw::shapes::PolyCommand;
                let new_cmd = PolyCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "POLY_C" => {
                use crate::modules::home::draw::shapes::PolyCCommand;
                let new_cmd = PolyCCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "POLY_E" => {
                use crate::modules::home::draw::shapes::PolyECommand;
                let new_cmd = PolyECommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "PLINE"|"PL" => {
                use crate::modules::home::draw::polyline::PlineCommand;
                let new_cmd = PlineCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "MOVE"|"M" => {
                let handles: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("MOVE");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    use crate::modules::home::modify::translate::MoveCommand;
                    let wires = self.tabs[i].scene.wire_models_for(&handles);
                    let new_cmd = MoveCommand::new(handles, wires);
                    self.command_line.push_info(&new_cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(new_cmd));
                }
            }

            "COPY"|"CO" => {
                let handles: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("COPY");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    use crate::modules::home::modify::copy::CopyCommand;
                    let wires = self.tabs[i].scene.wire_models_for(&handles);
                    let new_cmd = CopyCommand::new(handles, wires);
                    self.command_line.push_info(&new_cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(new_cmd));
                }
            }

            "ROTATE"|"RO" => {
                let handles: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("ROTATE");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    use crate::modules::home::modify::rotate::RotateCommand;
                    let wires = self.tabs[i].scene.wire_models_for(&handles);
                    let new_cmd = RotateCommand::new(handles, wires);
                    self.command_line.push_info(&new_cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(new_cmd));
                }
            }
            "POINT"|"PO" => {
                use crate::modules::home::draw::point::PointCommand;
                let new_cmd = PointCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "HATCH"|"H" => {
                use crate::modules::home::draw::hatch::HatchCommand;
                let outlines = self.tabs[i].scene.closed_outlines();
                let new_cmd = HatchCommand::new(outlines);
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "GRADIENT" => {
                use crate::modules::home::draw::hatch::GradientCommand;
                let outlines = self.tabs[i].scene.closed_outlines();
                let new_cmd = GradientCommand::new(outlines);
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "BOUNDARY" => {
                use crate::modules::home::draw::hatch::BoundaryCommand;
                let outlines = self.tabs[i].scene.closed_outlines();
                let new_cmd = BoundaryCommand::new(outlines);
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "ELLIPSE"|"EL" => {
                use crate::modules::home::draw::ellipse::EllipseCommand;
                let new_cmd = EllipseCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "ELLIPSE_AXIS" => {
                use crate::modules::home::draw::ellipse::EllipseAxisCommand;
                let new_cmd = EllipseAxisCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "ELLIPSE_ARC" => {
                use crate::modules::home::draw::ellipse::EllipseArcCommand;
                let new_cmd = EllipseArcCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "SPLINE"|"SPL" => {
                use crate::modules::home::draw::spline::SplineCommand;
                let new_cmd = SplineCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "SCALE"|"SC" => {
                let handles: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("SCALE");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    use crate::modules::home::modify::scale::ScaleCommand;
                    let wires = self.tabs[i].scene.wire_models_for(&handles);
                    let new_cmd = ScaleCommand::new(handles, wires);
                    self.command_line.push_info(&new_cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(new_cmd));
                }
            }

            "MIRROR"|"MI" => {
                let handles: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("MIRROR");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    use crate::modules::home::modify::mirror::MirrorCommand;
                    let wires = self.tabs[i].scene.wire_models_for(&handles);
                    let new_cmd = MirrorCommand::new(handles, wires);
                    self.command_line.push_info(&new_cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(new_cmd));
                }
            }

            "ERASE"|"E" => {
                let handles: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("ERASE");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    let n = handles.len();
                    self.push_undo_snapshot(i, "ERASE");
                    self.tabs[i].scene.erase_entities(&handles);
                    self.tabs[i].dirty = true;
                    self.refresh_properties();
                    self.command_line.push_output(&format!("{n} object(s) erased."));
                }
            }

            "TEXT"|"T"|"DT" => {
                use crate::modules::annotate::text::TextCommand;
                let new_cmd = TextCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "MTEXT"|"MT" => {
                use crate::modules::annotate::mtext::MTextCommand;
                let new_cmd = MTextCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "DIMLINEAR" => {
                use crate::modules::annotate::linear_dim::LinearDimensionCommand;
                let new_cmd = LinearDimensionCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "DIMRADIUS" => {
                use crate::modules::annotate::radius_dim::RadiusDimensionCommand;
                let new_cmd = RadiusDimensionCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "DIMANGULAR" => {
                use crate::modules::annotate::angular_dim::AngularDimensionCommand;
                let new_cmd = AngularDimensionCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "LEADER"|"LE" => {
                use crate::modules::annotate::leader_cmd::LeaderCommand;
                let new_cmd = LeaderCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "MLEADER"|"MLD" => {
                use crate::modules::annotate::mleader_cmd::MLeaderCommand;
                let new_cmd = MLeaderCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "ZOOM EXTENTS"|"ZOOMEXTENTS"|"ZE" => {
                self.tabs[i].scene.fit_all();
                self.command_line.push_output("Zoom Extents");
            }
            "STRETCH"|"SS" => {
                let handles: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("STRETCH");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    use crate::modules::home::modify::stretch::StretchCommand;
                    let new_cmd = StretchCommand::new(handles);
                    self.command_line.push_info(&new_cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(new_cmd));
                }
            }

            "FILLET"|"F" => {
                use crate::modules::home::modify::fillet::FilletCommand;
                let entities: Vec<_> = self.tabs[i].scene.entity_wires().iter()
                    .filter_map(|w| {
                        let h = Scene::handle_from_wire_name(&w.name)?;
                        self.tabs[i].scene.document.get_entity(h).cloned().map(|e| (h, e))
                    }).collect();
                let all_entities: Vec<_> = entities.into_iter().map(|(_, e)| e).collect();
                let new_cmd = FilletCommand::new(
                    crate::modules::home::defaults::get_fillet_radius(), all_entities);
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }


            "ARRAY"|"AR"|"ARRAYRECT" => {
                let handles: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("ARRAYRECT");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    use crate::modules::home::modify::array::ArrayRectCommand;
                    let wires = self.tabs[i].scene.wire_models_for(&handles);
                    let new_cmd = ArrayRectCommand::new(handles, wires);
                    self.command_line.push_info(&new_cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(new_cmd));
                }
            }

            "ARRAYPOLAR" => {
                let handles: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("ARRAYPOLAR");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    use crate::modules::home::modify::array::ArrayPolarCommand;
                    let wires = self.tabs[i].scene.wire_models_for(&handles);
                    let new_cmd = ArrayPolarCommand::new(handles, wires);
                    self.command_line.push_info(&new_cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(new_cmd));
                }
            }

            "ARRAYPATH" => {
                let handles: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("ARRAYPATH");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    use crate::modules::home::modify::array::ArrayPathCommand;
                    let wires       = self.tabs[i].scene.wire_models_for(&handles);
                    let all_entities: Vec<_> = self.tabs[i].scene.entity_wires().iter()
                        .filter_map(|w| {
                            let h = Scene::handle_from_wire_name(&w.name)?;
                            self.tabs[i].scene.document.get_entity(h).cloned()
                        }).collect();
                    let new_cmd = ArrayPathCommand::new(handles, wires, all_entities);
                    self.command_line.push_info(&new_cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(new_cmd));
                }
            }

            "CHAMFER"|"CHA" => {
                use crate::modules::home::modify::fillet::ChamferCommand;
                let entities: Vec<_> = self.tabs[i].scene.entity_wires().iter()
                    .filter_map(|w| {
                        let h = Scene::handle_from_wire_name(&w.name)?;
                        self.tabs[i].scene.document.get_entity(h).cloned().map(|e| (h, e))
                    }).collect();
                let all_entities: Vec<_> = entities.into_iter().map(|(_, e)| e).collect();
                let new_cmd = ChamferCommand::new(
                    crate::modules::home::defaults::get_chamfer_dist1(), all_entities);
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "EXPLODE"|"X" => {
                use crate::modules::home::modify::explode::explode_entity;
                // Collect into owned vec to release borrow before mutating
                let entities: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().collect();
                if entities.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("EXPLODE");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    // Pre-compute all replacements before mutating
                    let replacements: Vec<(acadrust::Handle, Vec<acadrust::EntityType>)> = entities
                        .iter()
                        .filter_map(|(h, e)| {
                            let pieces = explode_entity(e, &self.tabs[i].scene.document);
                            if pieces.is_empty() { None } else { Some((*h, pieces)) }
                        })
                        .collect();
                    let exploded = replacements.len();
                    if exploded > 0 {
                        self.push_undo_snapshot(i, "EXPLODE");
                    }
                    for (handle, pieces) in replacements {
                        self.tabs[i].scene.erase_entities(&[handle]);
                        for piece in pieces {
                            self.tabs[i].scene.add_entity(piece);
                        }
                    }
                    if exploded > 0 {
                        self.tabs[i].dirty = true;
                        self.refresh_properties();
                        self.command_line.push_output(&format!("{exploded} object(s) exploded."));
                    } else {
                        self.command_line.push_info("EXPLODE: no explodable objects selected.");
                    }
                }
            }

            "OFFSET"|"O" => {
                use crate::modules::home::modify::offset::OffsetCommand;
                let all_entities: Vec<_> = self.tabs[i].scene.entity_wires().iter()
                    .filter_map(|w| {
                        let h = Scene::handle_from_wire_name(&w.name)?;
                        self.tabs[i].scene.document.get_entity(h).cloned()
                    }).collect();
                let new_cmd = OffsetCommand::new(
                    crate::modules::home::defaults::get_offset_dist(), all_entities);
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "TRIM"|"TR" => {
                use crate::modules::home::modify::trim::TrimCommand;
                let entities: Vec<_> = self.tabs[i].scene.entity_wires().iter()
                    .filter_map(|w| {
                        let h = Scene::handle_from_wire_name(&w.name)?;
                        self.tabs[i].scene.document.get_entity(h).cloned().map(|e| (h, e))
                    }).collect();
                let all_entities: Vec<_> = entities.into_iter().map(|(_, e)| e).collect();
                let new_cmd = TrimCommand::new(all_entities);
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "EXTEND"|"EX" => {
                use crate::modules::home::modify::trim::ExtendCommand;
                let entities: Vec<_> = self.tabs[i].scene.entity_wires().iter()
                    .filter_map(|w| {
                        let h = Scene::handle_from_wire_name(&w.name)?;
                        self.tabs[i].scene.document.get_entity(h).cloned().map(|e| (h, e))
                    }).collect();
                let all_entities: Vec<_> = entities.into_iter().map(|(_, e)| e).collect();
                let new_cmd = ExtendCommand::new(all_entities);
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }
            "3DORBIT"|"3O"=> self.command_line.push_info("3D Orbit: drag with right mouse button."),
            "HELP"|"?"    => self.command_line.push_output("Draw: LINE CIRCLE ARC PLINE POINT ELLIPSE SPLINE  |  Modify: MOVE COPY ROTATE SCALE MIRROR ERASE  |  Text: TEXT MTEXT  |  File: OPEN SAVE SAVEAS"),

            "DONATE" => {
                let _ = open::that("https://patreon.com/HakanSeven12");
                self.command_line.push_info("Opening Patreon page...");
            }

            // ── Layout / viewport ─────────────────────────────────────────
            "MVIEW"|"MV" => {
                if self.tabs[i].scene.current_layout == "Model" {
                    self.command_line.push_error("MVIEW: önce bir paper space layout'una geçin.");
                } else {
                    use crate::modules::layout::mview::MviewCommand;
                    let new_cmd = MviewCommand::new();
                    self.command_line.push_info(&new_cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(new_cmd));
                }
            }

            _             => self.command_line.push_error(&format!("Unknown command: {cmd}")),
        }
        // Focus the command line whenever a command just became active.
        let i = self.active_tab;
        if self.tabs[i].active_cmd.is_some() {
            self.tabs[i].last_cmd = Some(cmd.to_string());
            self.focus_cmd_input()
        } else {
            Task::none()
        }
    }

    /// Rebuild the PropertiesPanel from the current entity selection.
    /// Preserves UI state (open pickers, edit buffer) across refreshes.
    fn refresh_properties(&mut self) {
        let i = self.active_tab;
        // Preserve transient UI state across rebuilds.
        let color_picker_open = self.tabs[i].properties.color_picker_open;
        let color_palette_open = self.tabs[i].properties.color_palette_open;
        let edit_buf = std::mem::take(&mut self.tabs[i].properties.edit_buf);
        let selected_group = self.tabs[i].properties.selected_group.clone();

        let layer_names: Vec<String> = self.tabs[i]
            .scene
            .document
            .layers
            .iter()
            .map(|l| l.name.clone())
            .collect();
        let linetype_items: Vec<ui::properties::LinetypeItem> = self.tabs[i]
            .scene
            .document
            .line_types
            .iter()
            .map(|lt| {
                let name = if lt.name.is_empty() {
                    "ByLayer".to_string()
                } else {
                    lt.name.clone()
                };
                let art = linetypes::extract_pattern(&lt.description);
                ui::properties::LinetypeItem { name, art }
            })
            .collect();
        let text_style_names: Vec<String> = self.tabs[i]
            .scene
            .document
            .text_styles
            .iter()
            .map(|style| style.name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect();

        let new_panel = {
            let selected = self.tabs[i].scene.selected_entities();
            let mut panel = match selected.len() {
                0 => ui::PropertiesPanel::empty(),
                1 => {
                    let (handle, entity) = selected[0];
                    let group_names = self.tabs[i].scene.group_names_for_entity(handle);
                    let mut sections =
                        dispatch::properties_sectioned(handle, entity, &text_style_names);
                    if !group_names.is_empty() {
                        let label = group_names.join(", ");
                        if let Some(general) = sections.first_mut() {
                            general.props.push(crate::scene::object::Property {
                                label: "Group".to_string(),
                                field: "group",
                                value: crate::scene::object::PropValue::ReadOnly(label),
                            });
                        }
                    }
                    let title = entity_type_label(entity);
                    ui::PropertiesPanel {
                        choice_combos: sections
                            .iter()
                            .flat_map(|section| section.props.iter())
                            .filter_map(|prop| match &prop.value {
                                crate::scene::object::PropValue::Choice { options, .. } => Some((
                                    prop.field.to_string(),
                                    iced::widget::combo_box::State::new(options.clone()),
                                )),
                                _ => None,
                            })
                            .collect(),
                        sections,
                        title,
                        layer_combo: iced::widget::combo_box::State::new(layer_names.clone()),
                        linetype_combo: iced::widget::combo_box::State::new(linetype_items.clone()),
                        hatch_pattern_combo: iced::widget::combo_box::State::new(
                            crate::scene::hatch_patterns::names(),
                        ),
                        lineweight_combo: iced::widget::combo_box::State::new(
                            ui::properties::lw_options(),
                        ),
                        linetype_items,
                        ..Default::default()
                    }
                }
                _ => {
                    let groups = build_selection_groups(&selected);
                    let active_group = selected_group
                        .and_then(|group| groups.iter().find(|g| g.label == group.label).cloned())
                        .or_else(|| groups.first().cloned());

                    let filtered: Vec<(Handle, &EntityType)> = active_group
                        .as_ref()
                        .map(|group| {
                            selected
                                .iter()
                                .filter(|(handle, _)| group.handles.contains(handle))
                                .copied()
                                .collect()
                        })
                        .unwrap_or_default();

                    let sections = aggregate_sections(&filtered, &text_style_names);
                    ui::PropertiesPanel {
                        choice_combos: sections
                            .iter()
                            .flat_map(|section| section.props.iter())
                            .filter_map(|prop| match &prop.value {
                                crate::scene::object::PropValue::Choice { options, .. } => Some((
                                    prop.field.to_string(),
                                    iced::widget::combo_box::State::new(options.clone()),
                                )),
                                _ => None,
                            })
                            .collect(),
                        sections,
                        title: format!("{} objects selected", selected.len()),
                        selection_group_combo: iced::widget::combo_box::State::new(groups.clone()),
                        selection_groups: groups,
                        selected_group: active_group,
                        layer_combo: iced::widget::combo_box::State::new(layer_names.clone()),
                        linetype_combo: iced::widget::combo_box::State::new(linetype_items.clone()),
                        hatch_pattern_combo: iced::widget::combo_box::State::new(
                            crate::scene::hatch_patterns::names(),
                        ),
                        lineweight_combo: iced::widget::combo_box::State::new(
                            ui::properties::lw_options(),
                        ),
                        linetype_items,
                        ..Default::default()
                    }
                }
            };
            // Restore UI state.
            panel.color_picker_open = color_picker_open;
            panel.color_palette_open = color_palette_open;
            panel.edit_buf = edit_buf;
            panel
            // `selected` is dropped here, releasing the borrow on `self.tabs[i].scene`
        };

        self.tabs[i].properties = new_panel;
        self.refresh_selected_grips();
    }

    /// Rebuild the cached selected_grips from the current entity selection.
    fn refresh_selected_grips(&mut self) {
        let i = self.active_tab;
        let (new_handle, new_grips) = {
            let selected = self.tabs[i].scene.selected_entities();
            if selected.len() == 1 {
                let (handle, entity) = selected[0];
                let grips = dispatch::grips(entity);
                (Some(handle), grips)
            } else {
                (None, vec![])
            }
            // `selected` is dropped here
        };
        self.tabs[i].selected_handle = new_handle;
        self.tabs[i].selected_grips = new_grips;
    }

    fn property_target_handles(&self, i: usize) -> Vec<Handle> {
        let handles = self.tabs[i].properties.selected_handles();
        if !handles.is_empty() {
            handles
        } else {
            self.tabs[i].selected_handle.into_iter().collect()
        }
    }

    fn history_label_from_active_cmd(&self, i: usize, fallback: &'static str) -> String {
        self.tabs[i]
            .active_cmd
            .as_ref()
            .map(|cmd| cmd.name().to_string())
            .unwrap_or_else(|| fallback.to_string())
    }

    fn capture_history_snapshot(&self, i: usize, label: impl Into<String>) -> HistorySnapshot {
        HistorySnapshot {
            document: self.tabs[i].scene.document.clone(),
            current_layout: self.tabs[i].scene.current_layout.clone(),
            selected: self.tabs[i].scene.selected.iter().copied().collect(),
            dirty: self.tabs[i].dirty,
            label: label.into(),
        }
    }

    fn sync_ribbon_layers(&mut self) {
        let i = self.active_tab;
        let active = self.tabs[i].active_layer.clone();
        // Build LayerInfo from the panel (already synced from doc).
        let infos: Vec<crate::ui::ribbon::LayerInfo> = self.tabs[i]
            .layers
            .layers
            .iter()
            .map(|l| crate::ui::ribbon::LayerInfo {
                name: l.name.clone(),
                color: l.color,
                visible: l.visible,
                frozen: l.frozen,
                locked: l.locked,
            })
            .collect();
        let names: Vec<String> = infos.iter().map(|l| l.name.clone()).collect();
        // If the active layer no longer exists, fall back to "0".
        let active = if names.contains(&active) { active } else { "0".to_string() };
        self.tabs[i].active_layer = active.clone();
        self.tabs[i].layers.current_layer = active.clone();
        self.ribbon.set_layers(infos, &active);
        // Sync linetypes into the layer panel (using LinetypeItem with ASCII art)
        let lt_items: Vec<ui::properties::LinetypeItem> = self.tabs[i]
            .scene
            .document
            .line_types
            .iter()
            .map(|lt| {
                let name = if lt.name.eq_ignore_ascii_case("bylayer") {
                    "ByLayer".to_string()
                } else {
                    lt.name.clone()
                };
                let art = linetypes::extract_pattern(&lt.description);
                ui::properties::LinetypeItem { name, art }
            })
            .collect();
        self.tabs[i].layers.sync_linetypes(lt_items.clone());
        self.ribbon.set_available_linetypes(lt_items);
    }

    fn push_undo_snapshot(&mut self, i: usize, label: impl Into<String>) {
        let snapshot = self.capture_history_snapshot(i, label);
        self.tabs[i].history.undo_stack.push(snapshot);
        self.tabs[i].history.redo_stack.clear();
    }

    fn restore_history_snapshot(&mut self, i: usize, snapshot: HistorySnapshot) {
        self.tabs[i].scene.document = snapshot.document;
        self.tabs[i].scene.current_layout = snapshot.current_layout;
        self.tabs[i].scene.selected = snapshot
            .selected
            .into_iter()
            .filter(|h| self.tabs[i].scene.document.get_entity(*h).is_some())
            .collect::<HashSet<_>>();
        self.tabs[i].scene.populate_hatches_from_document();
        self.tabs[i].scene.clear_preview_wire();
        self.tabs[i].scene.meshes.clear();
        self.tabs[i].active_cmd = None;
        self.tabs[i].snap_result = None;
        self.tabs[i].active_grip = None;
        self.tabs[i].dirty = snapshot.dirty;
        let doc_layers = self.tabs[i].scene.document.layers.clone();
        self.tabs[i].layers.sync_from_doc(&doc_layers);
        self.sync_ribbon_layers();
        self.refresh_properties();
    }

    fn undo_active_tab(&mut self) {
        self.undo_steps(1);
    }

    fn redo_active_tab(&mut self) {
        self.redo_steps(1);
    }

    fn undo_steps(&mut self, steps: usize) {
        let i = self.active_tab;
        let available = self.tabs[i].history.undo_stack.len();
        let steps = steps.min(available);
        if steps == 0 {
            self.command_line.push_info("Nothing to undo.");
            return;
        }

        let mut last_label = String::new();
        for _ in 0..steps {
            let Some(snapshot) = self.tabs[i].history.undo_stack.pop() else {
                break;
            };
            let label = snapshot.label.clone();
            let current = self.capture_history_snapshot(i, label.clone());
            self.tabs[i].history.redo_stack.push(current);
            self.restore_history_snapshot(i, snapshot);
            last_label = label;
        }
        self.command_line.push_output(&format!("Undo: {last_label}"));
    }

    fn redo_steps(&mut self, steps: usize) {
        let i = self.active_tab;
        let available = self.tabs[i].history.redo_stack.len();
        let steps = steps.min(available);
        if steps == 0 {
            self.command_line.push_info("Nothing to redo.");
            return;
        }

        let mut last_label = String::new();
        for _ in 0..steps {
            let Some(snapshot) = self.tabs[i].history.redo_stack.pop() else {
                break;
            };
            let label = snapshot.label.clone();
            let current = self.capture_history_snapshot(i, label.clone());
            self.tabs[i].history.undo_stack.push(current);
            self.restore_history_snapshot(i, snapshot);
            last_label = label;
        }
        self.command_line.push_output(&format!("Redo: {last_label}"));
    }

    /// Add an entity to the correct space:
    /// - Viewport entities while in paper space → add_entity_to_layout
    /// - Everything else → model space (add_entity)
    fn commit_entity(&mut self, mut entity: acadrust::EntityType) {
        let i = self.active_tab;
        let layer = &self.tabs[i].active_layer;
        if layer != "0" || entity.as_entity().layer().is_empty() {
            entity.as_entity_mut().set_layer(layer.clone());
        }

        // Apply active color/linetype/lineweight from Properties toolbar.
        crate::scene::dispatch::apply_color(&mut entity, self.ribbon.active_color);
        crate::scene::dispatch::apply_common_prop(
            &mut entity,
            "linetype",
            &self.ribbon.active_linetype.clone(),
        );
        crate::scene::dispatch::apply_line_weight(&mut entity, self.ribbon.active_lineweight);

        if matches!(&entity, acadrust::EntityType::Viewport(_))
            && self.tabs[i].scene.current_layout != "Model"
        {
            let layout = self.tabs[i].scene.current_layout.clone();
            match self.tabs[i]
                .scene
                .document
                .add_entity_to_layout(entity, &layout)
            {
                Ok(_) => {}
                Err(e) => self
                    .command_line
                    .push_error(&format!("Viewport eklenemedi: {e}")),
            }
        } else {
            self.tabs[i].scene.add_entity(entity);
        }
    }

    fn apply_cmd_result(&mut self, result: CmdResult) -> Task<Message> {
        let i = self.active_tab;
        match result {
            CmdResult::NeedPoint => {
                let prompt = self.tabs[i].active_cmd.as_ref().map(|c| c.prompt());
                if let Some(p) = prompt {
                    self.command_line.push_info(&p);
                }
            }
            CmdResult::Preview(wire) => {
                self.tabs[i].scene.set_preview_wires(vec![wire]);
                let prompt = self.tabs[i].active_cmd.as_ref().map(|c| c.prompt());
                if let Some(p) = prompt {
                    self.command_line.push_info(&p);
                }
            }
            CmdResult::InterimWire(wire) => {
                self.tabs[i].scene.set_interim_wire(wire);
                let prompt = self.tabs[i].active_cmd.as_ref().map(|c| c.prompt());
                if let Some(p) = prompt {
                    self.command_line.push_info(&p);
                }
            }
            CmdResult::CommitEntity(entity) => {
                let label = self.history_label_from_active_cmd(i, "ENTITY");
                self.push_undo_snapshot(i, label);
                self.commit_entity(entity);
                self.tabs[i].dirty = true;
                let prompt = self.tabs[i].active_cmd.as_ref().map(|c| c.prompt());
                if let Some(p) = prompt {
                    self.command_line.push_info(&p);
                }
            }
            CmdResult::TransformSelected(handles, transform) => {
                let label = self.history_label_from_active_cmd(i, "MOVE");
                self.push_undo_snapshot(i, label);
                self.tabs[i].scene.transform_entities(&handles, &transform);
                self.tabs[i].dirty = true;
                self.tabs[i].scene.clear_preview_wire();
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                if let Some(was_on) = self.pre_cmd_tangent.take() {
                    if !was_on {
                        self.snapper.enabled.remove(&crate::snap::SnapType::Tangent);
                    }
                }
                self.refresh_properties();
            }
            CmdResult::CopySelected(handles, transform) => {
                let label = self.history_label_from_active_cmd(i, "COPY");
                self.push_undo_snapshot(i, label);
                let new_handles = self.tabs[i].scene.copy_entities(&handles, &transform);
                self.tabs[i].dirty = true;
                self.tabs[i].scene.deselect_all();
                for h in new_handles {
                    self.tabs[i].scene.select_entity(h, false);
                }
                self.tabs[i].scene.clear_preview_wire();
                let prompt = self.tabs[i].active_cmd.as_ref().map(|c| c.prompt());
                if let Some(p) = prompt {
                    self.command_line.push_info(&p);
                }
                self.refresh_properties();
            }
            CmdResult::CommitAndExit(entity) => {
                let label = self.history_label_from_active_cmd(i, "ENTITY");
                self.push_undo_snapshot(i, label);
                self.commit_entity(entity);
                self.tabs[i].dirty = true;
                self.tabs[i].scene.clear_preview_wire();
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                if let Some(was_on) = self.pre_cmd_tangent.take() {
                    if !was_on {
                        self.snapper.enabled.remove(&crate::snap::SnapType::Tangent);
                    }
                }
            }
            CmdResult::CreateBlock {
                handles,
                name,
                base,
            } => {
                self.push_undo_snapshot(i, "BLOCK");
                match self.tabs[i].scene.create_block_from_entities(&handles, &name, base) {
                    Ok(insert_handle) => {
                    self.tabs[i].dirty = true;
                    self.tabs[i].scene.deselect_all();
                    if !insert_handle.is_null() {
                        self.tabs[i].scene.select_entity(insert_handle, false);
                    }
                    self.tabs[i].scene.clear_preview_wire();
                    self.tabs[i].active_cmd = None;
                    self.tabs[i].snap_result = None;
                    self.command_line
                        .push_output(&format!("Block \"{name}\" created."));
                    self.refresh_properties();
                    }
                    Err(err) => {
                        let _ = self.tabs[i].history.undo_stack.pop();
                        self.command_line.push_error(&err);
                        let prompt = self.tabs[i].active_cmd.as_ref().map(|c| c.prompt());
                        if let Some(p) = prompt {
                            self.command_line.push_info(&p);
                        }
                    }
                }
            }
            CmdResult::CommitHatch(hatch) => {
                let label = self.history_label_from_active_cmd(i, "HATCH");
                self.push_undo_snapshot(i, label);
                let new_handle = self.tabs[i].scene.add_hatch(hatch);
                // Select the newly created hatch so Properties reflects it immediately.
                if !new_handle.is_null() {
                    self.tabs[i].scene.select_entity(new_handle, true);
                }
                self.tabs[i].dirty = true;
                self.tabs[i].scene.clear_preview_wire();
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                if let Some(was_on) = self.pre_cmd_tangent.take() {
                    if !was_on {
                        self.snapper.enabled.remove(&crate::snap::SnapType::Tangent);
                    }
                }
                self.refresh_properties();
            }
            CmdResult::BatchCopy(handles, transforms) => {
                let label = self.history_label_from_active_cmd(i, "ARRAY");
                self.push_undo_snapshot(i, label);
                let count = transforms.len();
                for t in &transforms {
                    self.tabs[i].scene.copy_entities(&handles, t);
                }
                self.tabs[i].dirty = true;
                self.tabs[i].scene.clear_preview_wire();
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                if let Some(was_on) = self.pre_cmd_tangent.take() {
                    if !was_on {
                        self.snapper.enabled.remove(&crate::snap::SnapType::Tangent);
                    }
                }
                self.command_line
                    .push_output(&format!("ARRAY: {count} copies created."));
                self.refresh_properties();
            }
            CmdResult::ReplaceMany(replacements, additions) => {
                let label = self.history_label_from_active_cmd(i, "FILLET");
                self.push_undo_snapshot(i, label);
                for (handle, entities) in replacements {
                    self.tabs[i].scene.erase_entities(&[handle]);
                    for entity in entities {
                        self.tabs[i].scene.add_entity(entity);
                    }
                }
                for entity in additions {
                    self.tabs[i].scene.add_entity(entity);
                }
                self.tabs[i].dirty = true;
                self.tabs[i].scene.clear_preview_wire();
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.refresh_properties();
            }
            CmdResult::ReplaceEntity(handle, new_entities) => {
                let label = self.history_label_from_active_cmd(i, "TRIM");
                self.push_undo_snapshot(i, label);
                self.tabs[i].scene.erase_entities(&[handle]);
                let new_handles: Vec<acadrust::Handle> = new_entities
                    .into_iter()
                    .map(|e| self.tabs[i].scene.add_entity(e))
                    .collect();
                if let Some(cmd) = &mut self.tabs[i].active_cmd {
                    cmd.on_entity_replaced(handle, &new_handles);
                }
                self.tabs[i].dirty = true;
                let prompt = self.tabs[i].active_cmd.as_ref().map(|c| c.prompt());
                if let Some(p) = prompt {
                    self.command_line.push_info(&p);
                }
            }
            CmdResult::Cancel => {
                self.tabs[i].scene.clear_preview_wire();
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                if let Some(was_on) = self.pre_cmd_tangent.take() {
                    if !was_on {
                        self.snapper.enabled.remove(&crate::snap::SnapType::Tangent);
                    }
                }
                self.command_line.push_info("Command cancelled.");
            }
            CmdResult::Relaunch(cmd, handles) => {
                // Install the gathered selection then re-dispatch the original command.
                self.tabs[i].scene.deselect_all();
                for h in &handles {
                    self.tabs[i].scene.select_entity(*h, false);
                }
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                if let Some(was_on) = self.pre_cmd_tangent.take() {
                    if !was_on {
                        self.snapper.enabled.remove(&crate::snap::SnapType::Tangent);
                    }
                }
                let _ = self.dispatch_command(&cmd);
            }
            CmdResult::MatchEntityLayer { dest, src } => {
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                // Look up source entity's layer name.
                let src_layer = self.tabs[i].scene.document
                    .get_entity(src)
                    .map(|e| e.common().layer.clone());
                if let Some(layer) = src_layer {
                    self.push_undo_snapshot(i, "LAYMATCH");
                    for h in &dest {
                        if let Some(e) = self.tabs[i].scene.document.get_entity_mut(*h) {
                            e.as_entity_mut().set_layer(layer.clone());
                        }
                    }
                    self.tabs[i].dirty = true;
                    self.command_line.push_info(&format!("Layer matched to \"{layer}\"."));
                    self.sync_ribbon_layers();
                } else {
                    self.command_line.push_error("Source object not found.");
                }
            }
            CmdResult::MatchProperties { dest, src } => {
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();

                // Read all visual properties from source entity.
                let props = self.tabs[i].scene.document.get_entity(src).map(|e| {
                    let c = e.common();
                    (
                        c.layer.clone(),
                        c.color,
                        c.linetype.clone(),
                        c.linetype_scale,
                        c.line_weight,
                    )
                });

                if let Some((layer, color, linetype, lt_scale, lw)) = props {
                    self.push_undo_snapshot(i, "MATCHPROP");
                    for h in &dest {
                        if let Some(e) = self.tabs[i].scene.document.get_entity_mut(*h) {
                            e.as_entity_mut().set_layer(layer.clone());
                            crate::scene::dispatch::apply_color(e, color);
                            crate::scene::dispatch::apply_line_weight(e, lw);
                            e.common_mut().linetype = linetype.clone();
                            e.common_mut().linetype_scale = lt_scale;
                        }
                    }
                    self.tabs[i].dirty = true;
                    self.refresh_properties();
                    self.command_line.push_info(
                        &format!("Properties matched to {} object(s).", dest.len())
                    );
                } else {
                    self.command_line.push_error("Source object not found.");
                }
            }
            CmdResult::PasteClipboard { base_pt } => {
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                if self.clipboard.is_empty() {
                    self.command_line.push_error("Clipboard is empty.");
                } else {
                    let delta = base_pt - self.clipboard_centroid;
                    let translate = command::EntityTransform::Translate(delta);
                    self.push_undo_snapshot(i, "PASTECLIP");
                    let count = self.clipboard.len();
                    let new_handles: Vec<Handle> = self.clipboard.clone()
                        .into_iter()
                        .map(|mut entity| {
                            crate::scene::dispatch::apply_transform(&mut entity, &translate);
                            entity.common_mut().handle = acadrust::Handle::NULL;
                            self.tabs[i].scene.add_entity(entity)
                        })
                        .filter(|h| !h.is_null())
                        .collect();
                    self.tabs[i].scene.deselect_all();
                    for h in new_handles {
                        self.tabs[i].scene.select_entity(h, false);
                    }
                    self.tabs[i].dirty = true;
                    self.refresh_properties();
                    self.command_line.push_info(&format!("{count} object(s) pasted."));
                }
            }
            CmdResult::CreateGroup { handles, name } => {
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                self.push_undo_snapshot(i, "GROUP");
                self.tabs[i].scene.create_group(name.clone(), handles);
                self.tabs[i].dirty = true;
                self.command_line.push_info(&format!("Group \"{}\" created.", name));
            }
            CmdResult::DeleteGroups { handles } => {
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                self.push_undo_snapshot(i, "UNGROUP");
                let count = self.tabs[i].scene.delete_groups_containing(&handles);
                self.tabs[i].dirty = true;
                if count > 0 {
                    self.command_line.push_info(&format!("{} group(s) dissolved.", count));
                } else {
                    self.command_line.push_info("No groups found for selected objects.");
                }
            }
        }
        // Focus the command-line input while a command is active; blur it when the command ends.
        if self.tabs[i].active_cmd.is_some() {
            self.focus_cmd_input()
        } else {
            self.blur_cmd_input()
        }
    }

    /// Compute the centroid of a set of wire models (average of all points).
    fn entities_centroid(wires: &[scene::WireModel]) -> glam::Vec3 {
        let mut sum = glam::Vec3::ZERO;
        let mut count = 0usize;
        for w in wires {
            for p in &w.points {
                sum += glam::Vec3::from(*p);
                count += 1;
            }
        }
        if count > 0 { sum / count as f32 } else { glam::Vec3::ZERO }
    }

    /// Generate the next available auto group name ("*A1", "*A2", …).
    fn next_group_auto_name(scene: &scene::Scene) -> String {
        let existing: HashSet<String> = scene.groups().map(|g| g.name.clone()).collect();
        for n in 1..=9999 {
            let name = format!("*A{n}");
            if !existing.contains(&name) {
                return name;
            }
        }
        "*A".to_string()
    }

    fn focus_cmd_input(&self) -> Task<Message> {
        iced::widget::operation::focus(iced::widget::Id::new(ui::command_line::CMD_INPUT_ID))
    }

    fn blur_cmd_input(&self) -> Task<Message> {
        let op = iced::advanced::widget::operation::focusable::unfocus::<Message>();
        iced::advanced::widget::operate(op)
    }

    fn view(&self, window_id: window::Id) -> Element<'_, Message> {
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

            // Compute grip markers when a single entity is selected and no command active.
            let grips: Vec<ui::overlay::GripMarker> =
                if tab.active_cmd.is_none() && !tab.selected_grips.is_empty() {
                    let (vw, vh) = tab.scene.selection.borrow().vp_size;
                    let bounds = iced::Rectangle {
                        x: 0.0,
                        y: 0.0,
                        width: vw,
                        height: vh,
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
                            ui::overlay::GripMarker {
                                pos: screen,
                                shape,
                                is_hot,
                            }
                        })
                        .collect()
                } else {
                    vec![]
                };

            let grid = if self.show_grid {
                let (vw, vh) = tab.scene.selection.borrow().vp_size;
                let bounds = iced::Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: vw,
                    height: vh,
                };
                let cam = tab.scene.camera.borrow();
                let plane = grid_plane_from_camera(cam.pitch, cam.yaw);
                Some(overlay::GridParams {
                    view_proj: cam.view_proj(bounds),
                    bounds,
                    plane,
                })
            } else {
                None
            };
            overlay::selection_overlay(sel, snap_info, grips, grid)
        };

        let nav = container(overlay::nav_toolbar())
            .align_right(Fill)
            .align_top(Fill)
            .padding(iced::Padding {
                top: 148.0,
                right: 8.0,
                bottom: 0.0,
                left: 0.0,
            });

        let info = container(overlay::info_bar(
            if is_paper {
                &tab.scene.current_layout
            } else {
                "Custom View"
            },
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
            .padding(iced::Padding {
                top: VIEWCUBE_PAD,
                right: VIEWCUBE_PAD,
                bottom: 0.0,
                left: 0.0,
            })
            .width(Fill)
            .height(Fill);

        let bg_color = if is_paper {
            Color {
                r: 0.22,
                g: 0.24,
                b: 0.28,
                a: 1.0,
            } // slightly blue-grey for paper space
        } else {
            Color {
                r: 0.11,
                g: 0.11,
                b: 0.11,
                a: 1.0,
            } // dark for model space
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
        ];

        let viewport_stack = viewport_stack.width(Fill).height(Fill);
        let center_stack = iced::widget::stack![
            row![tab.properties.view(), viewport_stack]
                .width(Fill)
                .height(Fill),
        ]
        .width(Fill)
        .height(Fill);
        let center = center_stack;

        // Document tab bar — ribbon ile center arasında tam genişlikte
        let tab_bar = doc_tab_bar(&self.tabs, self.active_tab);

        let main_ui = container(
            column![
                self.ribbon.view(
                    is_paper,
                    self.tabs[self.active_tab].history.undo_stack.len(),
                    self.tabs[self.active_tab].history.redo_stack.len(),
                ),
                tab_bar,
                center,
                self.command_line.view(),
                self.status_bar.view(
                    &self.snapper,
                    self.snap_popup_open,
                    self.ortho_mode,
                    self.polar_mode,
                    self.show_grid,
                    tab.scene.layout_names(),
                    tab.scene.current_layout.clone()
                )
            ]
            .width(Fill)
            .height(Fill),
        )
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color {
                r: 0.11,
                g: 0.11,
                b: 0.11,
                a: 1.0,
            })),
            ..Default::default()
        })
        .width(Fill)
        .height(Fill);

        let snap_layer: Element<'_, Message> = if self.snap_popup_open {
            ui::snap_popup::snap_popup_overlay(&self.snapper, 4.0)
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

        stack![main_ui, self.app_menu.view(), snap_layer, dropdown_layer].into()
    }

    fn subscription(&self) -> Subscription<Message> {
        use iced::{event, keyboard};
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
                            // ── File shortcuts ────────────────────────────
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
}

impl Default for H7CAD {
    fn default() -> Self {
        Self::new()
    }
}

fn history_dropdown_labels(stack: &[HistorySnapshot]) -> Vec<String> {
    stack.iter().rev().map(|snapshot| snapshot.label.clone()).collect()
}

// ── Grid plane detection ───────────────────────────────────────────────────

/// Choose the grid plane whose normal is most aligned with the camera view direction.
/// Switches at the 45° octant boundary — no hysteresis needed since the grid is
/// a visual aid and not selection-sensitive.
fn grid_plane_from_camera(pitch: f32, yaw: f32) -> ui::overlay::GridPlane {
    use ui::overlay::GridPlane;
    // Camera forward direction components (Z-up, see camera.rs):
    //   forward.x = -cos(pitch) * sin(yaw)
    //   forward.y = -cos(pitch) * cos(yaw)
    //   forward.z = -sin(pitch)
    let fz = pitch.sin().abs(); // Z-axis alignment → XY plane
    let fy = (pitch.cos() * yaw.cos()).abs(); // Y-axis alignment → XZ plane
    let fx = (pitch.cos() * yaw.sin()).abs(); // X-axis alignment → YZ plane
    if fz >= fy && fz >= fx {
        GridPlane::Xy
    } else if fy >= fx {
        GridPlane::Xz
    } else {
        GridPlane::Yz
    }
}

// ── Drawing constraint helpers ─────────────────────────────────────────────

/// Constrain `pt` to the nearest 90° direction from `base` (XY plane, Z-up).
fn ortho_constrain(pt: glam::Vec3, base: glam::Vec3) -> glam::Vec3 {
    let dx = (pt.x - base.x).abs();
    let dy = (pt.y - base.y).abs();
    if dx >= dy {
        glam::Vec3::new(pt.x, base.y, pt.z) // horizontal
    } else {
        glam::Vec3::new(base.x, pt.y, pt.z) // vertical
    }
}

/// Constrain `pt` to the nearest polar angle multiple from `base` (XY plane, Z-up).
fn polar_constrain(pt: glam::Vec3, base: glam::Vec3, step_deg: f32) -> glam::Vec3 {
    let dx = pt.x - base.x;
    let dy = pt.y - base.y;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 1e-6 {
        return pt;
    }
    let step = step_deg.to_radians();
    let angle = dy.atan2(dx);
    let snapped = (angle / step).round() * step;
    glam::Vec3::new(
        base.x + dist * snapped.cos(),
        base.y + dist * snapped.sin(),
        pt.z,
    )
}

// ── Document tab bar ───────────────────────────────────────────────────────

fn doc_tab_bar<'a>(tabs: &'a [DocumentTab], active_tab: usize) -> Element<'a, Message> {
    const BAR_BG: Color = Color {
        r: 0.13,
        g: 0.13,
        b: 0.13,
        a: 1.0,
    };
    const TAB_ACTIVE: Color = Color {
        r: 0.22,
        g: 0.22,
        b: 0.22,
        a: 1.0,
    };
    const TAB_HOVER: Color = Color {
        r: 0.18,
        g: 0.18,
        b: 0.18,
        a: 1.0,
    };
    const TAB_INACTIVE: Color = Color {
        r: 0.13,
        g: 0.13,
        b: 0.13,
        a: 1.0,
    };
    const ACCENT: Color = Color {
        r: 0.20,
        g: 0.55,
        b: 0.90,
        a: 1.0,
    };
    const TEXT_ACTIVE: Color = Color::WHITE;
    const TEXT_INACTIVE: Color = Color {
        r: 0.60,
        g: 0.60,
        b: 0.60,
        a: 1.0,
    };
    const CLOSE_HOVER: Color = Color {
        r: 0.70,
        g: 0.22,
        b: 0.22,
        a: 1.0,
    };
    const BORDER_COLOR: Color = Color {
        r: 0.25,
        g: 0.25,
        b: 0.25,
        a: 1.0,
    };

    let mut bar = Row::new().spacing(0).align_y(iced::Center);

    for (idx, tab) in tabs.iter().enumerate() {
        let is_active = idx == active_tab;
        let name = tab.tab_display_name();
        let label = if tab.dirty {
            format!("● {}", name)
        } else {
            name
        };

        let title_btn = button(text(label).size(12))
            .on_press(Message::TabSwitch(idx))
            .padding([5, 12])
            .style(move |_: &Theme, status| button::Style {
                background: Some(Background::Color(match (is_active, status) {
                    (true, _) => TAB_ACTIVE,
                    (false, button::Status::Hovered) => TAB_HOVER,
                    _ => TAB_INACTIVE,
                })),
                text_color: if is_active {
                    TEXT_ACTIVE
                } else {
                    TEXT_INACTIVE
                },
                border: Border {
                    color: if is_active {
                        ACCENT
                    } else {
                        Color::TRANSPARENT
                    },
                    width: if is_active { 1.0 } else { 0.0 },
                    radius: 0.0.into(),
                },
                shadow: iced::Shadow::default(),
                snap: false,
            });

        // Show close button only when there are multiple tabs, or always show it
        let close_btn = button(text("×").size(11).color(Color {
            r: 0.55,
            g: 0.55,
            b: 0.55,
            a: 1.0,
        }))
        .on_press(Message::TabClose(idx))
        .padding([3, 5])
        .style(move |_: &Theme, status| button::Style {
            background: Some(Background::Color(match status {
                button::Status::Hovered => CLOSE_HOVER,
                _ => {
                    if is_active {
                        TAB_ACTIVE
                    } else {
                        TAB_INACTIVE
                    }
                }
            })),
            border: Border {
                radius: 3.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

        bar = bar.push(
            container(row![title_btn, close_btn].spacing(0).align_y(iced::Center)).style(
                move |_: &Theme| container::Style {
                    border: Border {
                        color: if is_active {
                            BORDER_COLOR
                        } else {
                            Color::TRANSPARENT
                        },
                        width: if is_active { 1.0 } else { 0.0 },
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                },
            ),
        );
    }

    // "+" new tab button
    let new_btn = button(text("+").size(14).color(Color {
        r: 0.65,
        g: 0.65,
        b: 0.65,
        a: 1.0,
    }))
    .on_press(Message::TabNew)
    .padding([4, 10])
    .style(|_: &Theme, status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered => TAB_HOVER,
            _ => Color::TRANSPARENT,
        })),
        border: Border {
            radius: 0.0.into(),
            ..Default::default()
        },
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

// ── Window icon ────────────────────────────────────────────────────────────

/// Builds a 32×32 RGBA icon: red background with H7 drawn in white pixels.
fn build_window_icon() -> Vec<u8> {
    const W: usize = 32;
    const SZ: usize = W * W * 4;

    let bg = [176u8, 48, 32, 255];
    let fg = [255u8, 255, 255, 255];

    let mut px = vec![0u8; SZ];
    for i in 0..W * W {
        px[i * 4..i * 4 + 4].copy_from_slice(&bg);
    }

    // Draw a 3px-wide line from (ax,ay) to (bx,by)
    fn stroke(px: &mut Vec<u8>, ax: i32, ay: i32, bx: i32, by: i32, fg: [u8; 4]) {
        let steps = ((bx - ax).abs().max((by - ay).abs()) * 3).max(1);
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let cx = ax as f32 + (bx - ax) as f32 * t;
            let cy = ay as f32 + (by - ay) as f32 * t;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let ix = cx.round() as i32 + dx;
                    let iy = cy.round() as i32 + dy;
                    if ix >= 0 && ix < W as i32 && iy >= 0 && iy < W as i32 {
                        let idx = (iy as usize * W + ix as usize) * 4;
                        px[idx..idx + 4].copy_from_slice(&fg);
                    }
                }
            }
        }
    }

    // H
    stroke(&mut px, 4, 5, 4, 26, fg);
    stroke(&mut px, 13, 5, 13, 26, fg);
    stroke(&mut px, 4, 15, 13, 15, fg);
    // 7
    stroke(&mut px, 17, 5, 27, 5, fg);
    stroke(&mut px, 27, 5, 20, 26, fg);
    stroke(&mut px, 20, 16, 26, 16, fg);

    px
}

// ── Standalone helpers ────────────────────────────────────────────────────

fn entity_type_label(entity: &acadrust::EntityType) -> String {
    use acadrust::EntityType::*;
    match entity {
        Line(_) => "Line",
        Circle(_) => "Circle",
        Arc(_) => "Arc",
        Ellipse(_) => "Ellipse",
        Spline(_) => "Spline",
        LwPolyline(_) => "Polyline",
        Text(_) => "Text",
        MText(_) => "MText",
        Dimension(_) => "Dimension",
        Insert(_) => "Block Reference",
        Point(_) => "Point",
        Hatch(_) => "Hatch",
        _ => "Entity",
    }
    .to_string()
}

fn entity_type_key(entity: &acadrust::EntityType) -> String {
    match entity {
        acadrust::EntityType::LwPolyline(_) => "pline",
        acadrust::EntityType::Circle(_) => "circle",
        acadrust::EntityType::Line(_) => "line",
        acadrust::EntityType::Arc(_) => "arc",
        acadrust::EntityType::Ellipse(_) => "ellipse",
        acadrust::EntityType::Spline(_) => "spline",
        acadrust::EntityType::Text(_) => "text",
        acadrust::EntityType::MText(_) => "mtext",
        acadrust::EntityType::Dimension(_) => "dimension",
        acadrust::EntityType::Insert(_) => "insert",
        acadrust::EntityType::Point(_) => "point",
        acadrust::EntityType::Hatch(_) => "hatch",
        _ => "entity",
    }
    .to_string()
}

fn build_selection_groups(
    selected: &[(Handle, &EntityType)],
) -> Vec<ui::properties::SelectionGroup> {
    let mut groups = vec![ui::properties::SelectionGroup {
        label: format!("All({})", selected.len()),
        handles: selected.iter().map(|(handle, _)| *handle).collect(),
    }];

    let mut by_type: std::collections::BTreeMap<String, Vec<Handle>> = std::collections::BTreeMap::new();
    for (handle, entity) in selected {
        by_type.entry(entity_type_key(entity)).or_default().push(*handle);
    }

    for (kind, handles) in by_type {
        groups.push(ui::properties::SelectionGroup {
            label: format!("{}({})", title_case_word(&kind), handles.len()),
            handles,
        });
    }

    groups
}

fn title_case_word(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => {
            let mut out = first.to_uppercase().collect::<String>();
            out.push_str(chars.as_str());
            out
        }
        None => String::new(),
    }
}

fn aggregate_sections(
    selected: &[(Handle, &EntityType)],
    text_style_names: &[String],
) -> Vec<crate::scene::object::PropSection> {
    if selected.is_empty() {
        return vec![];
    }

    let mut all_sections: Vec<Vec<crate::scene::object::PropSection>> = selected
        .iter()
        .map(|(handle, entity)| dispatch::properties_sectioned(*handle, entity, text_style_names))
        .collect();

    let mut result = all_sections.remove(0);
    for sections in all_sections {
        result = merge_sections(&result, &sections);
    }
    result
}

fn merge_sections(
    left: &[crate::scene::object::PropSection],
    right: &[crate::scene::object::PropSection],
) -> Vec<crate::scene::object::PropSection> {
    left.iter()
        .filter_map(|section| {
            let rhs = right.iter().find(|candidate| candidate.title == section.title)?;
            let props: Vec<crate::scene::object::Property> = section
                .props
                .iter()
                .filter_map(|prop| {
                    let other = rhs.props.iter().find(|candidate| candidate.field == prop.field)?;
                    Some(crate::scene::object::Property {
                        label: prop.label.clone(),
                        field: prop.field,
                        value: merge_prop_value(&prop.value, &other.value),
                    })
                })
                .collect();
            if props.is_empty() {
                None
            } else {
                Some(crate::scene::object::PropSection {
                    title: section.title.clone(),
                    props,
                })
            }
        })
        .collect()
}

fn merge_prop_value(
    left: &crate::scene::object::PropValue,
    right: &crate::scene::object::PropValue,
) -> crate::scene::object::PropValue {
    use crate::scene::object::PropValue;

    if left == right {
        return left.clone();
    }

    match (left, right) {
        (PropValue::LayerChoice(_), PropValue::LayerChoice(_)) => {
            PropValue::LayerChoice(VARIES_LABEL.into())
        }
        (PropValue::ColorChoice(_), PropValue::ColorChoice(_))
        | (PropValue::ColorVaries, _)
        | (_, PropValue::ColorVaries) => PropValue::ColorVaries,
        (PropValue::LwChoice(_), PropValue::LwChoice(_))
        | (PropValue::LwVaries, _)
        | (_, PropValue::LwVaries) => PropValue::LwVaries,
        (PropValue::LinetypeChoice(_), PropValue::LinetypeChoice(_)) => {
            PropValue::LinetypeChoice(VARIES_LABEL.into())
        }
        (
            PropValue::Choice { options, .. },
            PropValue::Choice {
                options: other_options,
                ..
            },
        ) if options == other_options => PropValue::Choice {
            selected: VARIES_LABEL.into(),
            options: options.clone(),
        },
        (PropValue::EditText(_), PropValue::EditText(_)) => {
            PropValue::EditText(VARIES_LABEL.into())
        }
        (PropValue::ReadOnly(_), PropValue::ReadOnly(_)) => {
            PropValue::ReadOnly(VARIES_LABEL.into())
        }
        (PropValue::HatchPatternChoice(_), PropValue::HatchPatternChoice(_)) => {
            PropValue::HatchPatternChoice(VARIES_LABEL.into())
        }
        (
            PropValue::BoolToggle { field, .. },
            PropValue::BoolToggle {
                field: other_field, ..
            },
        ) if field == other_field => PropValue::ReadOnly(VARIES_LABEL.into()),
        _ => left.clone(),
    }
}
