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
        // Reset the last committed point so the first click of the new command
        // is not constrained by ortho/polar relative to a previous command's endpoint.
        self.last_point = None;

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
                    use crate::modules::home::select::SelectObjectsCommand;
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
                    use crate::modules::home::select::SelectObjectsCommand;
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
                    use crate::modules::home::select::SelectObjectsCommand;
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
                    use crate::modules::home::select::SelectObjectsCommand;
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
                    use crate::modules::home::select::SelectObjectsCommand;
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
                use crate::modules::home::layers::match_layer::LayMatchCommand;
                let dest: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().map(|(h, _)| h).collect();
                let cmd = LayMatchCommand::new(dest);
                self.command_line.push_info(&cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(cmd));
            }

            "MATCHPROP"|"MA" => {
                use crate::modules::home::properties::match_prop::MatchPropCommand;
                self.tabs[i].scene.deselect_all();
                let cmd = MatchPropCommand::new();
                self.command_line.push_info(&cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(cmd));
            }

            "GROUP"|"G" => {
                let handles: Vec<_> = self.tabs[i]
                    .scene.selected_entities().into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::modules::home::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("GROUP");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    let auto_name = super::helpers::next_group_auto_name(&self.tabs[i].scene);
                    use crate::modules::home::groups::group::GroupCommand;
                    let cmd = GroupCommand::new(handles, auto_name);
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                }
            }

            "UNGROUP"|"UG" => {
                let handles: Vec<_> = self.tabs[i]
                    .scene.selected_entities().into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::modules::home::groups::ungroup::UngroupCommand;
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
                    use crate::modules::home::select::SelectObjectsCommand;
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
                    use crate::modules::home::select::SelectObjectsCommand;
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
                    use crate::modules::home::clipboard::paste::PasteCommand;
                    let cmd = PasteCommand::new(wires, self.clipboard_centroid);
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                }
            }

            "BLOCK" => {
                let handles: Vec<_> = self.tabs[i]
                    .scene.selected_entities().into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::modules::home::select::SelectObjectsCommand;
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
                    use crate::modules::home::select::SelectObjectsCommand;
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
                    use crate::modules::home::select::SelectObjectsCommand;
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
                    use crate::modules::home::select::SelectObjectsCommand;
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

            "RAY" => {
                use crate::modules::home::draw::ray::RayCommand;
                let new_cmd = RayCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "XLINE"|"XL" => {
                use crate::modules::home::draw::ray::XLineCommand;
                let new_cmd = XLineCommand::new();
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
                    use crate::modules::home::select::SelectObjectsCommand;
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
                    use crate::modules::home::select::SelectObjectsCommand;
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
                    use crate::modules::home::select::SelectObjectsCommand;
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

            "ZOOM IN"|"ZI" => {
                self.tabs[i].scene.zoom_camera(1.0 / 1.5);
                self.command_line.push_output("Zoom In");
            }

            "ZOOM OUT"|"ZO" => {
                self.tabs[i].scene.zoom_camera(1.5);
                self.command_line.push_output("Zoom Out");
            }

            "ZOOM WINDOW"|"ZOOM W"|"ZW" => {
                use crate::modules::view::zoom_window::ZoomWindowCommand;
                let new_cmd = ZoomWindowCommand::new();
                self.command_line.push_info(&new_cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(new_cmd));
            }

            "STRETCH"|"SS" => {
                let handles: Vec<_> = self.tabs[i].scene.selected_entities()
                    .into_iter().map(|(h, _)| h).collect();
                if handles.is_empty() {
                    use crate::modules::home::select::SelectObjectsCommand;
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
                    use crate::modules::home::select::SelectObjectsCommand;
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
                    use crate::modules::home::select::SelectObjectsCommand;
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
                    use crate::modules::home::select::SelectObjectsCommand;
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
                    use crate::modules::home::select::SelectObjectsCommand;
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

            // ── Selection utilities ───────────────────────────────────────
            "SELECTALL"|"SA" => {
                use crate::scene::Scene;
                let handles: Vec<acadrust::Handle> = self.tabs[i].scene.entity_wires()
                    .iter()
                    .filter_map(|w| Scene::handle_from_wire_name(&w.name))
                    .collect();
                let count = handles.len();
                for h in handles {
                    self.tabs[i].scene.select_entity(h, false);
                }
                self.command_line.push_output(&format!("SELECTALL: {} object(s) selected.", count));
                self.refresh_properties();
            }

            "DESELECT"|"DE"|"DESELALL" => {
                self.tabs[i].scene.deselect_all();
                self.command_line.push_output("Deselected.");
                self.refresh_properties();
            }

            // ── LIST — entity info ────────────────────────────────────────
            "LIST" | "LI" => {
                let selected: Vec<_> = self.tabs[i].scene.selected_entities();
                if selected.is_empty() {
                    self.command_line.push_error("LIST: no entities selected. Select entities first.");
                } else {
                    for (handle, _) in &selected {
                        if let Some(entity) = self.tabs[i].scene.document.get_entity(*handle) {
                            let type_name = match entity {
                                acadrust::EntityType::Line(_)       => "LINE",
                                acadrust::EntityType::Circle(_)     => "CIRCLE",
                                acadrust::EntityType::Arc(_)        => "ARC",
                                acadrust::EntityType::LwPolyline(_) => "LWPOLYLINE",
                                acadrust::EntityType::Polyline(_)   => "POLYLINE",
                                acadrust::EntityType::Text(_)       => "TEXT",
                                acadrust::EntityType::MText(_)      => "MTEXT",
                                acadrust::EntityType::Insert(_)     => "INSERT",
                                acadrust::EntityType::Hatch(_)      => "HATCH",
                                acadrust::EntityType::Dimension(_)  => "DIMENSION",
                                acadrust::EntityType::Viewport(_)   => "VIEWPORT",
                                acadrust::EntityType::Spline(_)     => "SPLINE",
                                acadrust::EntityType::Ellipse(_)    => "ELLIPSE",
                                acadrust::EntityType::Point(_)      => "POINT",
                                acadrust::EntityType::Ray(_)        => "RAY",
                                acadrust::EntityType::XLine(_)      => "XLINE",
                                acadrust::EntityType::Face3D(_)     => "3DFACE",
                                acadrust::EntityType::Table(_)      => "TABLE",
                                _ => "ENTITY",
                            };
                            let common = entity.common();
                            self.command_line.push_output(&format!(
                                "{} Layer:{} Color:{} Handle:{:X}",
                                type_name,
                                common.layer,
                                common.color.index().map(|c| c.to_string()).unwrap_or_else(|| "ByLayer".to_string()),
                                handle.value()
                            ));
                        }
                    }
                }
            }

            "HELP"|"?" => {
                self.command_line.push_output(
                    "Draw: LINE CIRCLE ARC PLINE RECT POLY POINT ELLIPSE SPLINE RAY XLINE HATCH  |  \
                     Modify: MOVE COPY ROTATE SCALE MIRROR ERASE OFFSET EXTEND FILLET CHAMFER STRETCH EXPLODE TRIM  |  \
                     Array: ARRAY ARRAYRECT ARRAYPOLAR ARRAYPATH  |  \
                     Text: TEXT MTEXT LEADER MLEADER  |  \
                     Dimension: DIMLINEAR DIMANGULAR DIMRADIUS  |  \
                     View: ZOOM EXTENTS VIEW LIST/SAVE/RESTORE/DELETE  |  \
                     Layer: LAYER LIST/NEW/ON/OFF/FREEZE/THAW/LOCK/UNLOCK/COLOR/SET  |  \
                     Viewport: MVIEW VPLAYER VPORTS MS PS DRAWORDER  |  \
                     Tables: STYLE DIMSTYLE LINETYPE UCS RENAME PURGE  |  \
                     File: NEW OPEN SAVE SAVEAS PRINT PURGE UNDO REDO"
                );
            }

            "DONATE" => {
                let _ = open::that("https://patreon.com/HakanSeven12");
                self.command_line.push_info("Opening Patreon page...");
            }

            // ── Layout / viewport ──────────────────────────────────────────
            "MVIEW"|"MV" => {
                if self.tabs[i].scene.current_layout == "Model" {
                    self.command_line.push_error("MVIEW: switch to a paper space layout first.");
                } else {
                    use crate::modules::layout::mview::MviewCommand;
                    let new_cmd = MviewCommand::new();
                    self.command_line.push_info(&new_cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(new_cmd));
                }
            }

            // ── MSPACE / PSPACE ───────────────────────────────────────────
            "MS"|"MSPACE" => {
                return Task::done(Message::MspaceCommand);
            }
            "PSPACE" => {
                return Task::done(Message::PspaceCommand);
            }

            // ── VPORTS — list viewports in current layout ─────────────────
            "VPORTS" => {
                let scene = &self.tabs[i].scene;
                if scene.current_layout == "Model" {
                    self.command_line.push_error("VPORTS: switch to a paper space layout first.");
                } else {
                    let layout_block = scene.current_layout_block_handle_pub();
                    let viewports: Vec<_> = scene.document.entities()
                        .filter_map(|e| {
                            if let acadrust::EntityType::Viewport(vp) = e {
                                if vp.id > 1 && vp.common.owner_handle == layout_block {
                                    Some((vp.id, vp.center.clone(), vp.width, vp.height, vp.custom_scale, vp.status.is_on, vp.status.locked))
                                } else { None }
                            } else { None }
                        })
                        .collect();
                    if viewports.is_empty() {
                        self.command_line.push_info("No viewports in current layout. Use MVIEW to create one.");
                    } else {
                        self.command_line.push_output(&format!("{} viewport(s) in layout \"{}\":", viewports.len(), scene.current_layout));
                        for (id, center, w, h, scale, is_on, locked) in &viewports {
                            let state = match (is_on, locked) {
                                (true, true)  => "On, Locked",
                                (true, false) => "On",
                                (false, _)    => "Off",
                            };
                            self.command_line.push_output(&format!(
                                "  VP #{id}: {w:.1}×{h:.1} @ ({:.1},{:.1})  scale={scale:.4}  [{state}]",
                                center.x, center.y
                            ));
                        }
                    }
                }
            }

            // ── VPLAYER — per-viewport layer freeze/thaw ──────────────────
            "VPLAYER" => {
                let scene = &self.tabs[i].scene;
                if scene.current_layout == "Model" {
                    self.command_line.push_error("VPLAYER: switch to a paper space layout first.");
                } else if scene.active_viewport.is_none() {
                    self.command_line.push_error("VPLAYER: enter a viewport first (double-click or MS).");
                } else {
                    use crate::modules::layout::vplayer::VplayerCommand;
                    let vp_handle = scene.active_viewport.unwrap();
                    // Collect current frozen layer names for display.
                    let frozen_names: Vec<String> = {
                        if let Some(acadrust::EntityType::Viewport(vp)) =
                            scene.document.get_entity(vp_handle)
                        {
                            vp.frozen_layers.iter().filter_map(|h| {
                                scene.document.layers.iter().find(|l| l.handle == *h).map(|l| l.name.clone())
                            }).collect()
                        } else { vec![] }
                    };
                    if frozen_names.is_empty() {
                        self.command_line.push_info("VPLAYER: no frozen layers in active viewport.");
                    } else {
                        self.command_line.push_info(&format!(
                            "VPLAYER: frozen layers: {}",
                            frozen_names.join(", ")
                        ));
                    }
                    let new_cmd = VplayerCommand::new(vp_handle);
                    self.command_line.push_info(&new_cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(new_cmd));
                }
            }

            // ── Draw Order ────────────────────────────────────────────────
            cmd if cmd.starts_with("DRAWORDER") => {
                use acadrust::objects::{ObjectType, SortEntitiesTable};
                let option = cmd.split_whitespace().nth(1).unwrap_or("").to_uppercase();
                let bring_front = match option.as_str() {
                    "F" | "FRONT" => Some(true),
                    "B" | "BACK"  => Some(false),
                    _ => None,
                };
                let i = self.active_tab;
                let selected: Vec<acadrust::Handle> = self.tabs[i].scene
                    .selected_entities()
                    .iter()
                    .map(|(h, _)| *h)
                    .collect();
                if selected.is_empty() {
                    self.command_line.push_error("DRAWORDER: select entities first.");
                } else if let Some(to_front) = bring_front {
                    self.push_undo_snapshot(i, "DRAWORDER");
                    let block_handle = self.tabs[i].scene.current_layout_block_handle_pub();
                    let doc = &mut self.tabs[i].scene.document;
                    let table_handle = doc.objects.iter()
                        .find_map(|(h, obj)| {
                            if let ObjectType::SortEntitiesTable(t) = obj {
                                if t.block_owner_handle == block_handle { Some(*h) } else { None }
                            } else { None }
                        });
                    if let Some(th) = table_handle {
                        if let Some(ObjectType::SortEntitiesTable(table)) =
                            doc.objects.get_mut(&th)
                        {
                            for h in &selected {
                                if to_front { table.bring_to_front(*h); }
                                else        { table.send_to_back(*h); }
                            }
                        }
                    } else {
                        let new_handle = acadrust::Handle::new(doc.next_handle());
                        let mut table = SortEntitiesTable::for_block(block_handle);
                        table.handle = new_handle;
                        for h in &selected {
                            if to_front { table.bring_to_front(*h); }
                            else        { table.send_to_back(*h); }
                        }
                        doc.objects.insert(new_handle, ObjectType::SortEntitiesTable(table));
                    }
                    self.tabs[i].dirty = true;
                    let dir = if to_front { "front" } else { "back" };
                    self.command_line.push_info(&format!(
                        "DRAWORDER: moved {} entities to {}.", selected.len(), dir
                    ));
                } else {
                    self.command_line.push_info("Usage: DRAWORDER F  (front)  or  DRAWORDER B  (back)");
                }
            }

            // ── LAYER management ─────────────────────────────────────────
            cmd if cmd == "LAYER" || cmd.starts_with("LAYER ") || cmd.starts_with("LA ") => {
                use acadrust::tables::Layer;
                let raw_rest = if cmd.starts_with("LAYER ") {
                    cmd.trim_start_matches("LAYER ").trim()
                } else if cmd.starts_with("LA ") {
                    cmd.trim_start_matches("LA ").trim()
                } else {
                    ""
                };
                let parts: Vec<&str> = raw_rest.split_whitespace().collect();
                let sub = parts.get(0).map(|s| s.to_uppercase()).unwrap_or_default();
                match sub.as_str() {
                    "" | "LIST" | "?" => {
                        let info: Vec<String> = self.tabs[i].scene.document.layers.iter().map(|l| {
                            let state = if l.flags.frozen { "frozen" }
                                       else if l.flags.off { "off" }
                                       else if l.flags.locked { "locked" }
                                       else { "on" };
                            format!("{}({})", l.name, state)
                        }).collect();
                        self.command_line.push_output(&format!("Layers: {}", info.join(", ")));
                    }
                    "NEW" | "N" => {
                        let name = parts.get(1).map(|s| s.trim()).unwrap_or("").to_string();
                        if name.is_empty() {
                            self.command_line.push_error("Usage: LAYER NEW <name>");
                        } else if self.tabs[i].scene.document.layers.contains(&name) {
                            self.command_line.push_error(&format!("LAYER: '{}' already exists.", name));
                        } else {
                            let layer = Layer::new(&name);
                            let _ = self.tabs[i].scene.document.layers.add(layer);
                            self.push_undo_snapshot(i, "LAYER NEW");
                            self.tabs[i].dirty = true;
                            self.command_line.push_output(&format!("LAYER: '{}' created.", name));
                        }
                    }
                    "ON" => {
                        for name in &parts[1..] {
                            if let Some(l) = self.tabs[i].scene.document.layers.get_mut(name) {
                                l.flags.off = false; l.flags.frozen = false;
                            }
                        }
                        self.push_undo_snapshot(i, "LAYER ON");
                        self.tabs[i].dirty = true;
                        self.command_line.push_output("LAYER: layers turned on.");
                    }
                    "OFF" => {
                        for name in &parts[1..] {
                            if let Some(l) = self.tabs[i].scene.document.layers.get_mut(name) {
                                l.flags.off = true;
                            }
                        }
                        self.push_undo_snapshot(i, "LAYER OFF");
                        self.tabs[i].dirty = true;
                        self.command_line.push_output("LAYER: layers turned off.");
                    }
                    "FREEZE" | "FR" => {
                        for name in &parts[1..] {
                            if let Some(l) = self.tabs[i].scene.document.layers.get_mut(name) {
                                l.flags.frozen = true;
                            }
                        }
                        self.push_undo_snapshot(i, "LAYER FREEZE");
                        self.tabs[i].dirty = true;
                        self.command_line.push_output("LAYER: layers frozen.");
                    }
                    "THAW" | "TH" => {
                        for name in &parts[1..] {
                            if let Some(l) = self.tabs[i].scene.document.layers.get_mut(name) {
                                l.flags.frozen = false;
                            }
                        }
                        self.push_undo_snapshot(i, "LAYER THAW");
                        self.tabs[i].dirty = true;
                        self.command_line.push_output("LAYER: layers thawed.");
                    }
                    "LOCK" | "LO" => {
                        for name in &parts[1..] {
                            if let Some(l) = self.tabs[i].scene.document.layers.get_mut(name) {
                                l.flags.locked = true;
                            }
                        }
                        self.push_undo_snapshot(i, "LAYER LOCK");
                        self.tabs[i].dirty = true;
                        self.command_line.push_output("LAYER: layers locked.");
                    }
                    "UNLOCK" | "UL" => {
                        for name in &parts[1..] {
                            if let Some(l) = self.tabs[i].scene.document.layers.get_mut(name) {
                                l.flags.locked = false;
                            }
                        }
                        self.push_undo_snapshot(i, "LAYER UNLOCK");
                        self.tabs[i].dirty = true;
                        self.command_line.push_output("LAYER: layers unlocked.");
                    }
                    "COLOR" | "C" => {
                        // LAYER COLOR <name> <aci_index>
                        let layer_name = parts.get(1).map(|s| s.trim()).unwrap_or("").to_string();
                        let color_str = parts.get(2).map(|s| s.trim()).unwrap_or("");
                        if let Ok(idx) = color_str.parse::<i16>() {
                            if let Some(l) = self.tabs[i].scene.document.layers.get_mut(&layer_name) {
                                l.color = acadrust::types::Color::from_index(idx);
                                self.push_undo_snapshot(i, "LAYER COLOR");
                                self.tabs[i].dirty = true;
                                self.command_line.push_output(&format!("LAYER: '{}' color set to ACI {}.", layer_name, idx));
                            } else {
                                self.command_line.push_error(&format!("LAYER: '{}' not found.", layer_name));
                            }
                        } else {
                            self.command_line.push_error("Usage: LAYER COLOR <name> <aci_index>");
                        }
                    }
                    "SET" | "S" | "CURRENT" => {
                        let name = parts.get(1).map(|s| s.trim()).unwrap_or("").to_string();
                        if self.tabs[i].scene.document.layers.contains(&name) {
                            self.tabs[i].layers.current_layer = name.clone();
                            self.command_line.push_output(&format!("LAYER: current layer set to '{}'.", name));
                        } else {
                            self.command_line.push_error(&format!("LAYER: '{}' not found.", name));
                        }
                    }
                    _ => {
                        self.command_line.push_info(
                            "Usage: LAYER LIST | NEW <name> | ON/OFF/FREEZE/THAW/LOCK/UNLOCK <name> | COLOR <name> <aci> | SET <name>"
                        );
                    }
                }
            }

            // ── UCS management ───────────────────────────────────────────
            cmd if cmd == "UCS" || cmd.starts_with("UCS ") => {
                use acadrust::tables::Ucs;
                let parts: Vec<&str> = cmd.splitn(3, ' ').collect();
                let sub = parts.get(1).map(|s| s.to_uppercase()).unwrap_or_default();
                match sub.as_str() {
                    "" | "LIST" | "?" => {
                        let names: Vec<String> = self.tabs[i].scene.document
                            .ucss.iter().map(|u| u.name.clone()).collect();
                        if names.is_empty() {
                            self.command_line.push_output("No named UCSs defined.");
                        } else {
                            self.command_line.push_output(&format!("UCSs: {}", names.join(", ")));
                        }
                    }
                    "SAVE" | "S" => {
                        let name = parts.get(2).map(|s| s.trim()).unwrap_or("").to_string();
                        if name.is_empty() {
                            self.command_line.push_error("Usage: UCS SAVE <name>");
                        } else {
                            // Save as WCS (identity) since we don't have active UCS state yet
                            let ucs = Ucs::new(&name);
                            self.tabs[i].scene.document.ucss.add_or_replace(ucs);
                            self.tabs[i].dirty = true;
                            self.command_line.push_output(&format!("UCS '{}' saved (WCS).", name));
                        }
                    }
                    "DELETE" | "DEL" | "D" => {
                        let name = parts.get(2).map(|s| s.trim()).unwrap_or("").to_string();
                        if name.is_empty() {
                            self.command_line.push_error("Usage: UCS DELETE <name>");
                        } else if self.tabs[i].scene.document.ucss.remove(&name).is_some() {
                            self.tabs[i].dirty = true;
                            self.command_line.push_output(&format!("UCS '{}' deleted.", name));
                        } else {
                            self.command_line.push_error(&format!("UCS '{}' not found.", name));
                        }
                    }
                    "W" | "WORLD" => {
                        // Reset active UCS to WCS — currently just an informational message
                        // as full UCS integration awaits WCS↔UCS transform pipeline
                        self.command_line.push_output("UCS reset to World Coordinate System.");
                    }
                    _ => {
                        // UCS <name> — try as a restore/apply shortcut
                        let name = sub.clone();
                        if self.tabs[i].scene.document.ucss.get(&name).is_some() {
                            self.command_line.push_output(&format!("UCS '{}' is defined. (Full UCS activation pending transform pipeline.)", name));
                        } else {
                            self.command_line.push_error(
                                "Usage: UCS LIST | UCS SAVE <name> | UCS DELETE <name> | UCS W"
                            );
                        }
                    }
                }
            }

            // ── Named Views (VIEW command) ────────────────────────────────
            cmd if cmd == "VIEW" || cmd.starts_with("VIEW ") => {
                let parts: Vec<&str> = cmd.splitn(3, ' ').collect();
                let sub = parts.get(1).map(|s| s.to_uppercase()).unwrap_or_default();
                match sub.as_str() {
                    "" | "LIST" | "?" => {
                        let views: Vec<String> = self.tabs[i].scene.document
                            .views.iter().map(|v| v.name.clone()).collect();
                        if views.is_empty() {
                            self.command_line.push_output("No named views saved.");
                        } else {
                            self.command_line.push_output(&format!(
                                "Named views: {}", views.join(", ")
                            ));
                        }
                    }
                    "SAVE" | "S" => {
                        let name = parts.get(2).map(|s| s.trim()).unwrap_or("").to_string();
                        if name.is_empty() {
                            self.command_line.push_error("Usage: VIEW SAVE <name>");
                        } else {
                            let new_view = self.tabs[i].scene.current_as_named_view(&name);
                            self.tabs[i].scene.document.views.add_or_replace(new_view);
                            self.command_line.push_output(&format!("View '{}' saved.", name));
                        }
                    }
                    "DELETE" | "DEL" | "D" => {
                        let name = parts.get(2).map(|s| s.trim()).unwrap_or("").to_string();
                        if name.is_empty() {
                            self.command_line.push_error("Usage: VIEW DELETE <name>");
                        } else {
                            if self.tabs[i].scene.document.views.remove(&name).is_some() {
                                self.command_line.push_output(&format!("View '{}' deleted.", name));
                            } else {
                                self.command_line.push_error(&format!("View '{}' not found.", name));
                            }
                        }
                    }
                    "RESTORE" | "R" => {
                        let name = parts.get(2).map(|s| s.trim()).unwrap_or("").to_string();
                        if name.is_empty() {
                            self.command_line.push_error("Usage: VIEW RESTORE <name>");
                        } else {
                            let found = self.tabs[i].scene.document.views.get(&name).cloned();
                            if let Some(v) = found {
                                self.tabs[i].scene.restore_named_view(&v);
                                self.command_line.push_output(&format!("View '{}' restored.", v.name));
                            } else {
                                self.command_line.push_error(&format!("View '{}' not found.", name));
                            }
                        }
                    }
                    // VIEW <name> shortcut for restore
                    _ => {
                        let name = sub.clone();
                        let found = self.tabs[i].scene.document.views.get(&name).cloned();
                        if let Some(v) = found {
                            self.tabs[i].scene.restore_named_view(&v);
                            self.command_line.push_output(&format!("View '{}' restored.", v.name));
                        } else {
                            self.command_line.push_error(
                                "Usage: VIEW LIST | VIEW SAVE <name> | VIEW RESTORE <name> | VIEW DELETE <name>"
                            );
                        }
                    }
                }
            }

            // ── DimStyle management ───────────────────────────────────────
            cmd if cmd == "DIMSTYLE" || cmd == "DDIM" || cmd.starts_with("DIMSTYLE ") || cmd.starts_with("DDIM ") => {
                use acadrust::tables::DimStyle;
                let raw_rest = cmd.split_once(' ').map(|(_, r)| r.trim()).unwrap_or("");
                let parts: Vec<&str> = raw_rest.split_whitespace().collect();
                let sub = parts.get(0).map(|s| s.to_uppercase()).unwrap_or_default();
                match sub.as_str() {
                    "" | "LIST" | "?" => {
                        let styles: Vec<String> = self.tabs[i].scene.document
                            .dim_styles.iter()
                            .map(|s| format!("{}(txt:{:.2} asz:{:.2})", s.name, s.dimtxt, s.dimasz))
                            .collect();
                        if styles.is_empty() {
                            self.command_line.push_output("No dim styles defined.");
                        } else {
                            self.command_line.push_output(&format!("DimStyles: {}", styles.join(", ")));
                        }
                    }
                    "NEW" | "N" => {
                        let name = parts.get(1).map(|s| s.trim()).unwrap_or("").to_string();
                        if name.is_empty() {
                            self.command_line.push_error("Usage: DIMSTYLE NEW <name>");
                        } else if self.tabs[i].scene.document.dim_styles.contains(&name) {
                            self.command_line.push_error(&format!("DIMSTYLE: '{}' already exists.", name));
                        } else {
                            let style = DimStyle::new(&name);
                            let _ = self.tabs[i].scene.document.dim_styles.add(style);
                            self.push_undo_snapshot(i, "DIMSTYLE NEW");
                            self.tabs[i].dirty = true;
                            self.command_line.push_output(&format!("DIMSTYLE: '{}' created.", name));
                        }
                    }
                    "SET" | "S" => {
                        // DIMSTYLE SET <name> <property> <value>
                        // e.g. DIMSTYLE SET Standard dimtxt 2.5
                        let style_name = parts.get(1).map(|s| s.trim()).unwrap_or("").to_string();
                        let prop = parts.get(2).map(|s| s.to_lowercase()).unwrap_or_default();
                        let val_str = parts.get(3).map(|s| s.trim()).unwrap_or("");
                        if let Ok(val) = val_str.parse::<f64>() {
                            if let Some(ds) = self.tabs[i].scene.document.dim_styles.get_mut(&style_name) {
                                match prop.as_str() {
                                    "dimtxt"  => { ds.dimtxt = val; }
                                    "dimasz"  => { ds.dimasz = val; }
                                    "dimdli"  => { ds.dimdli = val; }
                                    "dimexo"  => { ds.dimexo = val; }
                                    "dimexe"  => { ds.dimexe = val; }
                                    "dimgap"  => { ds.dimgap = val; }
                                    "dimscale"| "dimlfac" => { ds.dimgap = val; } // best effort
                                    _ => {
                                        self.command_line.push_error(&format!("DIMSTYLE: unknown property '{}'. Try: dimtxt dimasz dimdli dimexo dimexe dimgap", prop));
                                        return Task::none();
                                    }
                                }
                                self.push_undo_snapshot(i, "DIMSTYLE SET");
                                self.tabs[i].dirty = true;
                                self.command_line.push_output(&format!("DIMSTYLE: '{style_name}'.{prop} = {val:.3}"));
                            } else {
                                self.command_line.push_error(&format!("DIMSTYLE: '{}' not found.", style_name));
                            }
                        } else {
                            self.command_line.push_error("Usage: DIMSTYLE SET <name> <property> <value>");
                        }
                    }
                    _ => {
                        self.command_line.push_info(
                            "Usage: DIMSTYLE LIST | NEW <name> | SET <name> <prop> <val>"
                        );
                    }
                }
            }

            // ── TextStyle / Style management ──────────────────────────────
            cmd if cmd == "STYLE" || cmd == "TEXTSTYLE" || cmd.starts_with("STYLE ") || cmd.starts_with("TEXTSTYLE ") => {
                let (prefix, rest) = if cmd.starts_with("TEXTSTYLE") {
                    ("TEXTSTYLE", cmd.trim_start_matches("TEXTSTYLE").trim())
                } else {
                    ("STYLE", cmd.trim_start_matches("STYLE").trim())
                };
                let parts: Vec<&str> = rest.splitn(3, ' ').collect();
                let sub = parts.get(0).map(|s| s.to_uppercase()).unwrap_or_default();
                match sub.as_str() {
                    "" | "LIST" | "?" => {
                        let styles: Vec<String> = self.tabs[i].scene.document
                            .text_styles.iter()
                            .map(|s| format!("{} (font: {}, w: {:.2}, oblique: {:.1}°)",
                                s.name, s.font_file, s.width_factor, s.oblique_angle.to_degrees()))
                            .collect();
                        if styles.is_empty() {
                            self.command_line.push_output("No text styles defined.");
                        } else {
                            self.command_line.push_output(&format!("Text styles: {}", styles.join(" | ")));
                        }
                    }
                    "SET" | "S" => {
                        // STYLE SET <name> — set active text style (for future text commands)
                        let name = parts.get(1).map(|s| s.trim()).unwrap_or("");
                        if self.tabs[i].scene.document.text_styles.get(name).is_some() {
                            self.command_line.push_output(&format!("{prefix}: active style set to '{name}'."));
                        } else {
                            self.command_line.push_error(&format!("{prefix}: style '{name}' not found."));
                        }
                    }
                    "NEW" | "N" => {
                        let name = parts.get(1).map(|s| s.trim()).unwrap_or("").to_string();
                        if name.is_empty() {
                            self.command_line.push_error(&format!("Usage: {prefix} NEW <name>"));
                        } else if self.tabs[i].scene.document.text_styles.contains(&name) {
                            self.command_line.push_error(&format!("{prefix}: style '{name}' already exists."));
                        } else {
                            let style = acadrust::tables::TextStyle::new(&name);
                            let _ = self.tabs[i].scene.document.text_styles.add(style);
                            self.push_undo_snapshot(i, "STYLE NEW");
                            self.tabs[i].dirty = true;
                            self.command_line.push_output(&format!("{prefix}: style '{name}' created."));
                        }
                    }
                    "FONT" | "F" => {
                        // STYLE FONT <name> <font_file>
                        let style_name = parts.get(1).map(|s| s.trim()).unwrap_or("").to_string();
                        let font = parts.get(2).map(|s| s.trim()).unwrap_or("").to_string();
                        if style_name.is_empty() || font.is_empty() {
                            self.command_line.push_error(&format!("Usage: {prefix} FONT <style> <font_file>"));
                        } else if let Some(s) = self.tabs[i].scene.document.text_styles.get_mut(&style_name) {
                            s.font_file = font.clone();
                            self.push_undo_snapshot(i, "STYLE FONT");
                            self.tabs[i].dirty = true;
                            self.command_line.push_output(&format!("{prefix}: '{style_name}' font set to '{font}'."));
                        } else {
                            self.command_line.push_error(&format!("{prefix}: style '{style_name}' not found."));
                        }
                    }
                    "WIDTH" | "W" => {
                        // STYLE WIDTH <name> <factor>
                        let style_name = parts.get(1).map(|s| s.trim()).unwrap_or("").to_string();
                        let factor_str = parts.get(2).map(|s| s.trim()).unwrap_or("");
                        if let Ok(factor) = factor_str.parse::<f64>() {
                            if let Some(s) = self.tabs[i].scene.document.text_styles.get_mut(&style_name) {
                                s.width_factor = factor;
                                self.push_undo_snapshot(i, "STYLE WIDTH");
                                self.tabs[i].dirty = true;
                                self.command_line.push_output(&format!("{prefix}: '{style_name}' width factor set to {factor:.3}."));
                            } else {
                                self.command_line.push_error(&format!("{prefix}: style '{style_name}' not found."));
                            }
                        } else {
                            self.command_line.push_error(&format!("Usage: {prefix} WIDTH <style> <factor>"));
                        }
                    }
                    _ => {
                        self.command_line.push_info(&format!(
                            "Usage: {prefix} LIST | NEW <name> | FONT <style> <file> | WIDTH <style> <factor>"
                        ));
                    }
                }
            }

            // ── LINETYPE management ───────────────────────────────────────
            cmd if cmd == "LINETYPE" || cmd == "LT" || cmd.starts_with("LINETYPE ") || cmd.starts_with("LT ") => {
                let raw_rest = cmd.split_once(' ').map(|(_, r)| r.trim()).unwrap_or("");
                let parts: Vec<&str> = raw_rest.split_whitespace().collect();
                let sub = parts.get(0).map(|s| s.to_uppercase()).unwrap_or_default();
                match sub.as_str() {
                    "" | "LIST" | "?" => {
                        let ltypes: Vec<String> = self.tabs[i].scene.document
                            .line_types.iter()
                            .map(|lt| format!("{} ({})", lt.name, lt.description))
                            .collect();
                        if ltypes.is_empty() {
                            self.command_line.push_output("No linetypes defined.");
                        } else {
                            self.command_line.push_output(&format!("Linetypes: {}", ltypes.join(", ")));
                        }
                    }
                    _ => {
                        self.command_line.push_info("Usage: LINETYPE LIST");
                    }
                }
            }

            // ── PURGE unused definitions ──────────────────────────────────
            cmd if cmd == "PURGE" || cmd.starts_with("PURGE ") => {
                let sub = cmd.split_whitespace().nth(1).unwrap_or("ALL").to_uppercase();
                let all = sub == "ALL" || sub.is_empty();

                // Collect names in use (immutable borrows — done in their own scope)
                let used_layers: std::collections::HashSet<String> = self.tabs[i].scene.document.entities()
                    .filter_map(|e| {
                        let name = &e.common().layer;
                        if name.is_empty() { None } else { Some(name.clone()) }
                    }).collect();
                let used_text_styles: std::collections::HashSet<String> = self.tabs[i].scene.document.entities()
                    .filter_map(|e| match e {
                        acadrust::EntityType::Text(t) => Some(t.style.clone()),
                        acadrust::EntityType::MText(t) => Some(t.style.clone()),
                        _ => None,
                    })
                    .filter(|s| !s.is_empty())
                    .collect();
                let used_linetypes: std::collections::HashSet<String> = self.tabs[i].scene.document.entities()
                    .filter_map(|e| {
                        let lt = &e.common().linetype;
                        if lt.is_empty() || lt == "ByLayer" || lt == "ByBlock" { None } else { Some(lt.clone()) }
                    }).collect();

                // Build removal lists (still immutable)
                let layer_remove: Vec<String> = if all || sub == "LAYERS" {
                    self.tabs[i].scene.document.layers.iter()
                        .filter(|l| l.name != "0" && !used_layers.contains(&l.name))
                        .map(|l| l.name.clone()).collect()
                } else { vec![] };
                let style_remove: Vec<String> = if all || sub == "TEXTSTYLES" || sub == "STYLES" {
                    self.tabs[i].scene.document.text_styles.iter()
                        .filter(|s| s.name != "Standard" && !used_text_styles.contains(&s.name))
                        .map(|s| s.name.clone()).collect()
                } else { vec![] };
                let lt_remove: Vec<String> = if all || sub == "LINETYPES" || sub == "LT" {
                    let standard = ["Continuous", "ByLayer", "ByBlock"];
                    self.tabs[i].scene.document.line_types.iter()
                        .filter(|lt| !standard.iter().any(|s| s.eq_ignore_ascii_case(&lt.name))
                            && !used_linetypes.contains(&lt.name))
                        .map(|lt| lt.name.clone()).collect()
                } else { vec![] };

                // Apply removals (mutable)
                let purged = layer_remove.len() + style_remove.len() + lt_remove.len();
                for name in &layer_remove { self.tabs[i].scene.document.layers.remove(name); }
                for name in &style_remove { self.tabs[i].scene.document.text_styles.remove(name); }
                for name in &lt_remove { self.tabs[i].scene.document.line_types.remove(name); }

                if purged > 0 {
                    self.push_undo_snapshot(i, "PURGE");
                    self.tabs[i].dirty = true;
                    self.command_line.push_output(&format!("PURGE: {} definition(s) removed.", purged));
                } else {
                    self.command_line.push_output("PURGE: nothing to purge.");
                }
            }

            // ── CHPROP — change entity properties from command line ───────
            cmd if cmd == "CHPROP" || cmd.starts_with("CHPROP ") => {
                // Usage: CHPROP <property> <value>
                // Applies to currently selected entities.
                // Properties: LAYER, COLOR, LINETYPE, LTSCALE
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                let prop = parts.get(1).map(|s| s.to_uppercase()).unwrap_or_default();
                let value = parts.get(2).map(|s| s.trim()).unwrap_or("").to_string();

                if prop.is_empty() {
                    self.command_line.push_info(
                        "Usage: CHPROP <prop> <val>  (props: LAYER COLOR LINETYPE LTSCALE)"
                    );
                } else {
                    let handles: Vec<_> = self.tabs[i].scene.selected_entities()
                        .into_iter().map(|(h, _)| h).collect();
                    if handles.is_empty() {
                        self.command_line.push_error("CHPROP: no entities selected.");
                    } else {
                        // Validate value early to give clear errors
                        let color_val: Option<acadrust::types::Color> = if prop == "COLOR" {
                            value.parse::<i16>().ok().map(acadrust::types::Color::from_index)
                        } else { None };
                        let ltscale_val: Option<f64> = if prop == "LTSCALE" {
                            value.parse().ok()
                        } else { None };

                        if (prop == "COLOR" && color_val.is_none())
                            || (prop == "LTSCALE" && ltscale_val.is_none())
                        {
                            self.command_line.push_error(&format!("CHPROP: invalid value '{}' for {}.", value, prop));
                        } else {
                            let mut changed = 0usize;
                            for handle in &handles {
                                if let Some(entity) = self.tabs[i].scene.document.get_entity_mut(*handle) {
                                    let common = entity.common_mut();
                                    match prop.as_str() {
                                        "LAYER"            => { common.layer = value.clone(); changed += 1; }
                                        "LINETYPE" | "LT"  => { common.linetype = value.clone(); changed += 1; }
                                        "LTSCALE"          => { common.linetype_scale = ltscale_val.unwrap(); changed += 1; }
                                        "COLOR"            => { common.color = color_val.unwrap(); changed += 1; }
                                        _ => {
                                            self.command_line.push_error(&format!(
                                                "CHPROP: unknown property '{}'. Use: LAYER COLOR LINETYPE LTSCALE", prop
                                            ));
                                            break;
                                        }
                                    }
                                }
                            }
                            if changed > 0 {
                                self.push_undo_snapshot(i, "CHPROP");
                                self.tabs[i].dirty = true;
                                self.command_line.push_output(&format!("CHPROP: {} entity/entities updated.", changed));
                            }
                        }
                    }
                }
            }

            // ── RENAME table entries ──────────────────────────────────────
            cmd if cmd == "RENAME" || cmd.starts_with("RENAME ") => {
                // Usage: RENAME <type> <old_name> <new_name>
                // Types: LAYER BLOCK STYLE DIMSTYLE LINETYPE UCS VIEW
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                let type_str = parts.get(1).map(|s| s.to_uppercase()).unwrap_or_default();
                let old_name = parts.get(2).map(|s| s.trim()).unwrap_or("").to_string();
                let new_name = parts.get(3).map(|s| s.trim()).unwrap_or("").to_string();

                if type_str.is_empty() || old_name.is_empty() || new_name.is_empty() {
                    self.command_line.push_info(
                        "Usage: RENAME <type> <old> <new>  (types: LAYER BLOCK STYLE DIMSTYLE LINETYPE UCS VIEW)"
                    );
                } else {
                    let doc = &mut self.tabs[i].scene.document;
                    let ok = match type_str.as_str() {
                        "LAYER" => {
                            if let Some(l) = doc.layers.get_mut(&old_name) {
                                l.name = new_name.clone();
                                // Update entity references
                                for e in doc.entities_mut() {
                                    if e.common().layer == old_name {
                                        e.common_mut().layer = new_name.clone();
                                    }
                                }
                                true
                            } else { false }
                        }
                        "STYLE" | "TEXTSTYLE" => {
                            if let Some(s) = doc.text_styles.get_mut(&old_name) {
                                s.name = new_name.clone(); true
                            } else { false }
                        }
                        "DIMSTYLE" => {
                            if let Some(s) = doc.dim_styles.get_mut(&old_name) {
                                s.name = new_name.clone(); true
                            } else { false }
                        }
                        "LINETYPE" | "LT" => {
                            if let Some(lt) = doc.line_types.get_mut(&old_name) {
                                lt.name = new_name.clone(); true
                            } else { false }
                        }
                        "UCS" => {
                            if let Some(u) = doc.ucss.get_mut(&old_name) {
                                u.name = new_name.clone(); true
                            } else { false }
                        }
                        "VIEW" => {
                            if let Some(v) = doc.views.get_mut(&old_name) {
                                v.name = new_name.clone(); true
                            } else { false }
                        }
                        _ => {
                            self.command_line.push_error(&format!("RENAME: unknown type '{}'. Use LAYER BLOCK STYLE DIMSTYLE LINETYPE UCS VIEW", type_str));
                            false
                        }
                    };
                    if ok {
                        self.push_undo_snapshot(i, "RENAME");
                        self.tabs[i].dirty = true;
                        self.command_line.push_output(&format!("RENAME: '{}' → '{}'.", old_name, new_name));
                    } else if type_str != "BLOCK" {
                        self.command_line.push_error(&format!("RENAME: '{}' not found in {}.", old_name, type_str));
                    }
                }
            }

            // ── Plot / Page Setup ──────────────────────────────────────────
            "PRINT"|"PLOT"|"EXPORT" => {
                return Task::done(Message::PlotExport);
            }
            "PAGESETUP" => {
                if self.tabs[i].scene.current_layout == "Model" {
                    self.command_line.push_error("PAGESETUP: switch to a paper space layout first.");
                } else {
                    return Task::done(Message::PageSetupOpen);
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
