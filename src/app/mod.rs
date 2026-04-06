mod document;
mod helpers;
mod history;
mod properties;
mod layers;
mod commands;
mod cmd_result;
mod view;
mod update;

use document::DocumentTab;

use acadrust::types::{Color as AcadColor, LineWeight};
use acadrust::CadDocument;
use crate::modules::ModuleEvent;
use crate::scene::CubeRegion;
use crate::snap::Snapper;
use crate::ui::{AppMenu, CommandLine, Ribbon, StatusBar};

use iced::time::Instant;
use iced::window;
use iced::{mouse, Point, Task, Theme};

pub(super) const POLY_START_DELAY_MS: u128 = 150;
pub(super) const VARIES_LABEL: &str = "*VARIES*";

// ── Application state ──────────────────────────────────────────────────────

pub(super) struct H7CAD {
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
    /// Show the UCS icon in the bottom-left corner of model space (UCSICON).
    show_ucs_icon: bool,
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
    /// Which layout tab has its context menu open (None = closed).
    layout_context_menu: Option<String>,
    /// Inline rename state: (original_name, current_edit_value).
    layout_rename_state: Option<(String, String)>,
    /// Timestamp of the previous viewport left-click release (for double-click detection).
    last_vp_click_time: Option<Instant>,
    /// Screen position of the previous viewport left-click release.
    last_vp_click_pos: Option<Point>,
    /// Page Setup overlay open/closed.
    page_setup_open: bool,
    /// Editable paper width buffer for the Page Setup panel (string while typing).
    page_setup_w: String,
    /// Editable paper height buffer for the Page Setup panel (string while typing).
    page_setup_h: String,
    /// Plot area type: "Layout" | "Extents".
    page_setup_plot_area: String,
    /// Center the drawing on the page when exporting.
    page_setup_center: bool,
    /// Plot offset X in mm (applied after optional centering).
    page_setup_offset_x: String,
    /// Plot offset Y in mm.
    page_setup_offset_y: String,
    /// Plot rotation in degrees: "0" | "90" | "180" | "270".
    page_setup_rotation: String,
    /// Plot scale: "Fit" | "1:1" | "1:2" | "1:4" | "1:5" | "1:10" | "1:20" | "1:50" | "1:100" | "2:1".
    page_setup_scale: String,

    // ── Plot Style Table ──────────────────────────────────────────────────
    /// Currently loaded CTB/STB table (None = no override).
    active_plot_style: Option<crate::io::plot_style::PlotStyleTable>,

    // ── MLineStyle Dialog ─────────────────────────────────────────────────
    mlstyle_open: bool,
    mlstyle_selected: String,

    // ── DimStyle Dialog ───────────────────────────────────────────────────
    dimstyle_open: bool,
    /// Name of the style currently shown in the dialog.
    dimstyle_selected: String,
    /// Active tab: 0=Lines, 1=Arrows, 2=Text, 3=Scale/Units, 4=Tolerances.
    dimstyle_tab: u8,
    // Edit buffers (strings while typing):
    ds_dimdle: String, ds_dimdli: String, ds_dimgap: String,
    ds_dimexe: String, ds_dimexo: String,
    ds_dimsd1: bool,   ds_dimsd2: bool,
    ds_dimse1: bool,   ds_dimse2: bool,
    ds_dimasz: String, ds_dimcen: String, ds_dimtsz: String,
    ds_dimtxt: String, ds_dimtxsty: String, ds_dimtad: String,
    ds_dimtih: bool,   ds_dimtoh: bool,
    ds_dimscale: String, ds_dimlfac: String,
    ds_dimlunit: String, ds_dimdec: String, ds_dimpost: String,
    ds_dimtol: bool,   ds_dimlim: bool,
    ds_dimtp: String,  ds_dimtm: String,
    ds_dimtdec: String, ds_dimtfac: String,
}

/// Identifies a DimStyle field that can be edited in the dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DsField {
    Dimdle, Dimdli, Dimgap, Dimexe, Dimexo,
    Dimsd1, Dimsd2, Dimse1, Dimse2,
    Dimasz, Dimcen, Dimtsz,
    Dimtxt, Dimtxsty, Dimtad, Dimtih, Dimtoh,
    Dimscale, Dimlfac, Dimlunit, Dimdec, Dimpost,
    Dimtol, Dimlim, Dimtp, Dimtm, Dimtdec, Dimtfac,
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
    ToggleSnap(crate::snap::SnapType),
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
    PropSelectionGroupChanged(crate::ui::properties::SelectionGroup),
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
    /// Enter the model-space editing mode inside the given viewport (MSPACE).
    EnterViewport(acadrust::Handle),
    /// Exit MSPACE and return to paper-space editing (PSPACE).
    ExitViewport,
    /// MS command: enter MSPACE for the first available viewport.
    MspaceCommand,
    /// PS command: exit MSPACE (PSPACE).
    PspaceCommand,
    /// Switch to a named layout ("Model" or paper space layout name).
    LayoutSwitch(String),
    /// Create a new paper space layout.
    LayoutCreate,
    /// Delete the named paper space layout (Model cannot be deleted).
    LayoutDelete(String),
    /// Begin inline rename for the given layout tab.
    LayoutRenameStart(String),
    /// Live-update the rename text input buffer.
    LayoutRenameEdit(String),
    /// Commit the rename (Enter pressed in the text input).
    LayoutRenameCommit,
    /// Cancel an in-progress rename (Escape).
    LayoutRenameCancel,
    /// Open the right-click context menu for the given layout tab.
    LayoutContextMenu(String),
    /// Close the layout context menu.
    LayoutContextMenuClose,
    /// A window was closed by the OS (e.g. the user clicked the title-bar ✕).
    OsWindowClosed(window::Id),
    /// No-op — used as a fallback when a TabEvent has no host mapping.
    Noop,
    // ── Page Setup ────────────────────────────────────────────────────────
    /// Open the Page Setup panel for the current layout.
    PageSetupOpen,
    /// Close (cancel) the Page Setup panel without applying changes.
    PageSetupClose,
    /// Live-edit of the paper width field.
    PageSetupWidthEdit(String),
    /// Live-edit of the paper height field.
    PageSetupHeightEdit(String),
    /// User selected a paper size preset (e.g. "A4 Portrait").
    PageSetupPreset(String),
    /// User changed the plot area type ("Layout" or "Extents").
    PageSetupPlotArea(String),
    /// Toggle center-on-page.
    PageSetupCenterToggle,
    /// Live-edit of plot offset X.
    PageSetupOffsetXEdit(String),
    /// Live-edit of plot offset Y.
    PageSetupOffsetYEdit(String),
    /// User changed plot rotation.
    PageSetupRotation(String),
    PageSetupScale(String),
    /// Apply the changes entered in Page Setup.
    PageSetupCommit,
    // ── Plot / Export ─────────────────────────────────────────────────────
    /// Show the SVG save-file dialog and trigger export.
    PlotExport,
    /// Callback after the user picks (or cancels) the export path.
    PlotExportPath(Option<std::path::PathBuf>),
    // ── Plot Style Table ─────────────────────────────────────────────────
    /// Open file dialog to load a CTB/STB plot style table.
    PlotStyleLoad,
    /// Callback when the user picks (or cancels) a CTB/STB file.
    PlotStyleLoaded(Option<crate::io::plot_style::PlotStyleTable>),
    /// Clear the active plot style table.
    PlotStyleClear,
    // ── MLineStyle Dialog ─────────────────────────────────────────────────
    MlStyleDialogOpen,
    MlStyleDialogClose,
    MlStyleDialogSelect(String),
    MlStyleDialogSetCurrent,
    MlStyleDialogNew,
    MlStyleDialogDelete,
    // ── DimStyle Dialog ───────────────────────────────────────────────────
    DimStyleDialogOpen,
    DimStyleDialogClose,
    /// Apply edits to the selected style.
    DimStyleDialogApply,
    /// Select a different style in the dialog list.
    DimStyleDialogSelect(String),
    /// Switch the active tab.
    DimStyleDialogTab(u8),
    /// Create a new empty style (prompts via command line).
    DimStyleDialogNew,
    /// Set the selected style as the document's current dim style.
    DimStyleDialogSetCurrent,
    /// Delete the selected style.
    DimStyleDialogDelete,
    // Field edit messages:
    DsEdit(DsField, String),
    DsToggle(DsField),
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
            show_ucs_icon: true,
            last_point: None,
            layer_window: None,
            main_window: None,
            clipboard: Vec::new(),
            clipboard_centroid: glam::Vec3::ZERO,
            layout_context_menu: None,
            layout_rename_state: None,
            last_vp_click_time: None,
            last_vp_click_pos: None,
            page_setup_open: false,
            page_setup_w: String::new(),
            page_setup_h: String::new(),
            page_setup_plot_area: "Layout".to_string(),
            page_setup_center: true,
            page_setup_offset_x: "0.0".to_string(),
            page_setup_offset_y: "0.0".to_string(),
            page_setup_rotation: "0".to_string(),
            page_setup_scale: "Fit".to_string(),
            // Plot style
            active_plot_style: None,
            // MLineStyle dialog
            mlstyle_open: false,
            mlstyle_selected: "Standard".to_string(),
            // DimStyle dialog
            dimstyle_open: false,
            dimstyle_selected: "Standard".to_string(),
            dimstyle_tab: 0,
            ds_dimdle: "0".to_string(),       ds_dimdli: "3.75".to_string(),
            ds_dimgap: "0.625".to_string(),   ds_dimexe: "1.25".to_string(),
            ds_dimexo: "0.625".to_string(),
            ds_dimsd1: false, ds_dimsd2: false,
            ds_dimse1: false, ds_dimse2: false,
            ds_dimasz: "0.18".to_string(),    ds_dimcen: "0.09".to_string(),
            ds_dimtsz: "0".to_string(),
            ds_dimtxt: "0.18".to_string(),    ds_dimtxsty: "Standard".to_string(),
            ds_dimtad: "1".to_string(),
            ds_dimtih: false, ds_dimtoh: false,
            ds_dimscale: "1".to_string(),     ds_dimlfac: "1".to_string(),
            ds_dimlunit: "2".to_string(),     ds_dimdec: "2".to_string(),
            ds_dimpost: "<>".to_string(),
            ds_dimtol: false, ds_dimlim: false,
            ds_dimtp: "0".to_string(),        ds_dimtm: "0".to_string(),
            ds_dimtdec: "2".to_string(),      ds_dimtfac: "1".to_string(),
        };
        app.sync_ribbon_layers();
        app
    }

    /// Boot function for `iced::daemon`: returns initial state plus a task that
    /// opens the primary application window.
    fn boot() -> (Self, Task<Message>) {
        use helpers::build_window_icon;
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
}

use std::path::PathBuf;

pub fn run() -> iced::Result {
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
