use super::{H7CAD, Message, POLY_START_DELAY_MS};
use super::helpers::{parse_coord, angle_close, ortho_constrain, polar_constrain};
use crate::scene::{self, Scene, VIEWCUBE_DRAW_PX, VIEWCUBE_PAD, VIEWCUBE_PX};
use crate::scene::grip::{find_hit_grip, GripEdit};
use crate::scene::object::GripApply;
use crate::modules::ModuleEvent;
use crate::ui::PropertiesPanel;
use acadrust::types::Color as AcadColor;
use acadrust::{EntityType as AcadEntityType, Handle};
use iced::time::Instant;
use iced::window;
use iced::{mouse, Task};

const VIEWCUBE_HIT_SIZE: f32 = VIEWCUBE_DRAW_PX;

impl H7CAD {
    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Tick(t) => {
                self.tabs[self.active_tab].scene.update(t - self.start);
                Task::none()
            }

            Message::OpenFile => Task::perform(crate::io::pick_and_open(), Message::FileOpened),

            Message::FileOpened(Ok((name, path, doc))) => {
                let entity_count = doc.entities().count();
                self.command_line
                    .push_output(&format!("Opened \"{name}\" — {entity_count} entities"));
                self.app_menu.push_recent(path.clone());

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
                    let new_tab = super::document::DocumentTab::new_drawing(self.tab_counter);
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
                crate::linetypes::populate_document(&mut self.tabs[i].scene.document);
                self.tabs[i].properties = PropertiesPanel::empty();
                let doc_layers = self.tabs[i].scene.document.layers.clone();
                self.tabs[i].layers.sync_from_doc(&doc_layers);
                self.sync_ribbon_layers();
                self.tabs[i].scene.fit_all();
                self.tabs[i].dirty = false;
                self.tabs[i].history = super::document::HistoryState::default();
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
                    match crate::io::save(&self.tabs[i].scene.document, &path) {
                        Ok(()) => {
                            self.command_line
                                .push_output(&format!("Saved: {}", path.display()));
                            self.tabs[i].dirty = false;
                        }
                        Err(e) => self.command_line.push_error(&format!("Save failed: {e}")),
                    }
                } else {
                    return Task::perform(crate::io::pick_save_path(), Message::PickedSavePath);
                }
                Task::none()
            }

            Message::SaveAs => Task::perform(crate::io::pick_save_path(), Message::PickedSavePath),

            Message::PickedSavePath(Some(path)) => {
                let i = self.active_tab;
                match crate::io::save(&self.tabs[i].scene.document, &path) {
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
                crate::linetypes::populate_document(&mut self.tabs[i].scene.document);
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
                self.tabs[i].visual_style = if w { "Wireframe".into() } else { "Shaded".into() };
                self.command_line.push_output(if w { "Visual style: Wireframe" } else { "Visual style: Shaded" });
                Task::none()
            }

            Message::SetProjection(ortho) => {
                use crate::scene::Projection;
                let proj = if ortho { Projection::Orthographic } else { Projection::Perspective };
                let i = self.active_tab;
                self.tabs[i].scene.camera.borrow_mut().projection = proj;
                self.tabs[i].scene.camera_generation += 1;
                self.ribbon.set_ortho(ortho);
                self.command_line.push_output(if ortho { "Projection: Orthographic" } else { "Projection: Perspective" });
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
                        self.command_line.push_info("Open DWG/DXF: not yet implemented.");
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
                        self.tabs[i].visual_style = if w { "Wireframe".into() } else { "Shaded".into() };
                        self.command_line.push_output(if w { "Visual style: Wireframe" } else { "Visual style: Shaded" });
                    }
                    ModuleEvent::ToggleLayers => {
                        return Task::done(Message::ToggleLayers);
                    }
                }
                Task::none()
            }

            // ── Application menu ──────────────────────────────────────────
            Message::ToggleAppMenu => { self.app_menu.toggle(); Task::none() }
            Message::CloseAppMenu => { self.app_menu.close(); Task::none() }
            Message::CloseAppMenuAndRun(cmd) => {
                self.app_menu.close();
                self.dispatch_command(&cmd.clone())
            }
            Message::AppMenuSearch(s) => { self.app_menu.search = s; Task::none() }

            // ── Document tabs ─────────────────────────────────────────────
            Message::TabNew => {
                self.tab_counter += 1;
                let new_tab = super::document::DocumentTab::new_drawing(self.tab_counter);
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
                    self.tab_counter += 1;
                    self.tabs[0] = super::document::DocumentTab::new_drawing(self.tab_counter);
                    self.active_tab = 0;
                } else {
                    self.tabs.remove(idx);
                    if self.active_tab >= self.tabs.len() {
                        self.active_tab = self.tabs.len() - 1;
                    }
                }
                Task::none()
            }

            Message::CommandInput(s) => { self.command_line.input = s; Task::none() }

            Message::CommandSubmit => {
                let i = self.active_tab;
                if self.tabs[i].active_cmd.is_some() {
                    let text = self.command_line.input.trim().to_string();
                    self.command_line.input.clear();

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

                    if text.is_empty() {
                        let result = self.tabs[i].active_cmd.as_mut().map(|c| c.on_enter());
                        if let Some(r) = result {
                            return self.apply_cmd_result(r);
                        }
                        return Task::none();
                    }

                    if let Some(pt) = parse_coord(&text) {
                        let result = self.tabs[i].active_cmd.as_mut().map(|c| c.on_point(pt));
                        if let Some(r) = result {
                            return self.apply_cmd_result(r);
                        }
                        return Task::none();
                    }

                    if let Some(result) = self.tabs[i]
                        .active_cmd
                        .as_mut()
                        .and_then(|c| c.on_text_input(&text))
                    {
                        return self.apply_cmd_result(result);
                    }

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
                // Cancel layout rename / context menu first, then fall through.
                if self.layout_rename_state.take().is_some() || self.layout_context_menu.take().is_some() {
                    return Task::none();
                }
                let i = self.active_tab;
                if self.tabs[i].active_cmd.is_some() {
                    let result = self.tabs[i].active_cmd.as_mut().map(|c| c.on_escape());
                    if let Some(r) = result {
                        return self.apply_cmd_result(r);
                    }
                } else {
                    self.tabs[i].scene.deselect_all();
                    self.refresh_properties();
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
                    window::close(id)
                } else {
                    self.sync_ribbon_layers();
                    let (id, task) = window::open(window::Settings {
                        size: iced::Size::new(900.0, 360.0),
                        resizable: true,
                        ..Default::default()
                    });
                    self.layer_window = Some(id);
                    task.map(|_| Message::Noop)
                }
            }

            Message::OsWindowClosed(id) => {
                if self.main_window == Some(id) {
                    return iced::exit();
                }
                if self.layer_window == Some(id) {
                    self.layer_window = None;
                }
                Task::none()
            }

            // ── Layer panel messages ───────────────────────────────────────
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
                        "Layer \"{}\" {}", name, if on { "on" } else { "off" }
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
                        "Layer \"{}\" {}", name, if locked { "locked" } else { "unlocked" }
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
                        if let Some(old_layer) = self.tabs[i].scene.document.layers.get(&old_name) {
                            use acadrust::tables::layer::Layer as DocLayer;
                            let mut nl = DocLayer::new(&new_name);
                            nl.color = old_layer.color.clone();
                            nl.flags = old_layer.flags.clone();
                            let _ = self.tabs[i].scene.document.layers.add(nl);
                        }
                        self.tabs[i].scene.document.layers.remove(&old_name);
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
                self.tabs[i].layers.color_full_palette = !self.tabs[i].layers.color_full_palette;
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

            // ── Cursor / viewport messages ─────────────────────────────────
            Message::CursorMoved(p) => {
                let (vw, _vh) = self.tabs[self.active_tab].scene.selection.borrow().vp_size;
                self.cursor_pos = iced::Point::new(
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

                let (mid_down, mid_last, vp_size) =
                    (sel.middle_down, sel.middle_last_pos, sel.vp_size);
                if mid_down {
                    if let Some(last) = mid_last {
                        let (dx, dy) = (p.x - last.x, p.y - last.y);
                        let bounds = iced::Rectangle { x: 0.0, y: 0.0, width: vp_size.0, height: vp_size.1 };
                        // Drop `sel` before calling mutable scene methods.
                        drop(sel);
                        if self.tabs[i].scene.active_viewport.is_some() {
                            self.tabs[i].scene.pan_active_viewport(dx, dy, bounds);
                        } else {
                            self.tabs[i].scene.camera.borrow_mut().pan(dx, dy);
                        }
                        self.tabs[i].scene.selection.borrow_mut().middle_last_pos = Some(p);
                        return Task::none();
                    }
                    sel.middle_last_pos = Some(p);
                }

                let vp_size = sel.vp_size;
                drop(sel);

                // ── Grip drag ─────────────────────────────────────────────
                if let Some(grip) = self.tabs[i].active_grip.clone() {
                    let (vw, vh) = vp_size;
                    let bounds = iced::Rectangle { x: 0.0, y: 0.0, width: vw, height: vh };
                    let cam = self.tabs[i].scene.camera.borrow();
                    let raw = cam.pick_on_target_plane(p, bounds);
                    let vp_mat = cam.view_proj(bounds);
                    drop(cam);

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
                    self.tabs[i].scene.apply_grip(grip.handle, grip.grip_id, apply);
                    self.tabs[i].dirty = true;
                    self.tabs[i].active_grip.as_mut().unwrap().last_world = snapped;
                    self.refresh_selected_grips();
                    self.refresh_properties();
                    return Task::none();
                }

                if self.tabs[i].active_cmd.is_some() {
                    let (vw, vh) = vp_size;
                    let bounds = iced::Rectangle { x: 0.0, y: 0.0, width: vw, height: vh };
                    let cam = self.tabs[i].scene.camera.borrow();
                    let cursor_world = cam.pick_on_target_plane(p, bounds);
                    let view_proj = cam.view_proj(bounds);
                    drop(cam);

                    let all_wires = self.tabs[i].scene.entity_wires();
                    let needs_tan = self.tabs[i]
                        .active_cmd.as_ref().map(|c| c.needs_tangent_pick()).unwrap_or(false);
                    self.tabs[i].snap_result = if needs_tan {
                        self.snapper.snap_tangent_only(cursor_world, p, &all_wires, view_proj, bounds)
                    } else {
                        self.snapper.snap(cursor_world, p, &all_wires, view_proj, bounds)
                    };
                    let effective = {
                        let mut pt = self.tabs[i].snap_result.map(|s| s.world).unwrap_or(cursor_world);
                        if self.tabs[i].active_cmd.is_some() { pt.z = 0.0; }
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
                        .active_cmd.as_ref().map(|c| c.needs_entity_pick()).unwrap_or(false);
                    let previews = if needs_entity {
                        let hover_handle =
                            scene::hit_test::click_hit(p, &all_wires, view_proj, bounds)
                                .and_then(|s| Scene::handle_from_wire_name(s))
                                .unwrap_or(acadrust::Handle::NULL);
                        self.tabs[i].active_cmd.as_mut()
                            .map(|c| c.on_hover_entity(hover_handle, effective))
                            .unwrap_or_default()
                    } else {
                        self.tabs[i].active_cmd.as_mut()
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
                let bounds = iced::Rectangle { x: 0.0, y: 0.0, width: vw, height: vh };

                if vw > 1.0 && vh > 1.0 {
                    let cam = self.tabs[i].scene.camera.borrow();
                    if scene::hit_test(p.x, p.y, vw, vh, cam.view_rotation_mat(), VIEWCUBE_PX).is_some() {
                        return Task::none();
                    }
                }

                if self.tabs[i].active_cmd.is_none() && !self.tabs[i].selected_grips.is_empty() {
                    if let Some(handle) = self.tabs[i].selected_handle {
                        let vp_mat = self.tabs[i].scene.camera.borrow().view_proj(bounds);
                        let grip_hit = find_hit_grip(p, &self.tabs[i].selected_grips, vp_mat, bounds);
                        if let Some((grip_id, is_translate, world)) = grip_hit {
                            self.tabs[i].active_grip = Some(GripEdit {
                                handle,
                                grip_id,
                                is_translate,
                                origin_world: world,
                                last_world: world,
                            });
                            return Task::none();
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
                let (p, is_click, is_down) = {
                    let sel = self.tabs[i].scene.selection.borrow();
                    let p = match sel.last_move_pos {
                        Some(p) => p,
                        None => return Task::none(),
                    };
                    (p, !sel.left_dragging, sel.left_down)
                };

                if self.tabs[i].active_grip.is_some() {
                    self.tabs[i].active_grip = None;
                    self.refresh_properties();
                    return Task::none();
                }

                let is_gathering = self.tabs[i]
                    .active_cmd.as_ref().map(|c| c.is_selection_gathering()).unwrap_or(false);

                if is_down && is_click && self.tabs[i].active_cmd.is_some() && !is_gathering {
                    let (vw, vh) = self.tabs[i].scene.selection.borrow().vp_size;
                    let bounds = iced::Rectangle { x: 0.0, y: 0.0, width: vw, height: vh };

                    let snap_taken = self.tabs[i].snap_result.take();
                    let tangent_obj_at_click = snap_taken.and_then(|s| s.tangent_obj);

                    let world_pt = {
                        let raw_paper = self.tabs[i].scene.camera.borrow().pick_on_target_plane(p, bounds);
                        // Convert paper-space → model-space when inside a viewport.
                        let raw = self.tabs[i].scene.paper_to_model(raw_paper);
                        let vp_mat = self.tabs[i].scene.camera.borrow().view_proj(bounds);
                        let all_wires = self.tabs[i].scene.entity_wires();
                        let needs_tan = self.tabs[i].active_cmd.as_ref()
                            .map(|c| c.needs_tangent_pick()).unwrap_or(false);
                        let snap_hit = if needs_tan {
                            self.snapper.snap_tangent_only(raw, p, &all_wires, vp_mat, bounds)
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

                    let result = if self.tabs[i].active_cmd.as_ref()
                        .map(|c| c.needs_entity_pick()).unwrap_or(false)
                    {
                        let vp_mat2 = self.tabs[i].scene.camera.borrow().view_proj(bounds);
                        let all_wires2 = self.tabs[i].scene.entity_wires();
                        let hit = scene::hit_test::click_hit(p, &all_wires2, vp_mat2, bounds)
                            .and_then(|s| Scene::handle_from_wire_name(s));
                        if let Some(handle) = hit {
                            self.tabs[i].active_cmd.as_mut().map(|c| c.on_entity_pick(handle, world_pt))
                        } else {
                            self.command_line.push_info("Nothing found at that point.");
                            None
                        }
                    } else if self.tabs[i].active_cmd.as_ref()
                        .map(|c| c.needs_tangent_pick()).unwrap_or(false)
                    {
                        if let Some(obj) = tangent_obj_at_click {
                            self.tabs[i].active_cmd.as_mut().map(|c| c.on_tangent_point(obj, world_pt))
                        } else {
                            self.command_line.push_info("Select a tangent object.");
                            None
                        }
                    } else {
                        self.last_point = Some(world_pt);
                        self.tabs[i].active_cmd.as_mut().map(|c| c.on_point(world_pt))
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
                    let elapsed = sel.left_press_time
                        .map(|t| Instant::now().duration_since(t).as_millis())
                        .unwrap_or(u128::MAX);
                    (sel.left_down, sel.left_dragging, sel.box_anchor, sel.box_crossing, sel.vp_size, elapsed)
                };

                let mut selection_just_completed = false;

                if is_down2 {
                    let bounds = iced::Rectangle { x: 0.0, y: 0.0, width: vp_size.0, height: vp_size.1 };

                    if is_dragging {
                        if elapsed_ms < POLY_START_DELAY_MS {
                            if let Some(a) = box_anchor {
                                let crossing = box_crossing;
                                let all_wires = self.tabs[i].scene.entity_wires();
                                let vp_mat = self.tabs[i].scene.camera.borrow().view_proj(bounds);
                                let mut handles: Vec<Handle> = scene::hit_test::box_hit(
                                    a, p, crossing, &all_wires, vp_mat, bounds,
                                ).into_iter().filter_map(|s| Scene::handle_from_wire_name(s)).collect();
                                handles.extend(scene::hit_test::box_hit_hatch(
                                    a, p, crossing, &self.tabs[i].scene.hatches, vp_mat, bounds,
                                ));
                                self.tabs[i].scene.deselect_all();
                                for h in &handles { self.tabs[i].scene.select_entity(*h, false); }
                                self.tabs[i].scene.expand_selection_for_groups(&handles);
                                self.refresh_properties();
                                selection_just_completed = true;
                            }
                        } else {
                            let (poly_pts, crossing) = {
                                let sel = self.tabs[i].scene.selection.borrow();
                                (sel.poly_points.clone(), sel.poly_crossing)
                            };
                            self.tabs[i].scene.selection.borrow_mut().poly_last_crossing = crossing;
                            let all_wires = self.tabs[i].scene.entity_wires();
                            let vp_mat = self.tabs[i].scene.camera.borrow().view_proj(bounds);
                            let mut handles: Vec<Handle> = scene::hit_test::poly_hit(
                                &poly_pts, crossing, &all_wires, vp_mat, bounds,
                            ).into_iter().filter_map(|s| Scene::handle_from_wire_name(s)).collect();
                            handles.extend(scene::hit_test::poly_hit_hatch(
                                &poly_pts, crossing, &self.tabs[i].scene.hatches, vp_mat, bounds,
                            ));
                            self.tabs[i].scene.deselect_all();
                            for h in &handles { self.tabs[i].scene.select_entity(*h, false); }
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
                        if box_anchor.is_none() {
                            let all_wires = self.tabs[i].scene.entity_wires();
                            let vp_mat = self.tabs[i].scene.camera.borrow().view_proj(bounds);
                            let hit = scene::hit_test::click_hit(p, &all_wires, vp_mat, bounds)
                                .and_then(|s| Scene::handle_from_wire_name(s))
                                .or_else(|| scene::hit_test::click_hit_hatch(
                                    p, &self.tabs[i].scene.hatches, vp_mat, bounds,
                                ));
                            if let Some(handle) = hit {
                                self.tabs[i].scene.select_entity(handle, true);
                                self.tabs[i].scene.expand_selection_for_groups(&[handle]);
                                self.refresh_properties();
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
                            let a = box_anchor.unwrap();
                            let crossing = box_crossing;
                            let all_wires = self.tabs[i].scene.entity_wires();
                            let vp_mat = self.tabs[i].scene.camera.borrow().view_proj(bounds);
                            let mut handles: Vec<Handle> = scene::hit_test::box_hit(
                                a, p, crossing, &all_wires, vp_mat, bounds,
                            ).into_iter().filter_map(|s| Scene::handle_from_wire_name(s)).collect();
                            handles.extend(scene::hit_test::box_hit_hatch(
                                a, p, crossing, &self.tabs[i].scene.hatches, vp_mat, bounds,
                            ));
                            self.tabs[i].scene.deselect_all();
                            for h in &handles { self.tabs[i].scene.select_entity(*h, false); }
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

                if is_gathering && selection_just_completed {
                    let handles: Vec<Handle> = self.tabs[i]
                        .scene.selected_entities().into_iter().map(|(h, _)| h).collect();
                    if let Some(cmd) = self.tabs[i].active_cmd.as_mut() {
                        let result = cmd.on_selection_complete(handles);
                        return self.apply_cmd_result(result);
                    }
                }

                // ── Double-click: enter/exit MSPACE ───────────────────────
                // Only when no command is running, no drag, and we're in paper space.
                if is_click
                    && !is_down2
                    && self.tabs[i].active_cmd.is_none()
                    && self.tabs[i].scene.current_layout != "Model"
                {
                    let now = Instant::now();
                    let is_double = self
                        .last_vp_click_time
                        .map(|t| {
                            let dt = now.duration_since(t).as_millis();
                            let last = self.last_vp_click_pos.unwrap_or(p);
                            let d = (p.x - last.x).hypot(p.y - last.y);
                            dt < 400 && d < 8.0
                        })
                        .unwrap_or(false);

                    self.last_vp_click_time = Some(now);
                    self.last_vp_click_pos = Some(p);

                    if is_double {
                        let (vw, vh) = self.tabs[i].scene.selection.borrow().vp_size;
                        let bounds = iced::Rectangle { x: 0.0, y: 0.0, width: vw, height: vh };
                        let vp_mat = self.tabs[i].scene.camera.borrow().view_proj(bounds);
                        let all_wires = self.tabs[i].scene.entity_wires();
                        let hit = scene::hit_test::click_hit(p, &all_wires, vp_mat, bounds)
                            .and_then(|s| Scene::handle_from_wire_name(s));

                        if let Some(handle) = hit {
                            // Double-clicked on a user viewport → enter MSPACE.
                            if let Some(AcadEntityType::Viewport(vp)) =
                                self.tabs[i].scene.document.get_entity(handle)
                            {
                                if vp.id > 1 {
                                    return Task::done(Message::EnterViewport(handle));
                                }
                            }
                        } else if self.tabs[i].scene.active_viewport.is_some() {
                            // Double-clicked on empty area while in MSPACE → exit.
                            return Task::done(Message::ExitViewport);
                        }
                    }
                }

                Task::none()
            }

            Message::ViewportRightPress => {
                let i = self.active_tab;
                let mut sel = self.tabs[i].scene.selection.borrow_mut();
                let Some(p) = sel.last_move_pos else { return Task::none(); };
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
                let Some(_p) = sel.last_move_pos else { return Task::none(); };
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
                let now = Instant::now();
                let is_double = {
                    let sel = self.tabs[i].scene.selection.borrow();
                    sel.middle_last_press_time
                        .map(|t| now.duration_since(t).as_millis() < 300)
                        .unwrap_or(false)
                };
                {
                    let mut sel = self.tabs[i].scene.selection.borrow_mut();
                    let Some(p) = sel.last_move_pos else { return Task::none(); };
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
                let bounds = iced::Rectangle { x: 0.0, y: 0.0, width: vw, height: vh };
                if self.tabs[i].scene.active_viewport.is_some() {
                    // In MSPACE: zoom the active viewport's model-space view.
                    self.tabs[i].scene.zoom_active_viewport(s);
                } else {
                    let mut cam = self.tabs[i].scene.camera.borrow_mut();
                    if let Some(cursor) = cursor {
                        cam.zoom_about_point(cursor, bounds, s);
                    } else {
                        cam.zoom(s);
                    }
                }
                Task::none()
            }

            Message::ViewportClick => {
                let i = self.active_tab;
                let cam = self.tabs[i].scene.camera.borrow();
                let (vw, vh) = self.tabs[i].scene.selection.borrow().vp_size;
                if let Some(region) = scene::hit_test(
                    self.cursor_pos.x, self.cursor_pos.y, vw, vh, cam.view_rotation_mat(), VIEWCUBE_PX,
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
                self.command_line.push_output(&format!("View: {}", region.label()));
                Task::none()
            }

            // ── Snap / mode toggles ───────────────────────────────────────
            Message::ToggleSnapEnabled => { self.snapper.toggle_global(); Task::none() }
            Message::ToggleGridSnap => { self.snapper.toggle(crate::snap::SnapType::Grid); Task::none() }
            Message::ToggleGrid => { self.show_grid ^= true; Task::none() }
            Message::ToggleOrtho => {
                self.ortho_mode ^= true;
                if self.ortho_mode { self.polar_mode = false; }
                Task::none()
            }
            Message::TogglePolar => {
                self.polar_mode ^= true;
                if self.polar_mode { self.ortho_mode = false; }
                Task::none()
            }
            Message::ToggleSnap(t) => { self.snapper.toggle(t); Task::none() }
            Message::ToggleSnapPopup => { self.snap_popup_open ^= true; Task::none() }
            Message::CloseSnapPopup => { self.snap_popup_open = false; Task::none() }
            Message::SnapSelectAll => { self.snapper.enable_all(); Task::none() }
            Message::SnapClearAll => { self.snapper.disable_all(); Task::none() }

            // ── Ribbon dropdowns ──────────────────────────────────────────
            Message::ToggleRibbonDropdown(id) => { self.ribbon.toggle_dropdown(&id); Task::none() }
            Message::CloseRibbonDropdown => { self.ribbon.close_dropdown(); Task::none() }
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
                            crate::scene::dispatch::apply_common_prop(entity, "layer", &layer);
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
                            crate::scene::dispatch::apply_color(entity, color);
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
                            crate::scene::dispatch::apply_line_weight(entity, lw);
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
                            crate::scene::dispatch::apply_common_prop(entity, "linetype", &lt);
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
                                "invisible" => crate::scene::dispatch::toggle_invisible(entity),
                                _ => crate::scene::dispatch::apply_geom_prop(entity, field, "toggle"),
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
                            crate::scene::dispatch::apply_geom_prop(entity, field, &value);
                        }
                    }
                    self.tabs[i].dirty = true;
                    self.refresh_properties();
                }
                Task::none()
            }

            Message::PropGeomInput { field, value } => {
                self.tabs[self.active_tab].properties.edit_buf.insert(field.to_string(), value);
                Task::none()
            }

            Message::PropGeomCommit(field) => {
                let i = self.active_tab;
                let handles = self.property_target_handles(i);
                if !handles.is_empty() {
                    if let Some(val) = self.tabs[i].properties.edit_buf.remove(field) {
                        self.push_undo_snapshot(i, "CHPROP");
                        for handle in handles {
                            if let Some(entity) = self.tabs[i].scene.document.get_entity_mut(handle) {
                                match field {
                                    "linetype_scale" | "transparency" => {
                                        crate::scene::dispatch::apply_common_prop(entity, field, &val);
                                    }
                                    _ => {
                                        crate::scene::dispatch::apply_geom_prop(entity, field, &val);
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
                self.tabs[i].properties.color_picker_open = !self.tabs[i].properties.color_picker_open;
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
                // Cancel any pending rename/context-menu and active viewport when switching.
                self.layout_rename_state = None;
                self.layout_context_menu = None;
                self.tabs[i].scene.active_viewport = None;
                self.tabs[i].scene.current_layout = name;
                self.tabs[i].scene.deselect_all();
                self.tabs[i].scene.fit_all();
                if going_to_paper {
                    if let Some(idx) = self.ribbon.layout_module_index() {
                        self.ribbon.select(idx);
                    }
                } else if self.ribbon.active_is_layout() {
                    self.ribbon.select(0);
                }
                Task::none()
            }

            Message::LayoutCreate => {
                let i = self.active_tab;
                // Find a unique name (e.g. Layout2, Layout3, ...).
                let existing = self.tabs[i].scene.layout_names();
                let mut idx = existing.len();
                let new_name = loop {
                    let candidate = format!("Layout{}", idx);
                    if !existing.contains(&candidate) {
                        break candidate;
                    }
                    idx += 1;
                };
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
                    Err(e) => self.command_line.push_error(&format!("Layout oluşturulamadı: {e}")),
                }
                Task::none()
            }

            Message::LayoutDelete(name) => {
                let i = self.active_tab;
                self.push_undo_snapshot(i, "LAYOUT DEL");
                if self.tabs[i].scene.delete_layout(&name) {
                    self.layout_context_menu = None;
                    self.layout_rename_state = None;
                    // If we fell back to Model space, update ribbon.
                    if self.tabs[i].scene.current_layout == "Model"
                        && self.ribbon.active_is_layout()
                    {
                        self.ribbon.select(0);
                    }
                    self.command_line.push_output(&format!("Layout \"{name}\" silindi"));
                    self.tabs[i].dirty = true;
                }
                Task::none()
            }

            Message::LayoutRenameStart(name) => {
                if name != "Model" {
                    self.layout_rename_state = Some((name.clone(), name));
                    self.layout_context_menu = None;
                }
                Task::none()
            }

            Message::LayoutRenameEdit(val) => {
                if let Some((orig, _)) = &self.layout_rename_state {
                    let orig = orig.clone();
                    self.layout_rename_state = Some((orig, val));
                }
                Task::none()
            }

            Message::LayoutRenameCommit => {
                if let Some((orig, new_name)) = self.layout_rename_state.take() {
                    let new_name = new_name.trim().to_string();
                    if !new_name.is_empty() && new_name != orig {
                        let i = self.active_tab;
                        let exists = self.tabs[i]
                            .scene
                            .layout_names()
                            .iter()
                            .any(|n| *n == new_name);
                        if exists {
                            self.command_line.push_error(&format!(
                                "\"{}\" adı zaten kullanımda",
                                new_name
                            ));
                        } else {
                            self.push_undo_snapshot(i, "LAYOUT RENAME");
                            self.tabs[i].scene.rename_layout(&orig, &new_name);
                            if self.tabs[i].scene.current_layout == orig {
                                self.tabs[i].scene.current_layout = new_name.clone();
                            }
                            self.tabs[i].dirty = true;
                            self.command_line
                                .push_output(&format!("Layout \"{orig}\" → \"{new_name}\""));
                        }
                    }
                }
                Task::none()
            }

            Message::LayoutRenameCancel => {
                self.layout_rename_state = None;
                Task::none()
            }

            Message::LayoutContextMenu(name) => {
                if name != "Model" {
                    self.layout_context_menu = Some(name);
                }
                Task::none()
            }

            Message::LayoutContextMenuClose => {
                self.layout_context_menu = None;
                Task::none()
            }

            Message::EnterViewport(handle) => {
                let i = self.active_tab;
                self.tabs[i].scene.active_viewport = Some(handle);
                self.command_line.push_output("MSPACE — viewport entered. Middle-drag/scroll to navigate, double-click outside to exit.");
                Task::none()
            }

            Message::ExitViewport => {
                let i = self.active_tab;
                self.tabs[i].scene.active_viewport = None;
                self.command_line.push_output("PSPACE");
                Task::none()
            }

            Message::Undo => { self.undo_active_tab(); Task::none() }
            Message::Redo => { self.redo_active_tab(); Task::none() }

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
}
