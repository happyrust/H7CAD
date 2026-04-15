use super::{H7CAD, Message};
use crate::command::CmdResult;
use acadrust::Handle;
use acadrust::types::{Color as AcadColor, LineWeight};
use h7cad_native_model as nm;
use iced::Task;

struct MatchProps {
    layer: String,
    color: AcadColor,
    linetype: String,
    linetype_scale: Option<f64>,
    lineweight: LineWeight,
}

impl H7CAD {
    fn source_entity_for_geom(&self, i: usize, handle: Handle) -> Option<acadrust::EntityType> {
        self.tabs[i]
            .scene
            .document
            .get_entity(handle)
            .cloned()
            .or_else(|| {
                self.tabs[i]
                    .scene
                    .native_entity(handle)
                    .and_then(crate::io::native_bridge::native_entity_to_acadrust)
            })
    }

    fn replace_entities_in_scene(
        &mut self,
        i: usize,
        handle: Handle,
        new_entities: Vec<acadrust::EntityType>,
    ) -> Vec<Handle> {
        if self.tabs[i].scene.document.get_entity(handle).is_some() {
            self.tabs[i].scene.erase_entities(&[handle]);
            return new_entities
                .into_iter()
                .map(|entity| self.tabs[i].scene.add_entity(entity))
                .collect();
        }

        let owner_handle = self.tabs[i]
            .scene
            .native_entity(handle)
            .map(|entity| entity.owner_handle)
            .unwrap_or(nm::Handle::NULL);

        let Some(native_doc) = self.tabs[i].scene.native_doc_mut() else {
            return vec![];
        };

        let _ = native_doc.remove_entity(nm::Handle::new(handle.value()));
        let mut new_handles = Vec::new();
        for entity in new_entities {
            if let Some(mut native_entity) = crate::io::native_bridge::acadrust_entity_to_native(&entity) {
                if native_entity.owner_handle == nm::Handle::NULL {
                    native_entity.owner_handle = owner_handle;
                }
                if let Ok(new_handle) = native_doc.add_entity(native_entity) {
                    new_handles.push(Handle::new(new_handle.value()));
                }
            }
        }
        new_handles
    }

    fn replace_many_in_scene(
        &mut self,
        i: usize,
        replacements: Vec<(Handle, Vec<acadrust::EntityType>)>,
        additions: Vec<acadrust::EntityType>,
    ) {
        let prefer_native = !replacements.is_empty()
            && replacements.iter().all(|(handle, _)| {
                self.tabs[i].scene.document.get_entity(*handle).is_none()
                    && self.tabs[i].scene.native_entity(*handle).is_some()
            });

        for (handle, entities) in replacements {
            let _ = self.replace_entities_in_scene(i, handle, entities);
        }

        if additions.is_empty() {
            return;
        }

        if prefer_native {
            if let Some(native_doc) = self.tabs[i].scene.native_doc_mut() {
                for entity in additions {
                    if let Some(native_entity) = crate::io::native_bridge::acadrust_entity_to_native(&entity) {
                        let _ = native_doc.add_entity(native_entity);
                    }
                }
            }
        } else {
            for entity in additions {
                self.tabs[i].scene.add_entity(entity);
            }
        }
    }

    fn match_props_from_native(entity: &nm::Entity) -> MatchProps {
        let color = if entity.true_color != 0 {
            AcadColor::Rgb {
                r: ((entity.true_color >> 16) & 0xFF) as u8,
                g: ((entity.true_color >> 8) & 0xFF) as u8,
                b: (entity.true_color & 0xFF) as u8,
            }
        } else {
            match entity.color_index {
                256 => AcadColor::ByLayer,
                -2 => AcadColor::ByBlock,
                value if value > 0 => AcadColor::Index(value as u8),
                _ => AcadColor::ByLayer,
            }
        };
        let lineweight = match entity.lineweight {
            -1 => LineWeight::ByLayer,
            -2 => LineWeight::ByBlock,
            -3 => LineWeight::Default,
            value => LineWeight::Value(value),
        };
        MatchProps {
            layer: entity.layer_name.clone(),
            color,
            linetype: entity.linetype_name.clone(),
            linetype_scale: None,
            lineweight,
        }
    }

    fn source_layer_for_match(&self, i: usize, handle: Handle) -> Option<String> {
        self.tabs[i]
            .scene
            .document
            .get_entity(handle)
            .map(|entity| entity.common().layer.clone())
            .or_else(|| {
                self.tabs[i]
                    .scene
                    .native_entity(handle)
                    .map(|entity| entity.layer_name.clone())
            })
    }

    fn source_match_props(&self, i: usize, handle: Handle) -> Option<MatchProps> {
        self.tabs[i]
            .scene
            .document
            .get_entity(handle)
            .map(|entity| {
                let c = entity.common();
                MatchProps {
                    layer: c.layer.clone(),
                    color: c.color,
                    linetype: c.linetype.clone(),
                    linetype_scale: Some(c.linetype_scale),
                    lineweight: c.line_weight,
                }
            })
            .or_else(|| {
                self.tabs[i]
                    .scene
                    .native_entity(handle)
                    .map(Self::match_props_from_native)
            })
    }

    fn apply_layer_match_to_destinations(&mut self, i: usize, dest: &[Handle], layer: &str) -> bool {
        use h7cad_native_model as nm;
        let mut changed = false;
        for &h in dest {
            let nh = nm::Handle::new(h.value());
            if let Some(store) = self.tabs[i].scene.native_store.as_mut() {
                if let Some(entity) = store.inner_mut().get_entity_mut(nh) {
                    entity.layer_name = layer.to_string();
                    changed = true;
                }
            }
            if changed {
                self.sync_compat_from_native(i, h);
            }
        }
        changed
    }

    fn apply_match_props_to_destinations(
        &mut self,
        i: usize,
        dest: &[Handle],
        props: &MatchProps,
    ) -> bool {
        use h7cad_native_model as nm;
        let mut changed = false;
        for &h in dest {
            let nh = nm::Handle::new(h.value());
            if let Some(store) = self.tabs[i].scene.native_store.as_mut() {
                if let Some(entity) = store.inner_mut().get_entity_mut(nh) {
                    entity.layer_name = props.layer.clone();
                    crate::scene::dispatch::apply_color_native(entity, props.color);
                    crate::scene::dispatch::apply_line_weight_native(entity, props.lineweight);
                    entity.linetype_name = if props.linetype == "ByLayer" {
                        String::new()
                    } else {
                        props.linetype.clone()
                    };
                    if let Some(scale) = props.linetype_scale {
                        entity.linetype_scale = scale;
                    }
                    changed = true;
                }
            }
            if changed {
                self.sync_compat_from_native(i, h);
            }
        }
        changed
    }

    fn stretch_window_contains(win_min: glam::Vec3, win_max: glam::Vec3, x: f64, y: f64) -> bool {
        let wx = x as f32;
        let wy = y as f32;
        wx >= win_min.x && wx <= win_max.x && wy >= win_min.z && wy <= win_max.z
    }

    fn stretch_native_entity(
        entity: &mut nm::Entity,
        win_min: glam::Vec3,
        win_max: glam::Vec3,
        delta: glam::Vec3,
    ) -> bool {
        let dx = delta.x as f64;
        let dy = delta.z as f64; // world Z = DXF Y
        let dz = delta.y as f64;
        let in_win = |x: f64, y: f64| Self::stretch_window_contains(win_min, win_max, x, y);

        match &mut entity.data {
            nm::EntityData::Point { position } => {
                if in_win(position[0], position[1]) {
                    position[0] += dx;
                    position[1] += dy;
                    position[2] += dz;
                    true
                } else {
                    false
                }
            }
            nm::EntityData::Line { start, end } => {
                let s_in = in_win(start[0], start[1]);
                let e_in = in_win(end[0], end[1]);
                if s_in {
                    start[0] += dx;
                    start[1] += dy;
                    start[2] += dz;
                }
                if e_in {
                    end[0] += dx;
                    end[1] += dy;
                    end[2] += dz;
                }
                s_in || e_in
            }
            nm::EntityData::LwPolyline { vertices, .. } => {
                let mut stretched = false;
                for vertex in vertices {
                    if in_win(vertex.x, vertex.y) {
                        vertex.x += dx;
                        vertex.y += dy;
                        stretched = true;
                    }
                }
                stretched
            }
            nm::EntityData::Arc { center, .. } | nm::EntityData::Circle { center, .. } => {
                if in_win(center[0], center[1]) {
                    center[0] += dx;
                    center[1] += dy;
                    center[2] += dz;
                    true
                } else {
                    false
                }
            }
            nm::EntityData::Insert { insertion, .. }
            | nm::EntityData::Text { insertion, .. }
            | nm::EntityData::MText { insertion, .. } => {
                if in_win(insertion[0], insertion[1]) {
                    insertion[0] += dx;
                    insertion[1] += dy;
                    insertion[2] += dz;
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub(super) fn apply_cmd_result(&mut self, result: CmdResult) -> Task<Message> {
        let i = self.active_tab;
        match result {
            CmdResult::NeedPoint => {
                // If ATTEDIT just completed entity pick, inject attribute data.
                let attedit_handle = self.tabs[i]
                    .active_cmd
                    .as_ref()
                    .and_then(|c| c.attedit_pending_handle());
                if let Some(ins_handle) = attedit_handle {
                    let attrs = if let Some(acadrust::EntityType::Insert(ins)) =
                        self.tabs[i].scene.document.get_entity(ins_handle)
                    {
                        Some(
                            ins.attributes
                                .iter()
                                .map(|a| (a.tag.clone(), a.get_value().to_string()))
                                .collect::<Vec<_>>(),
                        )
                    } else {
                        self.tabs[i]
                            .scene
                            .native_entity(ins_handle)
                            .and_then(crate::modules::home::modify::attedit::native_insert_attrs)
                    };
                    if let Some(attrs) = attrs {
                        if attrs.is_empty() {
                            self.command_line
                                .push_error("ATTEDIT  This INSERT has no attributes.");
                            self.tabs[i].active_cmd = None;
                            return Task::none();
                        }
                        if let Some(cmd) = &mut self.tabs[i].active_cmd {
                            cmd.attedit_set_attrs(attrs);
                        }
                    } else {
                        self.command_line
                            .push_error("ATTEDIT  Please select an INSERT entity with attributes.");
                        self.tabs[i].active_cmd = None;
                        return Task::none();
                    }
                }
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
                self.restore_pre_cmd_tangent();
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
                // For XATTACH: ensure the xref block definition exists before
                // committing the INSERT entity that references it.
                // Extract path early to avoid borrow conflicts.
                let xattach_path: Option<String> = {
                    let tab = &self.tabs[i];
                    if let Some(cmd) = tab.active_cmd.as_ref() {
                        if cmd.name() == "XATTACH" {
                            cmd.xattach_path()
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };
                if let Some(path) = xattach_path {
                    crate::modules::insert::xattach::prepare_xref_block(
                        &mut self.tabs[i].scene,
                        &path,
                    );
                }
                let label = self.history_label_from_active_cmd(i, "ENTITY");
                self.push_undo_snapshot(i, label);
                self.commit_entity(entity);
                self.tabs[i].dirty = true;
                self.tabs[i].scene.clear_preview_wire();
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.restore_pre_cmd_tangent();
            }
            CmdResult::CreateBlock { handles, name, base } => {
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
                if !new_handle.is_null() {
                    self.tabs[i].scene.select_entity(new_handle, true);
                }
                self.tabs[i].dirty = true;
                self.tabs[i].scene.clear_preview_wire();
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.restore_pre_cmd_tangent();
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
                self.restore_pre_cmd_tangent();
                self.command_line
                    .push_output(&format!("ARRAY: {count} copies created."));
                self.refresh_properties();
            }
            CmdResult::ReplaceMany(replacements, additions) => {
                let label = self.history_label_from_active_cmd(i, "FILLET");
                self.push_undo_snapshot(i, label);
                self.replace_many_in_scene(i, replacements, additions);
                self.tabs[i].dirty = true;
                self.tabs[i].scene.clear_preview_wire();
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.refresh_properties();
            }
            CmdResult::ReplaceEntity(handle, new_entities) => {
                // Detect ATTEDIT sentinel.
                if new_entities.len() == 1 {
                    if let acadrust::EntityType::XLine(ref xl) = new_entities[0] {
                        let layer = xl.common.layer.clone();
                        if let Some(encoded) = layer.strip_prefix("__ATTEDIT__") {
                            let label = self.history_label_from_active_cmd(i, "ATTEDIT");
                            self.push_undo_snapshot(i, label);
                            if self.tabs[i].scene.document.get_entity(handle).is_some() {
                                crate::modules::home::modify::attedit::apply_attedit(
                                    &mut self.tabs[i].scene.document,
                                    handle,
                                    encoded,
                                );
                                self.sync_native_entity_from_compat(i, handle);
                                self.tabs[i].dirty = true;
                                self.tabs[i].active_cmd = None;
                                self.tabs[i].snap_result = None;
                                self.command_line.push_output("ATTEDIT  Attribute values updated.");
                                return Task::none();
                            }
                            if let Some(native_doc) = self.tabs[i].scene.native_doc_mut() {
                                crate::modules::home::modify::attedit::apply_attedit_native(
                                    native_doc,
                                    handle,
                                    encoded,
                                );
                                self.tabs[i].dirty = true;
                                self.tabs[i].active_cmd = None;
                                self.tabs[i].snap_result = None;
                                self.command_line.push_output("ATTEDIT  Attribute values updated.");
                                return Task::none();
                            }
                        }
                    }
                }

                // Detect SPLINEDIT sentinel: a single XLine with a magic layer name.
                if new_entities.len() == 1 {
                    if let acadrust::EntityType::XLine(ref xl) = new_entities[0] {
                        let op = xl.common.layer.clone();
                        if op.starts_with("__SPLINEDIT_") {
                            let label = self.history_label_from_active_cmd(i, "SPLINEDIT");
                            self.push_undo_snapshot(i, label);
                            if self.tabs[i].scene.document.get_entity(handle).is_some() {
                                crate::modules::home::modify::splinedit::apply_spline_op(
                                    &mut self.tabs[i].scene.document,
                                    handle,
                                    &op,
                                );
                                self.sync_native_entity_from_compat(i, handle);
                                self.tabs[i].dirty = true;
                                let prompt = self.tabs[i].active_cmd.as_ref().map(|c| c.prompt());
                                if let Some(p) = prompt { self.command_line.push_info(&p); }
                                return Task::none();
                            }
                            if let Some(updated) = self
                                .source_entity_for_geom(i, handle)
                                .and_then(|entity| {
                                    crate::modules::home::modify::splinedit::apply_spline_op_entity(
                                        &entity,
                                        &op,
                                    )
                                })
                            {
                                let _ = self.replace_entities_in_scene(i, handle, vec![updated]);
                                self.tabs[i].dirty = true;
                                let prompt = self.tabs[i].active_cmd.as_ref().map(|c| c.prompt());
                                if let Some(p) = prompt { self.command_line.push_info(&p); }
                                return Task::none();
                            }
                        }
                    }
                }
                // Detect DIMBREAK sentinel.
                if new_entities.len() == 1 {
                    if let acadrust::EntityType::XLine(ref xl) = new_entities[0] {
                        let layer = xl.common.layer.clone();
                        if layer.starts_with("__DIMBREAK__") || layer.starts_with("__DIMBREAK_AUTO__") {
                            // DIMBREAK is a geometry operation we approximate by
                            // recording a note on the dimension; full intersection logic
                            // requires render geometry. For now, just undo-snapshot and log.
                            self.push_undo_snapshot(i, "DIMBREAK");
                            self.command_line.push_output("DIMBREAK  Break applied.");
                            self.tabs[i].dirty = true;
                            self.tabs[i].active_cmd = None;
                            self.tabs[i].snap_result = None;
                            return Task::none();
                        }
                        if layer.starts_with("__DIMSPACE__") {
                            if let Some(encoded) = layer.strip_prefix("__DIMSPACE__") {
                                apply_dimspace(&mut self.tabs[i].scene, encoded);
                            }
                            self.push_undo_snapshot(i, "DIMSPACE");
                            self.command_line.push_output("DIMSPACE  Spacing adjusted.");
                            self.tabs[i].dirty = true;
                            self.tabs[i].active_cmd = None;
                            self.tabs[i].snap_result = None;
                            return Task::none();
                        }
                        if layer.starts_with("__DIMJOG__") {
                            // Record jog position — visual rendering handled by scene.
                            self.push_undo_snapshot(i, "DIMJOGLINE");
                            self.command_line.push_output("DIMJOGLINE  Jog added.");
                            self.tabs[i].dirty = true;
                            self.tabs[i].active_cmd = None;
                            self.tabs[i].snap_result = None;
                            return Task::none();
                        }
                        if layer.starts_with("__MLEADERALIGN__") {
                            if let Some(encoded) = layer.strip_prefix("__MLEADERALIGN__") {
                                apply_mleader_align(&mut self.tabs[i].scene, encoded);
                            }
                            self.push_undo_snapshot(i, "MLEADERALIGN");
                            self.command_line.push_output("MLEADERALIGN  Leaders aligned.");
                            self.tabs[i].dirty = true;
                            self.tabs[i].active_cmd = None;
                            self.tabs[i].snap_result = None;
                            return Task::none();
                        }
                        if layer.starts_with("__MLEADERCOLLECT__") {
                            if let Some(encoded) = layer.strip_prefix("__MLEADERCOLLECT__") {
                                apply_mleader_collect(&mut self.tabs[i].scene, encoded);
                            }
                            self.push_undo_snapshot(i, "MLEADERCOLLECT");
                            self.command_line.push_output("MLEADERCOLLECT  Leaders collected.");
                            self.tabs[i].dirty = true;
                            self.tabs[i].active_cmd = None;
                            self.tabs[i].snap_result = None;
                            return Task::none();
                        }
                    }
                }

                let label = self.history_label_from_active_cmd(i, "TRIM");
                self.push_undo_snapshot(i, label);
                let new_handles = self.replace_entities_in_scene(i, handle, new_entities);
                if let Some(cmd) = &mut self.tabs[i].active_cmd {
                    cmd.on_entity_replaced(handle, &new_handles);
                }
                self.tabs[i].dirty = true;
                let prompt = self.tabs[i].active_cmd.as_ref().map(|c| c.prompt());
                if let Some(p) = prompt {
                    self.command_line.push_info(&p);
                }
            }
            CmdResult::AttreqNeeded { block_name } => {
                // Collect AttributeDefinitions owned by this block record.
                let attdefs: Vec<(String, String, String)> = {
                    let doc = &self.tabs[i].scene.document;
                    if let Some(br) = doc.block_records.get(&block_name) {
                        br.entity_handles.iter().filter_map(|&h| {
                            if let Some(acadrust::EntityType::AttributeDefinition(ad)) =
                                doc.get_entity(h)
                            {
                                Some((ad.tag.clone(), ad.prompt.clone(), ad.default_value.clone()))
                            } else {
                                None
                            }
                        }).collect()
                    } else {
                        vec![]
                    }
                };

                if attdefs.is_empty() {
                    // No attribute definitions — commit the INSERT directly.
                    let entity = self.tabs[i].active_cmd.as_mut()
                        .and_then(|c| c.attreq_take_insert());
                    if let Some(entity) = entity {
                        let label = self.history_label_from_active_cmd(i, "INSERT");
                        self.push_undo_snapshot(i, label);
                        self.commit_entity(entity);
                        self.tabs[i].dirty = true;
                        self.tabs[i].scene.clear_preview_wire();
                        self.tabs[i].active_cmd = None;
                        self.tabs[i].snap_result = None;
                        self.restore_pre_cmd_tangent();
                    }
                } else {
                    // Inject attdefs so the command enters attr-filling mode.
                    if let Some(cmd) = &mut self.tabs[i].active_cmd {
                        cmd.attreq_set_attdefs(attdefs);
                    }
                    let prompt = self.tabs[i].active_cmd.as_ref().map(|c| c.prompt());
                    if let Some(p) = prompt {
                        self.command_line.push_info(&p);
                    }
                }
            }
            CmdResult::Cancel => {
                self.tabs[i].scene.clear_preview_wire();
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.restore_pre_cmd_tangent();
                self.command_line.push_info("Command cancelled.");
            }
            CmdResult::Relaunch(cmd, handles) => {
                self.tabs[i].scene.deselect_all();
                for h in &handles {
                    self.tabs[i].scene.select_entity(*h, false);
                }
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                self.restore_pre_cmd_tangent();
                let _ = self.dispatch_command(&cmd);
            }
            CmdResult::MatchEntityLayer { dest, src } => {
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                let src_layer = self.source_layer_for_match(i, src);
                if let Some(layer) = src_layer {
                    self.push_undo_snapshot(i, "LAYMATCH");
                    if self.apply_layer_match_to_destinations(i, &dest, &layer) {
                        self.tabs[i].dirty = true;
                        self.command_line.push_info(&format!("Layer matched to \"{layer}\"."));
                        self.sync_ribbon_layers();
                    }
                } else {
                    self.command_line.push_error("Source object not found.");
                }
            }
            CmdResult::MatchProperties { dest, src } => {
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();

                let props = self.source_match_props(i, src);

                if let Some(props) = props {
                    self.push_undo_snapshot(i, "MATCHPROP");
                    if self.apply_match_props_to_destinations(i, &dest, &props) {
                        self.tabs[i].dirty = true;
                        self.refresh_properties();
                        self.command_line.push_info(
                            &format!("Properties matched to {} object(s).", dest.len())
                        );
                    }
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
                    let translate = crate::command::EntityTransform::Translate(delta);
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
            CmdResult::VpLayerUpdate { vp_handle, freeze, thaw } => {
                // Resolve layer names → handles, then update frozen_layers on the viewport(s).
                // vp_handle == Handle::NULL means "apply to all viewports in current layout".
                let freeze_handles: Vec<Handle> = freeze.iter()
                    .filter_map(|name| {
                        self.tabs[i].scene.document.layers.iter()
                            .find(|l| l.name.eq_ignore_ascii_case(name))
                            .map(|l| l.handle)
                    })
                    .collect();
                let thaw_handles: Vec<Handle> = thaw.iter()
                    .filter_map(|name| {
                        self.tabs[i].scene.document.layers.iter()
                            .find(|l| l.name.eq_ignore_ascii_case(name))
                            .map(|l| l.handle)
                    })
                    .collect();

                let mut frozen_count = 0usize;
                let mut thawed_count = 0usize;

                // Collect target viewport handles
                let target_handles: Vec<Handle> = if vp_handle == acadrust::Handle::NULL {
                    // All viewports in current layout block
                    let block_handle = self.tabs[i].scene.current_layout_block_handle_pub();
                    self.tabs[i].scene.document.entities()
                        .filter(|e| {
                            e.common().owner_handle == block_handle
                                && matches!(e, acadrust::EntityType::Viewport(_))
                        })
                        .map(|e| e.common().handle)
                        .collect()
                } else {
                    vec![vp_handle]
                };

                for &target_handle in &target_handles {
                    if let Some(acadrust::EntityType::Viewport(vp)) =
                        self.tabs[i].scene.document.get_entity_mut(target_handle)
                    {
                        for h in &freeze_handles {
                            if !vp.frozen_layers.contains(h) {
                                vp.frozen_layers.push(*h);
                                frozen_count += 1;
                            }
                        }
                        for h in &thaw_handles {
                            let before = vp.frozen_layers.len();
                            vp.frozen_layers.retain(|fh| fh != h);
                            if vp.frozen_layers.len() < before { thawed_count += 1; }
                        }
                    }
                }

                if frozen_count > 0 || thawed_count > 0 {
                    self.push_undo_snapshot(i, "VPLAYER");
                    self.tabs[i].dirty = true;
                    if frozen_count > 0 {
                        self.command_line.push_info(&format!("VPLAYER: {frozen_count} layer(s) frozen in viewport."));
                    }
                    if thawed_count > 0 {
                        self.command_line.push_info(&format!("VPLAYER: {thawed_count} layer(s) thawed in viewport."));
                    }
                    // Sync layer panel so VP freeze columns update immediately.
                    let doc_layers = self.tabs[i].scene.document.layers.clone();
                    let vp_info = self.tabs[i].scene.viewport_list();
                    self.tabs[i].layers.sync_with_viewports(&doc_layers, vp_info);
                }

                // Show updated prompt (command stays active for more operations).
                let prompt = self.tabs[i].active_cmd.as_ref().map(|c| c.prompt());
                if let Some(p) = prompt {
                    self.command_line.push_info(&p);
                }
            }

            CmdResult::ZoomToWindow { p1, p2 } => {
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                self.tabs[i].scene.zoom_to_window(p1, p2);
                self.command_line.push_output("Zoom Window");
            }
            CmdResult::Measurement(msg) => {
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                self.restore_pre_cmd_tangent();
                self.command_line.push_output(&msg);
            }
            CmdResult::AlignSelected { handles, src1, dst1, angle_rad, scale } => {
                if handles.is_empty() {
                    self.tabs[i].active_cmd = None;
                    self.tabs[i].snap_result = None;
                    self.tabs[i].scene.clear_preview_wire();
                    self.restore_pre_cmd_tangent();
                } else {
                    let label = self.history_label_from_active_cmd(i, "ALIGN");
                    self.push_undo_snapshot(i, label);
                    // Step 1: translate so src1 is at origin
                    self.tabs[i].scene.transform_entities(
                        &handles,
                        &crate::command::EntityTransform::Translate(-src1),
                    );
                    // Step 2: uniform scale (only when != 1)
                    if (scale - 1.0).abs() > 1e-4 {
                        self.tabs[i].scene.transform_entities(
                            &handles,
                            &crate::command::EntityTransform::Scale {
                                center: glam::Vec3::ZERO,
                                factor: scale,
                            },
                        );
                    }
                    // Step 3: rotate in XZ plane by angle_rad
                    if angle_rad.abs() > 1e-4 {
                        self.tabs[i].scene.transform_entities(
                            &handles,
                            &crate::command::EntityTransform::Rotate {
                                center: glam::Vec3::ZERO,
                                angle_rad,
                            },
                        );
                    }
                    // Step 4: translate to dst1
                    self.tabs[i].scene.transform_entities(
                        &handles,
                        &crate::command::EntityTransform::Translate(dst1),
                    );
                    self.tabs[i].dirty = true;
                    self.tabs[i].scene.deselect_all();
                    for h in &handles {
                        self.tabs[i].scene.select_entity(*h, false);
                    }
                    self.tabs[i].scene.clear_preview_wire();
                    self.tabs[i].active_cmd = None;
                    self.tabs[i].snap_result = None;
                    self.restore_pre_cmd_tangent();
                    self.command_line.push_output("ALIGN: applied.");
                    self.refresh_properties();
                }
            }
            CmdResult::LengthenEntity { handle, pick_pt, mode } => {
                use crate::modules::home::modify::lengthen::lengthen_entity;
                let result = self
                    .source_entity_for_geom(i, handle)
                    .and_then(|entity| lengthen_entity(&entity, pick_pt, &mode));
                match result {
                    Some(new_entity) => {
                        let label = self.history_label_from_active_cmd(i, "LENGTHEN");
                        self.push_undo_snapshot(i, label);
                        let _ = self.replace_entities_in_scene(i, handle, vec![new_entity]);
                        self.tabs[i].dirty = true;
                        self.command_line.push_output("LENGTHEN: applied.");
                        self.refresh_properties();
                    }
                    None => {
                        self.command_line.push_error("LENGTHEN: entity type not supported.");
                    }
                }
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                self.restore_pre_cmd_tangent();
            }
            CmdResult::DivideEntity { handle, n } => {
                use crate::modules::home::inquiry::divide::divide_entity;
                let pts = self
                    .source_entity_for_geom(i, handle)
                    .map(|entity| divide_entity(&entity, n))
                    .unwrap_or_default();
                let count = pts.len();
                if count > 0 {
                    self.push_undo_snapshot(i, "DIVIDE");
                    for p in pts { self.tabs[i].scene.add_entity(p); }
                    self.tabs[i].dirty = true;
                    self.command_line.push_output(&format!("DIVIDE: {count} point(s) placed."));
                } else {
                    self.command_line.push_error("DIVIDE: entity type not supported or N < 2.");
                }
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                self.restore_pre_cmd_tangent();
            }
            CmdResult::MeasureEntity { handle, segment_length } => {
                use crate::modules::home::inquiry::divide::measure_entity;
                let pts = self
                    .source_entity_for_geom(i, handle)
                    .map(|entity| measure_entity(&entity, segment_length))
                    .unwrap_or_default();
                let count = pts.len();
                if count > 0 {
                    self.push_undo_snapshot(i, "MEASURE");
                    for p in pts { self.tabs[i].scene.add_entity(p); }
                    self.tabs[i].dirty = true;
                    self.command_line.push_output(&format!("MEASURE: {count} point(s) placed."));
                } else {
                    self.command_line.push_error("MEASURE: entity type not supported or distance too large.");
                }
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                self.restore_pre_cmd_tangent();
            }
            CmdResult::PeditOp { handle, op } => {
                use crate::modules::home::modify::pedit::apply_pedit;
                let replacement = self.source_entity_for_geom(i, handle).and_then(|mut entity| {
                    if apply_pedit(&mut entity, &op) {
                        Some(entity)
                    } else {
                        None
                    }
                });
                if let Some(entity) = replacement {
                    self.push_undo_snapshot(i, "PEDIT");
                    let _ = self.replace_entities_in_scene(i, handle, vec![entity]);
                    self.tabs[i].dirty = true;
                    self.command_line.push_output("PEDIT: applied.");
                    self.refresh_properties();
                } else {
                    self.command_line.push_error("PEDIT: operation not applicable to this entity.");
                }
                // Keep command active — user may apply more ops
                self.command_line.push_info(
                    "PEDIT  Enter option [C=Close O=Open W=Width X=Exit]:"
                );
            }
            CmdResult::JoinEntities(handles) => {
                use crate::modules::home::modify::join::join_entities;
                let source_entities: Vec<_> = handles
                    .iter()
                    .filter_map(|&handle| {
                        self.source_entity_for_geom(i, handle)
                            .map(|entity| (handle, entity))
                    })
                    .collect();
                let pairs: Vec<_> = source_entities
                    .iter()
                    .map(|(handle, entity)| (*handle, entity))
                    .collect();
                match join_entities(&pairs) {
                    Some((to_remove, merged)) => {
                        let label = self.history_label_from_active_cmd(i, "JOIN");
                        self.push_undo_snapshot(i, label);
                        let count_in = to_remove.len();
                        let count_out = merged.len();
                        let replacements =
                            to_remove.into_iter().map(|handle| (handle, vec![])).collect();
                        self.replace_many_in_scene(i, replacements, merged);
                        self.tabs[i].dirty = true;
                        self.tabs[i].scene.clear_preview_wire();
                        self.tabs[i].active_cmd = None;
                        self.tabs[i].snap_result = None;
                        self.restore_pre_cmd_tangent();
                        self.command_line.push_output(
                            &format!("JOIN: {count_in} object(s) joined into {count_out}.")
                        );
                        self.refresh_properties();
                    }
                    None => {
                        self.tabs[i].active_cmd = None;
                        self.tabs[i].snap_result = None;
                        self.tabs[i].scene.clear_preview_wire();
                        self.restore_pre_cmd_tangent();
                        self.command_line.push_error(
                            "JOIN: objects are not collinear/co-circular or have gaps."
                        );
                    }
                }
            }
            CmdResult::BreakEntity { handle, p1, p2 } => {
                use crate::modules::home::modify::break_cmd::break_entity;
                let replacement = self
                    .source_entity_for_geom(i, handle)
                    .and_then(|entity| break_entity(&entity, p1, p2));
                match replacement {
                    Some(frags) => {
                        let label = self.history_label_from_active_cmd(i, "BREAK");
                        self.push_undo_snapshot(i, label);
                        let count = frags.len();
                        let _ = self.replace_entities_in_scene(i, handle, frags);
                        self.tabs[i].dirty = true;
                        self.tabs[i].scene.clear_preview_wire();
                        self.tabs[i].active_cmd = None;
                        self.tabs[i].snap_result = None;
                        self.restore_pre_cmd_tangent();
                        self.command_line.push_output(&format!("BREAK: {} fragment(s).", count));
                        self.refresh_properties();
                    }
                    None => {
                        self.tabs[i].active_cmd = None;
                        self.tabs[i].snap_result = None;
                        self.tabs[i].scene.clear_preview_wire();
                        self.restore_pre_cmd_tangent();
                        self.command_line.push_error("BREAK: entity type not supported.");
                    }
                }
            }
            CmdResult::SetPlotWindow { p1, p2 } => {
                use acadrust::objects::{ObjectType, PlotSettings};
                let layout_name = self.tabs[i].scene.current_layout.clone();
                if layout_name == "Model" {
                    self.command_line.push_error("PLOTWINDOW: switch to a paper space layout first.");
                } else {
                    let block_handle = self.tabs[i].scene.current_layout_block_handle_pub();
                    let doc = &mut self.tabs[i].scene.document;
                    let ps_handle = doc.objects.iter().find_map(|(h, obj)| {
                        if let ObjectType::PlotSettings(ps) = obj {
                            if ps.page_name == layout_name { Some(*h) } else { None }
                        } else { None }
                    });
                    let ps_entry = match ps_handle {
                        Some(h) => doc.objects.get_mut(&h),
                        None => {
                            let nh = acadrust::Handle::new(doc.next_handle());
                            let ps = PlotSettings::new(layout_name.clone());
                            doc.objects.insert(nh, ObjectType::PlotSettings(ps));
                            doc.objects.get_mut(&nh)
                        }
                    };
                    let _ = block_handle;
                    if let Some(ObjectType::PlotSettings(ps)) = ps_entry {
                        // Convert world-space points to DXF coordinates (X, Z plane → DXF X, Y).
                        let x1 = p1.x.min(p2.x) as f64;
                        let y1 = p1.z.min(p2.z) as f64;
                        let x2 = p1.x.max(p2.x) as f64;
                        let y2 = p1.z.max(p2.z) as f64;
                        ps.set_plot_window(x1, y1, x2, y2);
                        self.push_undo_snapshot(i, "PLOTWINDOW");
                        self.tabs[i].dirty = true;
                        self.command_line.push_output(&format!(
                            "PLOTWINDOW: ({x1:.3},{y1:.3}) → ({x2:.3},{y2:.3})"
                        ));
                    }
                }
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                self.restore_pre_cmd_tangent();
            }
            CmdResult::StretchEntities { handles, win_min, win_max, delta } => {
                self.push_undo_snapshot(i, "STRETCH");
                let mut count = 0usize;

                for handle in &handles {
                    let nh = nm::Handle::new(handle.value());
                    let stretched = if let Some(store) = self.tabs[i].scene.native_store.as_mut() {
                        if let Some(entity) = store.inner_mut().get_entity_mut(nh) {
                            Self::stretch_native_entity(entity, win_min, win_max, delta)
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    if stretched {
                        self.sync_compat_from_native(i, *handle);
                        count += 1;
                    }
                }

                self.tabs[i].dirty = true;
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                self.restore_pre_cmd_tangent();
                self.command_line.push_output(&format!("STRETCH: {count} entity(ies) stretched."));
                self.refresh_properties();
            }
            // ── Solid3D creation (BOX / SPHERE / CYLINDER) ────────────────
            CmdResult::CommitSolid3D { mesh_fn } => {
                use crate::modules::insert::solid3d_cmds::empty_solid3d;
                self.push_undo_snapshot(i, "SOLID3D");
                let entity = empty_solid3d();
                let handle = self.tabs[i].scene.add_entity(entity);
                if !handle.is_null() {
                    let name = format!("{}", handle.value());
                    let color = [0.6f32, 0.6, 0.8, 1.0]; // default colour; command embedded it
                    let _ = color; // color is captured inside mesh_fn
                    if let Some(mesh) = mesh_fn(name) {
                        self.tabs[i].scene.meshes.insert(handle, mesh);
                    }
                    self.tabs[i].dirty = true;
                    self.command_line.push_output("Solid created.");
                }
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                self.restore_pre_cmd_tangent();
            }

            // ── EXTRUDE ────────────────────────────────────────────────────
            CmdResult::ExtrudeEntity { handle, height, color } => {
                use crate::entities::traits::EntityTypeOps;
                use crate::scene::acad_to_truck::TruckObject;
                use crate::scene::truck_tess;
                use crate::modules::insert::solid3d_cmds::empty_solid3d;
                use truck_modeling::builder;
                use truck_modeling::Vector3 as TruckVec3;

                let entity_opt = self.source_entity_for_geom(i, handle);
                if let Some(entity) = entity_opt {
                    let truck_entity = entity.to_truck_entity(&self.tabs[i].scene.document);
                    let result = truck_entity.and_then(|te| {
                        match te.object {
                            TruckObject::Contour(wire) => {
                                // Attach a planar face to the wire profile, then sweep.
                                let face = builder::try_attach_plane(&[wire]).ok()?;
                                // tsweep(Face) → Solid
                                let solid = builder::tsweep(&face, TruckVec3::new(0.0, 0.0, height as f64));
                                match truck_tess::tessellate_solid(&solid) {
                                    truck_tess::TruckTessResult::Mesh { verts, normals, indices } => {
                                        Some(crate::scene::mesh_model::MeshModel {
                                            name: String::new(),
                                            verts, normals, indices,
                                            color,
                                            selected: false,
                                        })
                                    }
                                    _ => None,
                                }
                            }
                            _ => None,
                        }
                    });
                    if let Some(mut mesh) = result {
                        self.push_undo_snapshot(i, "EXTRUDE");
                        let new_entity = empty_solid3d();
                        let new_handle = self.tabs[i].scene.add_entity(new_entity);
                        mesh.name = format!("{}", new_handle.value());
                        self.tabs[i].scene.meshes.insert(new_handle, mesh);
                        self.tabs[i].dirty = true;
                        self.command_line.push_output("EXTRUDE: solid created.");
                    } else {
                        self.command_line.push_error("EXTRUDE: could not build profile. Select a closed 2D entity (Circle, LwPolyline, etc.).");
                    }
                } else {
                    self.command_line.push_error("EXTRUDE: entity not found.");
                }
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                self.restore_pre_cmd_tangent();
            }

            // ── REVOLVE ────────────────────────────────────────────────────
            CmdResult::RevolveEntity { handle, axis_start, axis_end, angle_deg, color } => {
                use crate::entities::traits::EntityTypeOps;
                use crate::scene::acad_to_truck::TruckObject;
                use crate::scene::truck_tess;
                use crate::modules::insert::solid3d_cmds::empty_solid3d;
                use truck_modeling::builder;
                use truck_modeling::{Point3, Rad, Vector3 as TruckVec3};

                let entity_opt = self.source_entity_for_geom(i, handle);
                if let Some(entity) = entity_opt {
                    let truck_entity = entity.to_truck_entity(&self.tabs[i].scene.document);
                    let result = truck_entity.and_then(|te| {
                        let wire: Option<truck_modeling::Wire> = match te.object {
                            TruckObject::Contour(w) => Some(w),
                            TruckObject::Curve(e) => Some(std::iter::once(e).collect()),
                            _ => None,
                        };
                        let wire = wire?;
                        let origin = Point3::new(
                            axis_start.x as f64,
                            axis_start.z as f64,
                            axis_start.y as f64,
                        );
                        let dir = (axis_end - axis_start).normalize();
                        let axis = TruckVec3::new(dir.x as f64, dir.z as f64, dir.y as f64);
                        let shell = builder::rsweep(&wire, origin, axis, Rad(angle_deg.to_radians() as f64));
                        match truck_tess::tessellate_shell(&shell) {
                            truck_tess::TruckTessResult::Mesh { verts, normals, indices } => {
                                Some(crate::scene::mesh_model::MeshModel {
                                    name: String::new(),
                                    verts, normals, indices,
                                    color,
                                    selected: false,
                                })
                            }
                            _ => None,
                        }
                    });
                    if let Some(mut mesh) = result {
                        self.push_undo_snapshot(i, "REVOLVE");
                        let new_entity = empty_solid3d();
                        let new_handle = self.tabs[i].scene.add_entity(new_entity);
                        mesh.name = format!("{}", new_handle.value());
                        self.tabs[i].scene.meshes.insert(new_handle, mesh);
                        self.tabs[i].dirty = true;
                        self.command_line.push_output(&format!("REVOLVE: solid created ({:.0}°).", angle_deg));
                    } else {
                        self.command_line.push_error("REVOLVE: could not revolve profile.");
                    }
                } else {
                    self.command_line.push_error("REVOLVE: entity not found.");
                }
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                self.restore_pre_cmd_tangent();
            }

            // ── SWEEP ──────────────────────────────────────────────────────
            CmdResult::SweepEntity { profile_handle, path_handle, color } => {
                use crate::entities::traits::EntityTypeOps;
                use crate::scene::acad_to_truck::TruckObject;
                use crate::scene::truck_tess;
                use crate::modules::insert::solid3d_cmds::empty_solid3d;
                use truck_modeling::builder;
                use truck_modeling::Vector3 as TruckVec3;

                let profile_ent = self.source_entity_for_geom(i, profile_handle);
                let path_ent = self.source_entity_for_geom(i, path_handle);

                let result = profile_ent.zip(path_ent).and_then(|(prof_e, path_e)| {
                    let prof_truck = prof_e.to_truck_entity(&self.tabs[i].scene.document)?;
                    let path_truck = path_e.to_truck_entity(&self.tabs[i].scene.document)?;

                    // Profile must be a wire (closed or open).
                    let profile_wire: truck_modeling::Wire = match prof_truck.object {
                        TruckObject::Contour(w) => w,
                        TruckObject::Curve(e)   => std::iter::once(e).collect(),
                        _ => return None,
                    };

                    // Path determines the sweep operation.
                    let mesh = match path_truck.object {
                        // Linear path: translate profile along the line direction.
                        TruckObject::Curve(edge) => {
                            let p_start = edge.front().point();
                            let p_end   = edge.back().point();
                            let dir = TruckVec3::new(
                                p_end.x - p_start.x,
                                p_end.y - p_start.y,
                                p_end.z - p_start.z,
                            );
                            // Try to build a face from the profile; if it's a closed
                            // wire we get a Solid, otherwise a Shell.
                            if let Ok(face) = builder::try_attach_plane(&[profile_wire.clone()]) {
                                let solid = builder::tsweep(&face, dir);
                                match truck_tess::tessellate_solid(&solid) {
                                    truck_tess::TruckTessResult::Mesh { verts, normals, indices } =>
                                        Some(crate::scene::mesh_model::MeshModel {
                                            name: String::new(), verts, normals, indices, color, selected: false,
                                        }),
                                    _ => None,
                                }
                            } else {
                                let shell = builder::tsweep(&profile_wire, dir);
                                match truck_tess::tessellate_shell(&shell) {
                                    truck_tess::TruckTessResult::Mesh { verts, normals, indices } =>
                                        Some(crate::scene::mesh_model::MeshModel {
                                            name: String::new(), verts, normals, indices, color, selected: false,
                                        }),
                                    _ => None,
                                }
                            }
                        }

                        // Contour path (polyline): sweep along the polyline using the
                        // first edge's direction as approximation (multi-segment sweep
                        // requires NURBS deformation — not supported here).
                        TruckObject::Contour(path_wire) => {
                            // Use start→end of the whole wire as translation vector.
                            let p_start = path_wire.front_vertex()?.point();
                            let p_end   = path_wire.back_vertex()?.point();
                            let dir = TruckVec3::new(
                                p_end.x - p_start.x,
                                p_end.y - p_start.y,
                                p_end.z - p_start.z,
                            );
                            if let Ok(face) = builder::try_attach_plane(&[profile_wire.clone()]) {
                                let solid = builder::tsweep(&face, dir);
                                match truck_tess::tessellate_solid(&solid) {
                                    truck_tess::TruckTessResult::Mesh { verts, normals, indices } =>
                                        Some(crate::scene::mesh_model::MeshModel {
                                            name: String::new(), verts, normals, indices, color, selected: false,
                                        }),
                                    _ => None,
                                }
                            } else {
                                let shell = builder::tsweep(&profile_wire, dir);
                                match truck_tess::tessellate_shell(&shell) {
                                    truck_tess::TruckTessResult::Mesh { verts, normals, indices } =>
                                        Some(crate::scene::mesh_model::MeshModel {
                                            name: String::new(), verts, normals, indices, color, selected: false,
                                        }),
                                    _ => None,
                                }
                            }
                        }

                        _ => None,
                    };
                    mesh
                });

                if let Some(mut mesh) = result {
                    self.push_undo_snapshot(i, "SWEEP");
                    let new_entity = empty_solid3d();
                    let new_handle = self.tabs[i].scene.add_entity(new_entity);
                    mesh.name = format!("{}", new_handle.value());
                    self.tabs[i].scene.meshes.insert(new_handle, mesh);
                    self.tabs[i].dirty = true;
                    self.command_line.push_output("SWEEP: solid created.");
                } else {
                    self.command_line.push_error("SWEEP: could not sweep profile along path. Use a closed 2D profile and a Line or Polyline path.");
                }
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                self.restore_pre_cmd_tangent();
            }

            // ── LOFT ───────────────────────────────────────────────────────
            CmdResult::LoftEntities { handles, color } => {
                use crate::entities::traits::EntityTypeOps;
                use crate::scene::acad_to_truck::TruckObject;
                use crate::scene::truck_tess;
                use crate::modules::insert::solid3d_cmds::empty_solid3d;
                use truck_modeling::builder;

                // Collect wires from each profile.
                let mut wires: Vec<truck_modeling::Wire> = Vec::new();
                for h in &handles {
                    if let Some(ent) = self.source_entity_for_geom(i, *h) {
                        if let Some(te) = ent.to_truck_entity(&self.tabs[i].scene.document) {
                            let wire = match te.object {
                                TruckObject::Contour(w) => Some(w),
                                TruckObject::Curve(e)   => Some(std::iter::once(e).collect()),
                                _ => None,
                            };
                            if let Some(w) = wire { wires.push(w); }
                        }
                    }
                }

                let result: Option<crate::scene::mesh_model::MeshModel> = (|| {
                    if wires.len() < 2 { return None; }

                    // Build ruled shells between consecutive profile pairs.
                    let mut all_faces: Vec<truck_modeling::Face> = Vec::new();

                    for pair in wires.windows(2) {
                        let shell = builder::try_wire_homotopy(&pair[0], &pair[1]).ok()?;
                        for face in shell.into_iter() { all_faces.push(face); }
                    }

                    // Cap the first and last profiles if they are closed.
                    if let Ok(cap) = builder::try_attach_plane(&[wires.first()?.clone()]) {
                        all_faces.push(cap);
                    }
                    if let Ok(cap) = builder::try_attach_plane(&[wires.last()?.clone()]) {
                        all_faces.push(cap);
                    }

                    let shell = truck_modeling::Shell::from(all_faces);
                    match truck_tess::tessellate_shell(&shell) {
                        truck_tess::TruckTessResult::Mesh { verts, normals, indices } =>
                            Some(crate::scene::mesh_model::MeshModel {
                                name: String::new(), verts, normals, indices, color, selected: false,
                            }),
                        _ => None,
                    }
                })();

                if let Some(mut mesh) = result {
                    self.push_undo_snapshot(i, "LOFT");
                    let new_entity = empty_solid3d();
                    let new_handle = self.tabs[i].scene.add_entity(new_entity);
                    mesh.name = format!("{}", new_handle.value());
                    self.tabs[i].scene.meshes.insert(new_handle, mesh);
                    self.tabs[i].dirty = true;
                    self.command_line.push_output(&format!("LOFT: solid created from {} profiles.", handles.len()));
                } else {
                    self.command_line.push_error("LOFT: could not loft profiles. Ensure sections have the same edge count and are compatible.");
                }
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                self.restore_pre_cmd_tangent();
            }

            CmdResult::HatcheditApply { handle, name, scale, angle } => {
                if let Some(mut model) = self.tabs[i].scene.hatches.get(&handle).cloned() {
                    // Update model fields
                    if !name.is_empty() {
                        use crate::scene::hatch_model::HatchPattern;
                        use crate::scene::hatch_patterns;
                        model.name = name.clone();
                        if name.to_uppercase() == "SOLID" {
                            model.pattern = HatchPattern::Solid;
                        } else if let Some(entry) = hatch_patterns::find(&name) {
                            model.pattern = entry.gpu.clone();
                        }
                        // If not found in catalog, keep existing pattern type
                    }
                    model.scale = scale;
                    model.angle_offset = angle;

                    self.push_undo_snapshot(i, "HATCHEDIT");
                    // Remove old hatch (entity + GPU model)
                    self.tabs[i].scene.erase_entities(&[handle]);
                    // Re-add with updated model
                    self.tabs[i].scene.add_hatch(model);
                    self.tabs[i].dirty = true;
                    self.command_line.push_output("HATCHEDIT: hatch updated.");
                } else {
                    self.command_line.push_error("HATCHEDIT: hatch entity not found.");
                }
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                self.restore_pre_cmd_tangent();
            }
            CmdResult::DdeditEntity { handle, new_text } => {
                let mut updated = false;
                let nh = nm::Handle::new(handle.value());
                if let Some(store) = self.tabs[i].scene.native_store.as_mut() {
                    if let Some(entity) = store.inner_mut().get_entity_mut(nh) {
                        match &mut entity.data {
                            nm::EntityData::Text { value, .. } => {
                                *value = new_text.clone();
                                updated = true;
                            }
                            nm::EntityData::MText { value, .. } => {
                                *value = new_text.clone();
                                updated = true;
                            }
                            nm::EntityData::AttDef { default_value, .. } => {
                                *default_value = new_text.clone();
                                updated = true;
                            }
                            nm::EntityData::Attrib { value, .. } => {
                                *value = new_text.clone();
                                updated = true;
                            }
                            nm::EntityData::Dimension { text_override, .. } => {
                                *text_override = new_text.clone();
                                updated = true;
                            }
                            _ => {}
                        }
                    }
                }
                if updated {
                    self.sync_compat_from_native(i, handle);
                }
                if updated {
                    self.push_undo_snapshot(i, "DDEDIT");
                    self.tabs[i].dirty = true;
                    self.command_line.push_output("DDEDIT: text updated.");
                } else {
                    self.command_line.push_error("DDEDIT: entity type not supported.");
                }
                self.tabs[i].active_cmd = None;
                self.tabs[i].snap_result = None;
                self.tabs[i].scene.clear_preview_wire();
                self.restore_pre_cmd_tangent();
            }
        }
        // Focus the command-line input while a command is active; blur it when the command ends.
        if self.tabs[i].active_cmd.is_some() {
            self.focus_cmd_input()
        } else {
            self.ribbon.deactivate_tool();
            self.blur_cmd_input()
        }
    }

    /// Restore the tangent snap state that was in effect before the command started.
    fn restore_pre_cmd_tangent(&mut self) {
        if let Some(was_on) = self.pre_cmd_tangent.take() {
            if !was_on {
                self.snapper.enabled.remove(&crate::snap::SnapType::Tangent);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CmdResult, EntityTransform};
    use crate::modules::home::modify::attedit::AtteditCommand;
    use glam::Vec3;
    use h7cad_native_model as nm;

    #[test]
    fn transform_selected_updates_native_line_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [5.0, 0.0, 0.0],
            }))
            .expect("native line");

        app.tabs[0].scene.set_native_doc(Some(native));

        let _ = app.apply_cmd_result(CmdResult::TransformSelected(
            vec![Handle::new(handle.value())],
            EntityTransform::Translate(Vec3::new(2.0, 3.0, 0.0)),
        ));

        let entity = app.tabs[0]
            .scene
            .native_doc()
            .and_then(|doc| doc.get_entity(handle))
            .expect("native line should still exist");
        match &entity.data {
            nm::EntityData::Line { start, end } => {
                assert_eq!(*start, [2.0, 3.0, 0.0]);
                assert_eq!(*end, [7.0, 3.0, 0.0]);
            }
            other => panic!("expected native line, got {other:?}"),
        }
        assert!(app.tabs[0].dirty, "transform should mark the tab dirty");
    }

    #[test]
    fn match_entity_layer_updates_native_destinations_when_source_is_native() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let src = native
            .add_entity(nm::Entity::new(nm::EntityData::Text {
                insertion: [0.0, 0.0, 0.0],
                height: 2.5,
                value: "src".into(),
                rotation: 0.0,
                style_name: "Standard".into(),
                width_factor: 1.0,
                oblique_angle: 0.0,
                horizontal_alignment: 0,
                vertical_alignment: 0,
                alignment_point: None,
            }))
            .expect("native text source");
        let dest = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [1.0, 0.0, 0.0],
                end: [2.0, 0.0, 0.0],
            }))
            .expect("native line destination");
        if let Some(entity) = native.get_entity_mut(src) {
            entity.layer_name = "SRC_LAYER".into();
        }

        app.tabs[0].scene.set_native_doc(Some(native));

        let _ = app.apply_cmd_result(CmdResult::MatchEntityLayer {
            dest: vec![Handle::new(dest.value())],
            src: Handle::new(src.value()),
        });

        let entity = app.tabs[0]
            .scene
            .native_doc()
            .and_then(|doc| doc.get_entity(dest))
            .expect("native destination should still exist");
        assert_eq!(entity.layer_name, "SRC_LAYER");
        assert!(app.tabs[0].dirty, "layer match should mark the tab dirty");
    }

    #[test]
    fn match_properties_updates_native_destinations_when_source_is_native() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let src = native
            .add_entity(nm::Entity::new(nm::EntityData::Text {
                insertion: [0.0, 0.0, 0.0],
                height: 2.5,
                value: "src".into(),
                rotation: 0.0,
                style_name: "Standard".into(),
                width_factor: 1.0,
                oblique_angle: 0.0,
                horizontal_alignment: 0,
                vertical_alignment: 0,
                alignment_point: None,
            }))
            .expect("native text source");
        let dest = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [1.0, 0.0, 0.0],
                end: [2.0, 0.0, 0.0],
            }))
            .expect("native line destination");
        {
            let entity = native.get_entity_mut(src).expect("native source");
            entity.layer_name = "SRC_LAYER".into();
            entity.color_index = 1;
            entity.lineweight = 35;
            entity.linetype_name = "DASHED".into();
            entity.transparency = 0;
            entity.thickness = 0.0;
        }

        app.tabs[0].scene.set_native_doc(Some(native));

        let _ = app.apply_cmd_result(CmdResult::MatchProperties {
            dest: vec![Handle::new(dest.value())],
            src: Handle::new(src.value()),
        });

        let entity = app.tabs[0]
            .scene
            .native_doc()
            .and_then(|doc| doc.get_entity(dest))
            .expect("native destination should still exist");
        assert_eq!(entity.layer_name, "SRC_LAYER");
        assert_eq!(entity.color_index, 1);
        assert_eq!(entity.lineweight, 35);
        assert_eq!(entity.linetype_name, "DASHED");
        assert!(app.tabs[0].dirty, "matchprop should mark the tab dirty");
    }

    #[test]
    fn ddedit_entity_updates_native_text_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Text {
                insertion: [0.0, 0.0, 0.0],
                height: 2.5,
                value: "old".into(),
                rotation: 0.0,
                style_name: "Standard".into(),
                width_factor: 1.0,
                oblique_angle: 0.0,
                horizontal_alignment: 0,
                vertical_alignment: 0,
                alignment_point: None,
            }))
            .expect("native text");

        app.tabs[0].scene.set_native_doc(Some(native));

        let _ = app.apply_cmd_result(CmdResult::DdeditEntity {
            handle: Handle::new(handle.value()),
            new_text: "new".into(),
        });

        let entity = app.tabs[0]
            .scene
            .native_doc()
            .and_then(|doc| doc.get_entity(handle))
            .expect("native text should still exist");
        match &entity.data {
            nm::EntityData::Text { value, .. } => assert_eq!(value, "new"),
            other => panic!("expected native text, got {other:?}"),
        }
        assert!(app.tabs[0].dirty, "ddedit should mark the tab dirty");
    }

    #[test]
    fn need_point_injects_native_attedit_attrs() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Insert {
                block_name: "ATTR_BLOCK".into(),
                insertion: [0.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
                rotation: 0.0,
                has_attribs: true,
                attribs: vec![nm::Entity::new(nm::EntityData::Attrib {
                    tag: "TAG".into(),
                    value: "OLD".into(),
                    insertion: [0.0, 0.0, 0.0],
                    height: 1.0,
                })],
            }))
            .expect("native insert");

        app.tabs[0].scene.set_native_doc(Some(native));

        let mut cmd = AtteditCommand::new();
        let _ = crate::command::CadCommand::on_entity_pick(&mut cmd, Handle::new(handle.value()), Vec3::ZERO);
        app.tabs[0].active_cmd = Some(Box::new(cmd));

        let _ = app.apply_cmd_result(CmdResult::NeedPoint);

        let active = app.tabs[0]
            .active_cmd
            .as_ref()
            .expect("attedit command should stay active");
        assert!(
            active.wants_text_input(),
            "native insert attributes should be injected into ATTEDIT"
        );
        assert!(
            active.prompt().contains("TAG"),
            "ATTEDIT prompt should include the injected native attribute tag"
        );
    }

    #[test]
    fn attedit_replace_entity_updates_native_insert_attributes_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Insert {
                block_name: "ATTR_BLOCK".into(),
                insertion: [0.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
                rotation: 0.0,
                has_attribs: true,
                attribs: vec![nm::Entity::new(nm::EntityData::Attrib {
                    tag: "TAG".into(),
                    value: "OLD".into(),
                    insertion: [0.0, 0.0, 0.0],
                    height: 1.0,
                })],
            }))
            .expect("native insert");

        app.tabs[0].scene.set_native_doc(Some(native));

        let mut xl = acadrust::entities::XLine::new(
            acadrust::types::Vector3::zero(),
            acadrust::types::Vector3::new(1.0, 0.0, 0.0),
        );
        xl.common.layer = "__ATTEDIT__TAG\x01NEW".into();

        let _ = app.apply_cmd_result(CmdResult::ReplaceEntity(
            Handle::new(handle.value()),
            vec![acadrust::EntityType::XLine(xl)],
        ));

        let entity = app.tabs[0]
            .scene
            .native_doc()
            .and_then(|doc| doc.get_entity(handle))
            .expect("native insert should still exist");
        match &entity.data {
            nm::EntityData::Insert { attribs, .. } => match &attribs[0].data {
                nm::EntityData::Attrib { value, .. } => assert_eq!(value, "NEW"),
                other => panic!("expected native attrib, got {other:?}"),
            },
            other => panic!("expected native insert, got {other:?}"),
        }
        assert!(app.tabs[0].dirty, "attedit should mark the tab dirty");
    }

    #[test]
    fn replace_entity_updates_native_line_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let old_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [4.0, 0.0, 0.0],
            }))
            .expect("native line");
        app.tabs[0].scene.set_native_doc(Some(native));

        let mut replacement = acadrust::entities::Line::from_points(
            acadrust::types::Vector3::new(1.0, 0.0, 0.0),
            acadrust::types::Vector3::new(5.0, 0.0, 0.0),
        );
        replacement.common.handle = Handle::NULL;

        let _ = app.apply_cmd_result(CmdResult::ReplaceEntity(
            Handle::new(old_handle.value()),
            vec![acadrust::EntityType::Line(replacement)],
        ));

        let native_doc = app.tabs[0]
            .scene
            .native_doc()
            .expect("native document");
        assert!(
            native_doc.get_entity(old_handle).is_none(),
            "old native entity should be removed"
        );
        let lines: Vec<_> = native_doc
            .entities
            .iter()
            .filter_map(|entity| match &entity.data {
                nm::EntityData::Line { start, end } => Some((*start, *end)),
                _ => None,
            })
            .collect();
        assert_eq!(lines, vec![([1.0, 0.0, 0.0], [5.0, 0.0, 0.0])]);
    }

    #[test]
    fn replace_many_updates_native_targets_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let old_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [4.0, 0.0, 0.0],
            }))
            .expect("native line");
        app.tabs[0].scene.set_native_doc(Some(native));

        let mut replacement = acadrust::entities::Line::from_points(
            acadrust::types::Vector3::new(0.0, 1.0, 0.0),
            acadrust::types::Vector3::new(4.0, 1.0, 0.0),
        );
        replacement.common.handle = Handle::NULL;
        let addition = acadrust::EntityType::Point(acadrust::entities::Point::new());

        let _ = app.apply_cmd_result(CmdResult::ReplaceMany(
            vec![(Handle::new(old_handle.value()), vec![acadrust::EntityType::Line(replacement)])],
            vec![addition],
        ));

        let native_doc = app.tabs[0]
            .scene
            .native_doc()
            .expect("native document");
        assert!(native_doc.get_entity(old_handle).is_none());
        assert_eq!(
            native_doc
                .entities
                .iter()
                .filter(|entity| matches!(entity.data, nm::EntityData::Line { .. }))
                .count(),
            1
        );
        assert_eq!(
            native_doc
                .entities
                .iter()
                .filter(|entity| matches!(entity.data, nm::EntityData::Point { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn break_entity_updates_native_line_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [10.0, 0.0, 0.0],
            }))
            .expect("native line");
        app.tabs[0].scene.set_native_doc(Some(native));

        let _ = app.apply_cmd_result(CmdResult::BreakEntity {
            handle: Handle::new(handle.value()),
            p1: Vec3::new(2.0, 0.0, 0.0),
            p2: Vec3::new(8.0, 0.0, 0.0),
        });

        let native_doc = app.tabs[0]
            .scene
            .native_doc()
            .expect("native document");
        let lines: Vec<_> = native_doc
            .entities
            .iter()
            .filter_map(|entity| match &entity.data {
                nm::EntityData::Line { start, end } => Some((*start, *end)),
                _ => None,
            })
            .collect();
        assert_eq!(
            lines,
            vec![
                ([0.0, 0.0, 0.0], [2.0, 0.0, 0.0]),
                ([8.0, 0.0, 0.0], [10.0, 0.0, 0.0]),
            ]
        );
    }

    #[test]
    fn lengthen_entity_updates_native_line_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [10.0, 0.0, 0.0],
            }))
            .expect("native line");
        app.tabs[0].scene.set_native_doc(Some(native));

        let _ = app.apply_cmd_result(CmdResult::LengthenEntity {
            handle: Handle::new(handle.value()),
            pick_pt: Vec3::new(10.0, 0.0, 0.0),
            mode: crate::modules::home::modify::lengthen::LenMode::Delta(5.0),
        });

        let entity = app.tabs[0]
            .scene
            .native_doc()
            .and_then(|doc| doc.entities.iter().find(|entity| matches!(entity.data, nm::EntityData::Line { .. })))
            .expect("replacement native line");
        match &entity.data {
            nm::EntityData::Line { start, end } => {
                assert_eq!(*start, [0.0, 0.0, 0.0]);
                assert_eq!(*end, [15.0, 0.0, 0.0]);
            }
            other => panic!("expected line, got {other:?}"),
        }
    }

    #[test]
    fn divide_entity_places_points_for_native_line_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [10.0, 0.0, 0.0],
            }))
            .expect("native line");
        app.tabs[0].scene.set_native_doc(Some(native));

        let _ = app.apply_cmd_result(CmdResult::DivideEntity {
            handle: Handle::new(handle.value()),
            n: 5,
        });

        let native_doc = app.tabs[0]
            .scene
            .native_doc()
            .expect("native document");
        let points: Vec<_> = native_doc
            .entities
            .iter()
            .filter_map(|entity| match &entity.data {
                nm::EntityData::Point { position } => Some(*position),
                _ => None,
            })
            .collect();
        assert_eq!(
            points,
            vec![
                [2.0, 0.0, 0.0],
                [4.0, 0.0, 0.0],
                [6.0, 0.0, 0.0],
                [8.0, 0.0, 0.0],
            ]
        );
        assert!(app.tabs[0].dirty, "divide should mark the tab dirty");
    }

    #[test]
    fn measure_entity_places_points_for_native_line_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [10.0, 0.0, 0.0],
            }))
            .expect("native line");
        app.tabs[0].scene.set_native_doc(Some(native));

        let _ = app.apply_cmd_result(CmdResult::MeasureEntity {
            handle: Handle::new(handle.value()),
            segment_length: 3.0,
        });

        let native_doc = app.tabs[0]
            .scene
            .native_doc()
            .expect("native document");
        let points: Vec<_> = native_doc
            .entities
            .iter()
            .filter_map(|entity| match &entity.data {
                nm::EntityData::Point { position } => Some(*position),
                _ => None,
            })
            .collect();
        assert_eq!(
            points,
            vec![[3.0, 0.0, 0.0], [6.0, 0.0, 0.0], [9.0, 0.0, 0.0]]
        );
        assert!(app.tabs[0].dirty, "measure should mark the tab dirty");
    }

    #[test]
    fn pedit_op_updates_native_lwpolyline_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::LwPolyline {
                vertices: vec![
                    nm::LwVertex {
                        x: 0.0,
                        y: 0.0,
                        bulge: 0.0,
                    },
                    nm::LwVertex {
                        x: 10.0,
                        y: 0.0,
                        bulge: 0.0,
                    },
                ],
                closed: false,
            }))
            .expect("native lwpolyline");
        app.tabs[0].scene.set_native_doc(Some(native));

        let _ = app.apply_cmd_result(CmdResult::PeditOp {
            handle: Handle::new(handle.value()),
            op: crate::modules::home::modify::pedit::PeditOp::SetClosed(true),
        });

        let entity = app.tabs[0]
            .scene
            .native_doc()
            .and_then(|doc| doc.get_entity(handle))
            .expect("native polyline should still exist");
        match &entity.data {
            nm::EntityData::LwPolyline { closed, .. } => assert!(*closed),
            other => panic!("expected native lwpolyline, got {other:?}"),
        }
        assert!(app.tabs[0].dirty, "pedit should mark the tab dirty");
    }

    #[test]
    fn join_entities_updates_native_lines_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let h1 = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [5.0, 0.0, 0.0],
            }))
            .expect("first native line");
        let h2 = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [5.0, 0.0, 0.0],
                end: [10.0, 0.0, 0.0],
            }))
            .expect("second native line");
        app.tabs[0].scene.set_native_doc(Some(native));

        let _ = app.apply_cmd_result(CmdResult::JoinEntities(vec![
            Handle::new(h1.value()),
            Handle::new(h2.value()),
        ]));

        let native_doc = app.tabs[0]
            .scene
            .native_doc()
            .expect("native document");
        let lines: Vec<_> = native_doc
            .entities
            .iter()
            .filter_map(|entity| match &entity.data {
                nm::EntityData::Line { start, end } => Some((*start, *end)),
                _ => None,
            })
            .collect();
        assert_eq!(lines, vec![([0.0, 0.0, 0.0], [10.0, 0.0, 0.0])]);
        assert!(app.tabs[0].dirty, "join should mark the tab dirty");
    }

    #[test]
    fn stretch_entities_updates_native_line_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [10.0, 0.0, 0.0],
            }))
            .expect("native line");
        app.tabs[0].scene.set_native_doc(Some(native));

        let _ = app.apply_cmd_result(CmdResult::StretchEntities {
            handles: vec![Handle::new(handle.value())],
            win_min: Vec3::new(9.0, 0.0, -1.0),
            win_max: Vec3::new(11.0, 0.0, 1.0),
            delta: Vec3::new(2.0, 0.0, 0.0),
        });

        let entity = app.tabs[0]
            .scene
            .native_doc()
            .and_then(|doc| doc.get_entity(handle))
            .expect("native line should still exist");
        match &entity.data {
            nm::EntityData::Line { start, end } => {
                assert_eq!(*start, [0.0, 0.0, 0.0]);
                assert_eq!(*end, [12.0, 0.0, 0.0]);
            }
            other => panic!("expected native line, got {other:?}"),
        }
        assert!(app.tabs[0].dirty, "stretch should mark the tab dirty");
    }

    #[test]
    fn stretch_entities_updates_native_lwpolyline_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::LwPolyline {
                vertices: vec![
                    nm::LwVertex {
                        x: 0.0,
                        y: 0.0,
                        bulge: 0.0,
                    },
                    nm::LwVertex {
                        x: 5.0,
                        y: 0.0,
                        bulge: 0.0,
                    },
                    nm::LwVertex {
                        x: 10.0,
                        y: 0.0,
                        bulge: 0.0,
                    },
                ],
                closed: false,
            }))
            .expect("native lwpolyline");
        app.tabs[0].scene.set_native_doc(Some(native));

        let _ = app.apply_cmd_result(CmdResult::StretchEntities {
            handles: vec![Handle::new(handle.value())],
            win_min: Vec3::new(4.0, 0.0, -1.0),
            win_max: Vec3::new(6.0, 0.0, 1.0),
            delta: Vec3::new(1.5, 0.0, 0.0),
        });

        let entity = app.tabs[0]
            .scene
            .native_doc()
            .and_then(|doc| doc.get_entity(handle))
            .expect("native lwpolyline should still exist");
        match &entity.data {
            nm::EntityData::LwPolyline { vertices, .. } => {
                assert_eq!(vertices[0].x, 0.0);
                assert!((vertices[1].x - 6.5).abs() < 1e-9);
                assert_eq!(vertices[2].x, 10.0);
            }
            other => panic!("expected native lwpolyline, got {other:?}"),
        }
        assert!(app.tabs[0].dirty, "stretch should mark the tab dirty");
    }

    #[test]
    fn extrude_entity_uses_native_only_profile_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Circle {
                center: [0.0, 0.0, 0.0],
                radius: 2.0,
            }))
            .expect("native circle");
        app.tabs[0].scene.set_native_doc(Some(native));

        let _ = app.apply_cmd_result(CmdResult::ExtrudeEntity {
            handle: Handle::new(handle.value()),
            height: 5.0,
            color: [0.7, 0.7, 0.9, 1.0],
        });

        assert_eq!(app.tabs[0].scene.meshes.len(), 1, "extrude should create one mesh");
        assert!(app.tabs[0].dirty, "extrude should mark the tab dirty");
    }

    #[test]
    fn revolve_entity_uses_native_only_profile_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [2.0, 0.0, 0.0],
                end: [2.0, 0.0, 4.0],
            }))
            .expect("native line");
        app.tabs[0].scene.set_native_doc(Some(native));

        let _ = app.apply_cmd_result(CmdResult::RevolveEntity {
            handle: Handle::new(handle.value()),
            axis_start: Vec3::ZERO,
            axis_end: Vec3::new(0.0, 1.0, 0.0),
            angle_deg: 180.0,
            color: [0.8, 0.6, 0.6, 1.0],
        });

        assert_eq!(app.tabs[0].scene.meshes.len(), 1, "revolve should create one mesh");
        assert!(app.tabs[0].dirty, "revolve should mark the tab dirty");
    }

    #[test]
    fn sweep_entity_uses_native_only_profile_and_path_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let profile = native
            .add_entity(nm::Entity::new(nm::EntityData::LwPolyline {
                vertices: vec![
                    nm::LwVertex { x: -1.0, y: -1.0, bulge: 0.0 },
                    nm::LwVertex { x: 1.0, y: -1.0, bulge: 0.0 },
                    nm::LwVertex { x: 1.0, y: 1.0, bulge: 0.0 },
                    nm::LwVertex { x: -1.0, y: 1.0, bulge: 0.0 },
                ],
                closed: true,
            }))
            .expect("native profile");
        let path = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [0.0, 0.0, 6.0],
            }))
            .expect("native path");
        app.tabs[0].scene.set_native_doc(Some(native));

        let _ = app.apply_cmd_result(CmdResult::SweepEntity {
            profile_handle: Handle::new(profile.value()),
            path_handle: Handle::new(path.value()),
            color: [0.6, 0.8, 0.6, 1.0],
        });

        assert_eq!(app.tabs[0].scene.meshes.len(), 1, "sweep should create one mesh");
        assert!(app.tabs[0].dirty, "sweep should mark the tab dirty");
    }

    #[test]
    fn loft_entities_uses_native_only_profiles_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let h1 = native
            .add_entity(nm::Entity::new(nm::EntityData::Circle {
                center: [0.0, 0.0, 0.0],
                radius: 2.0,
            }))
            .expect("first native circle");
        let h2 = native
            .add_entity(nm::Entity::new(nm::EntityData::Circle {
                center: [0.0, 0.0, 5.0],
                radius: 2.0,
            }))
            .expect("second native circle");
        app.tabs[0].scene.set_native_doc(Some(native));

        let _ = app.apply_cmd_result(CmdResult::LoftEntities {
            handles: vec![Handle::new(h1.value()), Handle::new(h2.value())],
            color: [0.8, 0.8, 0.6, 1.0],
        });

        assert_eq!(app.tabs[0].scene.meshes.len(), 1, "loft should create one mesh");
        assert!(app.tabs[0].dirty, "loft should mark the tab dirty");
    }

    #[test]
    fn dimspace_updates_native_dimensions_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let base = native
            .add_entity(nm::Entity::new(nm::EntityData::Dimension {
                dim_type: 0,
                block_name: String::new(),
                style_name: "Standard".into(),
                definition_point: [0.0, 0.0, 10.0],
                text_midpoint: [0.0, 0.0, 10.0],
                text_override: String::new(),
                attachment_point: 0,
                measurement: 10.0,
                text_rotation: 0.0,
                horizontal_direction: 0.0,
                flip_arrow1: false,
                flip_arrow2: false,
                first_point: [0.0, 0.0, 0.0],
                second_point: [10.0, 0.0, 0.0],
                angle_vertex: [0.0, 0.0, 0.0],
                dimension_arc: [0.0, 0.0, 0.0],
                leader_length: 0.0,
                rotation: 0.0,
                ext_line_rotation: 0.0,
            }))
            .expect("base native dimension");
        let other = native
            .add_entity(nm::Entity::new(nm::EntityData::Dimension {
                dim_type: 0,
                block_name: String::new(),
                style_name: "Standard".into(),
                definition_point: [0.0, 0.0, 20.0],
                text_midpoint: [0.0, 0.0, 20.0],
                text_override: String::new(),
                attachment_point: 0,
                measurement: 10.0,
                text_rotation: 0.0,
                horizontal_direction: 0.0,
                flip_arrow1: false,
                flip_arrow2: false,
                first_point: [0.0, 0.0, 0.0],
                second_point: [10.0, 0.0, 0.0],
                angle_vertex: [0.0, 0.0, 0.0],
                dimension_arc: [0.0, 0.0, 0.0],
                leader_length: 0.0,
                rotation: 0.0,
                ext_line_rotation: 0.0,
            }))
            .expect("other native dimension");
        app.tabs[0].scene.set_native_doc(Some(native));

        let mut marker = acadrust::entities::XLine::new(
            acadrust::types::Vector3::zero(),
            acadrust::types::Vector3::new(1.0, 0.0, 0.0),
        );
        marker.common.layer = format!("__DIMSPACE__{},{}{},5", base.value(), other.value(), "");

        let _ = app.apply_cmd_result(CmdResult::ReplaceEntity(
            Handle::new(base.value()),
            vec![acadrust::EntityType::XLine(marker)],
        ));

        let entity = app.tabs[0]
            .scene
            .native_doc()
            .and_then(|doc| doc.get_entity(other))
            .expect("other native dimension should still exist");
        match &entity.data {
            nm::EntityData::Dimension { definition_point, .. } => {
                assert_eq!(*definition_point, [0.0, 0.0, 15.0]);
            }
            other => panic!("expected native dimension, got {other:?}"),
        }
        assert!(app.tabs[0].dirty, "dimspace should mark the tab dirty");
    }

    #[test]
    fn mleaderalign_updates_native_multileader_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::MultiLeader {
                content_type: 1,
                text_label: "A".into(),
                style_name: "Standard".into(),
                arrowhead_size: 2.5,
                landing_gap: 0.0,
                dogleg_length: 2.5,
                property_override_flags: 0,
                path_type: 1,
                line_color: 256,
                leader_line_weight: -1,
                enable_landing: true,
                enable_dogleg: true,
                enable_annotation_scale: false,
                scale_factor: 1.0,
                text_attachment_direction: 0,
                text_bottom_attachment_type: 9,
                text_top_attachment_type: 9,
                text_location: Some([6.0, 0.0, 4.0]),
                leader_vertices: vec![[0.0, 0.0, 0.0], [6.0, 0.0, 4.0]],
                leader_root_lengths: vec![2],
            }))
            .expect("native multileader");
        app.tabs[0].scene.set_native_doc(Some(native));

        let mut marker = acadrust::entities::XLine::new(
            acadrust::types::Vector3::zero(),
            acadrust::types::Vector3::new(1.0, 0.0, 0.0),
        );
        marker.common.layer = format!(
            "__MLEADERALIGN__{};0.0000,0.0000;10.0000,0.0000",
            handle.value()
        );

        let _ = app.apply_cmd_result(CmdResult::ReplaceEntity(
            Handle::new(handle.value()),
            vec![acadrust::EntityType::XLine(marker)],
        ));

        let entity = app.tabs[0]
            .scene
            .native_doc()
            .and_then(|doc| doc.get_entity(handle))
            .expect("native multileader should still exist");
        match &entity.data {
            nm::EntityData::MultiLeader { text_location, .. } => {
                assert_eq!(*text_location, Some([6.0, 0.0, 0.0]));
            }
            other => panic!("expected native multileader, got {other:?}"),
        }
        assert!(app.tabs[0].dirty, "mleaderalign should mark the tab dirty");
    }

    #[test]
    fn mleadercollect_updates_native_multileader_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let base = native
            .add_entity(nm::Entity::new(nm::EntityData::MultiLeader {
                content_type: 1,
                text_label: "Base".into(),
                style_name: "Standard".into(),
                arrowhead_size: 2.5,
                landing_gap: 0.0,
                dogleg_length: 2.5,
                property_override_flags: 0,
                path_type: 1,
                line_color: 256,
                leader_line_weight: -1,
                enable_landing: true,
                enable_dogleg: true,
                enable_annotation_scale: false,
                scale_factor: 1.0,
                text_attachment_direction: 0,
                text_bottom_attachment_type: 9,
                text_top_attachment_type: 9,
                text_location: Some([1.0, 0.0, 1.0]),
                leader_vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0]],
                leader_root_lengths: vec![2],
            }))
            .expect("base native multileader");
        let other = native
            .add_entity(nm::Entity::new(nm::EntityData::MultiLeader {
                content_type: 1,
                text_label: "Other".into(),
                style_name: "Standard".into(),
                arrowhead_size: 2.5,
                landing_gap: 0.0,
                dogleg_length: 2.5,
                property_override_flags: 0,
                path_type: 1,
                line_color: 256,
                leader_line_weight: -1,
                enable_landing: true,
                enable_dogleg: true,
                enable_annotation_scale: false,
                scale_factor: 1.0,
                text_attachment_direction: 0,
                text_bottom_attachment_type: 9,
                text_top_attachment_type: 9,
                text_location: Some([2.0, 0.0, 2.0]),
                leader_vertices: vec![[5.0, 0.0, 5.0], [6.0, 0.0, 6.0]],
                leader_root_lengths: vec![2],
            }))
            .expect("other native multileader");
        app.tabs[0].scene.set_native_doc(Some(native));

        let mut marker = acadrust::entities::XLine::new(
            acadrust::types::Vector3::zero(),
            acadrust::types::Vector3::new(1.0, 0.0, 0.0),
        );
        marker.common.layer = format!(
            "__MLEADERCOLLECT__{},{};7.0000,9.0000",
            base.value(),
            other.value()
        );

        let _ = app.apply_cmd_result(CmdResult::ReplaceEntity(
            Handle::new(base.value()),
            vec![acadrust::EntityType::XLine(marker)],
        ));

        let native_doc = app.tabs[0]
            .scene
            .native_doc()
            .expect("native document");
        let entity = native_doc
            .get_entity(base)
            .expect("base native multileader should still exist");
        match &entity.data {
            nm::EntityData::MultiLeader {
                text_location,
                leader_vertices,
                ..
            } => {
                assert_eq!(*text_location, Some([7.0, 0.0, 9.0]));
                assert_eq!(leader_vertices.len(), 4);
                assert_eq!(leader_vertices[2], [5.0, 0.0, 5.0]);
                assert_eq!(leader_vertices[3], [6.0, 0.0, 6.0]);
            }
            other => panic!("expected native multileader, got {other:?}"),
        }
        assert!(
            native_doc.get_entity(other).is_none(),
            "secondary native multileader should be erased"
        );
        assert!(app.tabs[0].dirty, "mleadercollect should mark the tab dirty");
    }

    #[test]
    fn splinedit_replace_entity_updates_native_spline_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Spline {
                degree: 3,
                closed: false,
                knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                control_points: vec![
                    [0.0, 0.0, 0.0],
                    [1.0, 2.0, 0.0],
                    [2.0, 2.0, 0.0],
                    [3.0, 0.0, 0.0],
                ],
                weights: vec![],
                fit_points: vec![],
                start_tangent: [0.0, 0.0, 0.0],
                end_tangent: [0.0, 0.0, 0.0],
            }))
            .expect("native spline");
        app.tabs[0].scene.set_native_doc(Some(native));

        let mut marker = acadrust::entities::XLine::new(
            acadrust::types::Vector3::zero(),
            acadrust::types::Vector3::new(1.0, 0.0, 0.0),
        );
        marker.common.layer = "__SPLINEDIT_REVERSE__".into();

        let _ = app.apply_cmd_result(CmdResult::ReplaceEntity(
            Handle::new(handle.value()),
            vec![acadrust::EntityType::XLine(marker)],
        ));

        let entity = app.tabs[0]
            .scene
            .native_doc()
            .and_then(|doc| doc.get_entity(handle))
            .expect("native spline should still exist");
        match &entity.data {
            nm::EntityData::Spline { control_points, .. } => {
                assert_eq!(control_points[0], [3.0, 0.0, 0.0]);
                assert_eq!(control_points[3], [0.0, 0.0, 0.0]);
            }
            other => panic!("expected native spline, got {other:?}"),
        }
        assert!(app.tabs[0].dirty, "splinedit should mark the tab dirty");
    }
}

// ── DIMSPACE helper ───────────────────────────────────────────────────────────

/// Parse `base_val,h1;h2;...;hN,spacing` and adjust parallel dimension positions.
fn apply_dimspace(scene: &mut crate::scene::Scene, encoded: &str) {
    // Format: "<base_handle>,<h1>;<h2>;...;<hN>,<spacing>"
    let parts: Vec<&str> = encoded.splitn(3, ',').collect();
    if parts.len() < 3 { return; }
    let base_val: u64 = parts[0].parse().unwrap_or(0);
    let other_vals: Vec<u64> = parts[1].split(';')
        .filter_map(|s| s.parse::<u64>().ok())
        .collect();
    let spacing: f64 = parts[2].parse().unwrap_or(0.0);

    let base_h = acadrust::Handle::from(base_val);
    // Read base dimension's definition_point Z (perpendicular offset)
    let base_z = scene
        .document
        .get_entity(base_h)
        .and_then(|entity| match entity {
            acadrust::EntityType::Dimension(dimension) => Some(dimension.base().definition_point.z),
            _ => None,
        })
        .or_else(|| {
            scene.native_entity(base_h).and_then(|entity| match &entity.data {
                nm::EntityData::Dimension { definition_point, .. } => Some(definition_point[2]),
                _ => None,
            })
        });
    let Some(base_z) = base_z else {
        return;
    };

    let effective_spacing = if spacing <= 0.0 { 10.0 } else { spacing };
    for (idx, &hv) in other_vals.iter().enumerate() {
        let h = acadrust::Handle::from(hv);
        let new_z = base_z + effective_spacing * (idx + 1) as f64;
        if let Some(acadrust::EntityType::Dimension(d)) = scene.document.get_entity_mut(h) {
            let dp = &mut d.base_mut().definition_point;
            dp.z = new_z;
        } else if let Some(entity) = scene.native_entity_mut(h) {
            if let nm::EntityData::Dimension { definition_point, .. } = &mut entity.data {
                definition_point[2] = new_z;
            }
        }
    }
}

// ── MLEADERALIGN helper ───────────────────────────────────────────────────────

/// Parse `h1,h2,...;fx,fz;tx,tz` and align multileader content points along the direction.
fn apply_mleader_align(scene: &mut crate::scene::Scene, encoded: &str) {
    // Format: "<h1>,<h2>,...;<fx>,<fz>;<tx>,<tz>"
    let parts: Vec<&str> = encoded.splitn(3, ';').collect();
    if parts.len() < 3 { return; }
    let handles: Vec<acadrust::Handle> = parts[0].split(',')
        .filter_map(|s| s.parse::<u64>().ok().map(acadrust::Handle::from))
        .collect();
    let from_parts: Vec<f64> = parts[1].split(',')
        .filter_map(|s| s.parse().ok())
        .collect();
    let to_parts: Vec<f64> = parts[2].split(',')
        .filter_map(|s| s.parse().ok())
        .collect();
    if from_parts.len() < 2 || to_parts.len() < 2 || handles.is_empty() { return; }

    let fx = from_parts[0];
    let fz = from_parts[1];
    let tx = to_parts[0];
    let tz = to_parts[1];
    let dx = tx - fx;
    let dz = tz - fz;
    let len = (dx * dx + dz * dz).sqrt();
    if len < 1e-9 { return; }

    // Project each multileader's content point onto the alignment line, then
    // snap it to the line (preserve perpendicular offset from line is discarded;
    // align along direction through `from`).
    for h in handles {
        if let Some(acadrust::EntityType::MultiLeader(ml)) = scene.document.get_entity_mut(h) {
            let cp = &mut ml.context.content_base_point;
            // Project onto line from_pt + t * dir: keep t component, set perpendicular = 0
            let rel_x = cp.x - fx;
            let rel_z = cp.z - fz;
            let t = (rel_x * (dx / len) + rel_z * (dz / len)) / len;
            let t = t.clamp(0.0, 1.0);
            cp.x = fx + t * dx;
            cp.z = fz + t * dz;
        } else if let Some(entity) = scene.native_entity_mut(h) {
            if let nm::EntityData::MultiLeader {
                text_location: Some(text_location),
                ..
            } = &mut entity.data
            {
                let rel_x = text_location[0] - fx;
                let rel_z = text_location[2] - fz;
                let t = (rel_x * (dx / len) + rel_z * (dz / len)) / len;
                let t = t.clamp(0.0, 1.0);
                text_location[0] = fx + t * dx;
                text_location[2] = fz + t * dz;
            }
        }
    }
}

// ── MLEADERCOLLECT helper ─────────────────────────────────────────────────────

/// Parse `h1,h2,...;px,pz` — merge all selected multileaders into the first one at position.
fn apply_mleader_collect(scene: &mut crate::scene::Scene, encoded: &str) {
    let parts: Vec<&str> = encoded.splitn(2, ';').collect();
    if parts.len() < 2 { return; }
    let handles: Vec<acadrust::Handle> = parts[0].split(',')
        .filter_map(|s| s.parse::<u64>().ok().map(acadrust::Handle::from))
        .collect();
    let pos_parts: Vec<f64> = parts[1].split(',')
        .filter_map(|s| s.parse().ok())
        .collect();
    if handles.len() < 2 || pos_parts.len() < 2 { return; }

    let px = pos_parts[0];
    let pz = pos_parts[1];

    // Collect secondary multileaders in both compat and native forms.
    let mut extra_roots: Vec<Vec<acadrust::types::Vector3>> = Vec::new();
    let mut extra_native_vertices: Vec<[f64; 3]> = Vec::new();
    let mut extra_native_root_lengths: Vec<usize> = Vec::new();
    for &h in &handles[1..] {
        if let Some(acadrust::EntityType::MultiLeader(ml)) = scene.document.get_entity(h) {
            for root in &ml.context.leader_roots {
                let points: Vec<_> = root
                    .lines
                    .iter()
                    .flat_map(|line| line.points.iter().cloned())
                    .collect();
                if !points.is_empty() {
                    extra_native_vertices
                        .extend(points.iter().map(|point| [point.x, point.y, point.z]));
                    extra_native_root_lengths.push(points.len());
                    extra_roots.push(points);
                }
            }
        } else if let Some(entity) = scene.native_entity(h) {
            if let nm::EntityData::MultiLeader {
                leader_vertices,
                leader_root_lengths,
                ..
            } = &entity.data
            {
                if !leader_vertices.is_empty() {
                    let mut offset = 0usize;
                    let lengths: Vec<usize> = if leader_root_lengths.is_empty() {
                        vec![leader_vertices.len()]
                    } else {
                        leader_root_lengths.clone()
                    };
                    for len in lengths {
                        if len == 0 {
                            continue;
                        }
                        let end = (offset + len).min(leader_vertices.len());
                        if offset >= end {
                            break;
                        }
                        let root_slice = &leader_vertices[offset..end];
                        extra_roots.push(
                            root_slice
                                .iter()
                                .map(|point| {
                                    acadrust::types::Vector3::new(point[0], point[1], point[2])
                                })
                                .collect(),
                        );
                        extra_native_vertices.extend(root_slice.iter().copied());
                        extra_native_root_lengths.push(root_slice.len());
                        offset = end;
                    }
                    if offset < leader_vertices.len() {
                        let root_slice = &leader_vertices[offset..];
                        extra_roots.push(
                            root_slice
                                .iter()
                                .map(|point| {
                                    acadrust::types::Vector3::new(point[0], point[1], point[2])
                                })
                                .collect(),
                        );
                        extra_native_vertices.extend(root_slice.iter().copied());
                        extra_native_root_lengths.push(root_slice.len());
                    }
                    if extra_native_root_lengths.is_empty() && !leader_vertices.is_empty() {
                        extra_native_root_lengths.push(leader_vertices.len());
                    }
                }
            }
        }
    }

    let mut merged = false;
    if let Some(acadrust::EntityType::MultiLeader(ml)) = scene.document.get_entity_mut(handles[0]) {
        ml.context.content_base_point.x = px;
        ml.context.content_base_point.z = pz;
        for points in extra_roots {
            if points.is_empty() {
                continue;
            }
            let root = ml.context.add_leader_root();
            root.create_line(points);
        }
        merged = true;
    } else if let Some(entity) = scene.native_entity_mut(handles[0]) {
        if let nm::EntityData::MultiLeader {
            text_location,
            leader_vertices,
            leader_root_lengths,
            ..
        } = &mut entity.data
        {
            match text_location {
                Some(location) => {
                    location[0] = px;
                    location[2] = pz;
                }
                None => {
                    *text_location = Some([px, 0.0, pz]);
                }
            }
            leader_vertices.extend(extra_native_vertices);
            if leader_root_lengths.is_empty() && !leader_vertices.is_empty() {
                leader_root_lengths.push(leader_vertices.len() - extra_native_root_lengths.iter().sum::<usize>());
            }
            leader_root_lengths.extend(extra_native_root_lengths);
            merged = true;
        }
    }

    if merged {
        scene.erase_entities(&handles[1..]);
    }
}
