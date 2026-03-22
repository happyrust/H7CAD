use super::{H7CAD, Message};
use crate::command::CadCommand;
use crate::scene::Scene;
use iced::Task;
use std::path::PathBuf;

impl H7CAD {
    pub(super) fn dispatch_command(&mut self, cmd: &str) -> Task<Message> {
        let i = self.active_tab;
        // Cancel any running command before starting a new one.
        if self.tabs[i].active_cmd.is_some() {
            self.tabs[i].scene.clear_preview_wire();
            self.tabs[i].active_cmd = None;
        }

        if let Some(path_str) = cmd.strip_prefix("OPEN_RECENT:") {
            let path = PathBuf::from(path_str);
            return Task::perform(crate::io::open_path(path), Message::FileOpened);
        }

        match cmd {
            "NEW"                => return Task::done(Message::ClearScene),
            "OPEN"               => return Task::done(Message::OpenFile),
            "SAVE"|"QSAVE"       => return Task::done(Message::SaveFile),
            "SAVEAS"             => return Task::done(Message::SaveAs),
            "UNDO"|"U"           => return Task::done(Message::Undo),
            "REDO"               => return Task::done(Message::Redo),
            "CLEAR"|"CLR"        => return Task::done(Message::ClearScene),
            "WIREFRAME"|"VW"     => return Task::done(Message::SetWireframe(true)),
            "SOLID"|"VS"         => return Task::done(Message::SetWireframe(false)),
            "ORTHO"              => return Task::done(Message::SetProjection(true)),
            "PERSP"              => return Task::done(Message::SetProjection(false)),
            "LAYERS"|"LA"        => return Task::done(Message::ToggleLayers),

            // ── Layer object commands ──────────────────────────────────────
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
                    .scene.selected_entities().into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("GROUP");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    let auto_name = super::helpers::next_group_auto_name(&self.tabs[i].scene);
                    use crate::command::group::GroupCommand;
                    let cmd = GroupCommand::new(handles, auto_name);
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                }
            }

            "UNGROUP"|"UG" => {
                let handles: Vec<_> = self.tabs[i]
                    .scene.selected_entities().into_iter().map(|(h, _)| h).collect();
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
                    self.clipboard_centroid = super::helpers::entities_centroid(
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
                    self.clipboard_centroid = super::helpers::entities_centroid(
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
                    .scene.selected_entities().into_iter().map(|(h, _)| h).collect();
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

            // ── Draw commands ──────────────────────────────────────────────
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

            // ── Modify commands ────────────────────────────────────────────
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

            // ── Annotate commands ──────────────────────────────────────────
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
                    let wires = self.tabs[i].scene.wire_models_for(&handles);
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
                let entities: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().collect();
                if entities.is_empty() {
                    use crate::command::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("EXPLODE");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
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

            "3DORBIT"|"3O" => {
                self.command_line.push_info("3D Orbit: drag with right mouse button.");
            }

            "HELP"|"?" => {
                self.command_line.push_output(
                    "Draw: LINE CIRCLE ARC PLINE POINT ELLIPSE SPLINE  |  \
                     Modify: MOVE COPY ROTATE SCALE MIRROR ERASE  |  \
                     Text: TEXT MTEXT  |  File: OPEN SAVE SAVEAS"
                );
            }

            "DONATE" => {
                let _ = open::that("https://patreon.com/HakanSeven12");
                self.command_line.push_info("Opening Patreon page...");
            }

            // ── Layout / viewport ──────────────────────────────────────────
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

            _ => self.command_line.push_error(&format!("Unknown command: {cmd}")),
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
}
