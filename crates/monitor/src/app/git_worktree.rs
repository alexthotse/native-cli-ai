//! Git banner, `.nca/worktrees` listing, diff review, merge/remove worktree.

use super::{palette, session_io, widgets, DesktopApp};
use eframe::egui;
use nca_runtime::worktree::WorktreeManager;
use std::path::PathBuf;

impl DesktopApp {
    pub(crate) fn show_git_worktree_view(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(palette::BG)
                    .inner_margin(egui::Margin::symmetric(24.0, 16.0)),
            )
            .show(ctx, |ui| {
                let Some(ws) = self.selected_workspace() else {
                    ui.colored_label(
                        palette::TEXT_DIM,
                        "Select a project folder or open a workspace first.",
                    );
                    return;
                };

                let wt_mgr = WorktreeManager::new(&ws);
                if !wt_mgr.is_git_repo() {
                    egui::Frame::none()
                        .fill(palette::WARNING.linear_multiply(0.12))
                        .rounding(8.0)
                        .inner_margin(egui::Margin::same(14.0))
                        .stroke(egui::Stroke::new(1.0, palette::WARNING))
                        .show(ui, |ui| {
                            ui.colored_label(
                                palette::WARNING,
                                "This folder is not a git repository. Sub-agents will not get isolated worktrees.",
                            );
                        });
                    return;
                }

                let branch = wt_mgr.current_branch().unwrap_or_else(|_| "?".into());
                ui.horizontal(|ui| {
                    ui.colored_label(
                        palette::WHITE,
                        egui::RichText::new("Git & worktrees").size(17.0).strong(),
                    );
                    ui.add_space(16.0);
                    ui.colored_label(
                        palette::TEXT_DIM,
                        format!("{}", ws.display()),
                    );
                    ui.add_space(12.0);
                    ui.colored_label(palette::TEXT_DIM, format!("HEAD: {branch}"));
                });
                ui.add_space(6.0);
                ui.colored_label(
                    palette::TEXT_DIM,
                    egui::RichText::new(
                        "Worktrees live under .nca/worktrees/<session_id>. Merge and remove are destructive — confirm carefully.",
                    )
                    .size(11.0),
                );
                ui.add_space(16.0);

                let config = self.effective_project_config();
                let list = wt_mgr.list_worktrees();
                if list.is_empty() {
                    ui.colored_label(
                        palette::TEXT_DIM,
                        "No NCA worktrees yet. Spawn a sub-agent (spawn_subagent) to create one.",
                    );
                    return;
                }

                ui.columns(2, |cols| {
                    cols[0].set_min_width(280.0);
                    egui::ScrollArea::vertical()
                        .max_height(420.0)
                        .show(&mut cols[0], |ui| {
                            ui.colored_label(
                                palette::WHITE,
                                egui::RichText::new("Worktrees").size(13.0).strong(),
                            );
                            ui.add_space(8.0);
                            for info in &list {
                                let meta = self
                                    .project_sessions
                                    .iter()
                                    .find(|m| m.id == info.session_id)
                                    .cloned()
                                    .or_else(|| {
                                        session_io::load_session_state(&ws, &config, &info.session_id)
                                            .map(|s| s.meta)
                                    });
                                let selected = self.git_selected_session_id.as_deref() == Some(info.session_id.as_str());
                                let subtitle = meta
                                    .as_ref()
                                    .map(|m| format!("{:?}", m.status))
                                    .unwrap_or_else(|| "unknown".into());
                                if widgets::draw_entity_tile(ui, selected, &info.session_id, &subtitle)
                                    .clicked()
                                {
                                    self.git_selected_session_id = Some(info.session_id.clone());
                                    self.git_selected_file = None;
                                }
                                if let Some(m) = &meta {
                                    ui.horizontal(|ui| {
                                        ui.add_space(16.0);
                                        if ui.small_button("Open session").clicked() {
                                            self.resume_or_attach_session(m.clone());
                                        }
                                    });
                                    ui.add_space(4.0);
                                }
                            }
                        });

                    cols[1].vertical(|ui| {
                        ui.colored_label(
                            palette::WHITE,
                            egui::RichText::new("Review").size(13.0).strong(),
                        );
                        ui.add_space(8.0);
                        let Some(sel) = self.git_selected_session_id.clone() else {
                            ui.colored_label(palette::TEXT_DIM, "Select a worktree.");
                            return;
                        };
                        let wt_path: Option<PathBuf> = self
                            .project_sessions
                            .iter()
                            .find(|m| m.id == sel)
                            .and_then(|m| m.worktree_path.clone())
                            .or_else(|| {
                                list.iter()
                                    .find(|i| i.session_id == sel)
                                    .map(|i| i.worktree_path.clone())
                            });
                        let Some(wt_path) = wt_path else {
                            ui.colored_label(palette::TEXT_DIM, "No worktree path in session metadata.");
                            return;
                        };
                        let base = self
                            .project_sessions
                            .iter()
                            .find(|m| m.id == sel)
                            .and_then(|m| m.base_branch.clone())
                            .or_else(|| {
                                session_io::load_session_state(&ws, &config, &sel)
                                    .and_then(|s| s.meta.base_branch)
                            })
                            .unwrap_or_else(|| branch.clone());

                        let files = wt_mgr.changed_files(&wt_path, &base);
                        if files.is_empty() {
                            ui.colored_label(palette::TEXT_DIM, "No diff vs base (clean or same tree).");
                        } else {
                            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                                for cf in &files {
                                    let label = format!("{} {}", cf.change_type, cf.path.display());
                                    if ui.selectable_label(
                                        self.git_selected_file.as_ref() == Some(&cf.path),
                                        label,
                                    )
                                    .clicked()
                                    {
                                        self.git_selected_file = Some(cf.path.clone());
                                        self.git_diff_buffer =
                                            wt_mgr.file_diff(&wt_path, &base, &cf.path);
                                    }
                                }
                            });
                        }
                        ui.add_space(10.0);
                        if self.git_selected_file.is_some() {
                            if let Some(fp) = &self.git_selected_file {
                                ui.label(
                                    egui::RichText::new(fp.display().to_string())
                                        .size(11.0)
                                        .strong(),
                                );
                            }
                            ui.add_space(4.0);
                            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.git_diff_buffer)
                                        .desired_width(f32::INFINITY)
                                        .font(egui::TextStyle::Monospace),
                                );
                            });
                        }

                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    true,
                                    egui::Button::new(
                                        egui::RichText::new("Merge into base…")
                                            .color(palette::WHITE),
                                    )
                                    .fill(palette::ACCENT),
                                )
                                .on_hover_text(
                                    "Checks out the base branch in the main repo and merges nca/<session>.",
                                )
                                .clicked()
                            {
                                self.git_pending_merge = Some((sel.clone(), base.clone()));
                            }
                            if ui
                                .button("Remove worktree…")
                                .on_hover_text("git worktree remove (and optionally delete branch)")
                                .clicked()
                            {
                                self.git_pending_remove = Some((sel.clone(), base.clone(), true));
                            }
                        });
                    });
                });
            });
    }

    pub(crate) fn show_git_confirmations(&mut self, ctx: &egui::Context) {
        if let Some((sid, base)) = self.git_pending_merge.clone() {
            let mut open = true;
            egui::Window::new("Merge branch into base")
                .collapsible(false)
                .resizable(true)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.colored_label(
                        palette::WARNING,
                        "This modifies your main repository: it checks out the base branch and merges the NCA session branch.",
                    );
                    ui.add_space(8.0);
                    ui.label(format!("Session: {sid}"));
                    ui.label(format!("Base: {base}"));
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.git_pending_merge = None;
                        }
                        if ui.button("Merge").clicked() {
                            let ws = match self.selected_workspace() {
                                Some(w) => w,
                                None => {
                                    self.git_pending_merge = None;
                                    return;
                                }
                            };
                            let wt_mgr = WorktreeManager::new(&ws);
                            match wt_mgr.merge_into_base(&sid, &base) {
                                Ok(()) => {
                                    self.set_status("Merge completed.", false);
                                }
                                Err(e) => {
                                    self.set_status(e.to_string(), true);
                                }
                            }
                            self.git_pending_merge = None;
                            self.reload_selected_workspace_data();
                        }
                    });
                });
            if !open {
                self.git_pending_merge = None;
            }
        }

        if let Some((sid, _base, delete_branch)) = self.git_pending_remove.clone() {
            let mut open = true;
            egui::Window::new("Remove NCA worktree")
                .collapsible(false)
                .resizable(true)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.colored_label(
                        palette::ERROR,
                        "Removes the worktree directory. Optional: delete branch nca/<session>.",
                    );
                    ui.add_space(8.0);
                    ui.label(format!("Session: {sid}"));
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.git_pending_remove = None;
                        }
                        if ui.button("Remove worktree").clicked() {
                            let ws = match self.selected_workspace() {
                                Some(w) => w,
                                None => {
                                    self.git_pending_remove = None;
                                    return;
                                }
                            };
                            let wt_mgr = WorktreeManager::new(&ws);
                            let _ = wt_mgr.remove_worktree(&sid, delete_branch);
                            self.git_pending_remove = None;
                            self.git_selected_session_id = None;
                            self.git_selected_file = None;
                            self.reload_selected_workspace_data();
                            self.set_status("Worktree removed.", false);
                        }
                    });
                });
            if !open {
                self.git_pending_remove = None;
            }
        }
    }
}
