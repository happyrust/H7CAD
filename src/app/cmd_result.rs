use super::{H7CAD, Message};
use crate::command::CmdResult;
use acadrust::Handle;
use iced::Task;

impl H7CAD {
    pub(super) fn apply_cmd_result(&mut self, result: CmdResult) -> Task<Message> {
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

                let props = self.tabs[i].scene.document.get_entity(src).map(|e| {
                    let c = e.common();
                    (c.layer.clone(), c.color, c.linetype.clone(), c.linetype_scale, c.line_weight)
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
                let result = self.tabs[i].scene.document
                    .get_entity(handle)
                    .and_then(|e| lengthen_entity(e, pick_pt, &mode));
                match result {
                    Some(new_entity) => {
                        let label = self.history_label_from_active_cmd(i, "LENGTHEN");
                        self.push_undo_snapshot(i, label);
                        self.tabs[i].scene.erase_entities(&[handle]);
                        self.tabs[i].scene.add_entity(new_entity);
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
                let pts = self.tabs[i].scene.document
                    .get_entity(handle)
                    .map(|e| divide_entity(e, n))
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
                let pts = self.tabs[i].scene.document
                    .get_entity(handle)
                    .map(|e| measure_entity(e, segment_length))
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
                let changed = self.tabs[i].scene.document
                    .get_entity_mut(handle)
                    .map(|e| apply_pedit(e, &op))
                    .unwrap_or(false);
                if changed {
                    self.push_undo_snapshot(i, "PEDIT");
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
                let pairs: Vec<_> = handles.iter()
                    .filter_map(|&h| self.tabs[i].scene.document.get_entity(h).map(|e| (h, e)))
                    .collect();
                match join_entities(&pairs) {
                    Some((to_remove, merged)) => {
                        let label = self.history_label_from_active_cmd(i, "JOIN");
                        self.push_undo_snapshot(i, label);
                        self.tabs[i].scene.erase_entities(&to_remove);
                        let count_in = to_remove.len();
                        let count_out = merged.len();
                        for e in merged {
                            self.tabs[i].scene.add_entity(e);
                        }
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
                let replacement = self.tabs[i].scene.document
                    .get_entity(handle)
                    .and_then(|e| break_entity(e, p1, p2));
                match replacement {
                    Some(frags) => {
                        let label = self.history_label_from_active_cmd(i, "BREAK");
                        self.push_undo_snapshot(i, label);
                        self.tabs[i].scene.erase_entities(&[handle]);
                        let count = frags.len();
                        for e in frags {
                            self.tabs[i].scene.add_entity(e);
                        }
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
