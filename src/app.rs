//! Glassroom UI — Classic-style Library / Develop chrome (original branding).

use crate::catalog::{default_catalog_path, Catalog, Collection, DevelopParams, Flag, Photo};
use crate::develop::{self, apply_develop, for_preview, load_image, load_thumb_color_image, to_color_image};
use eframe::egui;
use std::collections::HashMap;
use std::path::PathBuf;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Module {
    Library,
    Develop,
}

#[derive(Clone, PartialEq, Eq)]
enum SidebarSel {
    All,
    Folder(String),
    Collection(i64),
}

pub struct GlassroomApp {
    catalog: Catalog,
    module: Module,
    sidebar: SidebarSel,
    photos: Vec<Photo>,
    folders: Vec<(String, i64)>,
    collections: Vec<Collection>,
    selected: Option<i64>,
    status: String,
    // Thumbnail texture cache: photo_id → texture
    thumbs: HashMap<i64, egui::TextureHandle>,
    // Develop preview
    preview_tex: Option<egui::TextureHandle>,
    preview_for_id: Option<i64>,
    preview_params: DevelopParams,
    dirty_develop: bool,
    new_collection_name: String,
    grid_thumb_size: f32,
}

fn apply_dark_theme(ctx: &egui::Context) {
    // Force Classic-like dark chrome every frame (macOS light system theme can otherwise wash out panels).
    let mut visuals = egui::Visuals::dark();
    visuals.dark_mode = true;
    visuals.override_text_color = Some(egui::Color32::from_rgb(220, 220, 220));
    visuals.panel_fill = egui::Color32::from_rgb(32, 32, 32);
    visuals.window_fill = egui::Color32::from_rgb(38, 38, 38);
    visuals.extreme_bg_color = egui::Color32::from_rgb(22, 22, 22);
    visuals.faint_bg_color = egui::Color32::from_rgb(40, 40, 40);
    visuals.code_bg_color = egui::Color32::from_rgb(28, 28, 28);
    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(45, 45, 45);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(52, 52, 52);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(68, 68, 68);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(78, 78, 78);
    visuals.widgets.open.bg_fill = egui::Color32::from_rgb(60, 60, 60);
    visuals.selection.bg_fill = egui::Color32::from_rgb(70, 110, 160);
    visuals.hyperlink_color = egui::Color32::from_rgb(120, 170, 220);
    ctx.set_visuals(visuals);
}

fn dark_panel_frame() -> egui::Frame {
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(32, 32, 32))
        .inner_margin(egui::Margin::same(8.0))
}

impl GlassroomApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_dark_theme(&cc.egui_ctx);

        let path = default_catalog_path();
        let catalog = Catalog::open(&path).expect("open catalog");
        let mut app = Self {
            catalog,
            module: Module::Library,
            sidebar: SidebarSel::All,
            photos: Vec::new(),
            folders: Vec::new(),
            collections: Vec::new(),
            selected: None,
            status: format!("Catalog: {}", path.display()),
            thumbs: HashMap::new(),
            preview_tex: None,
            preview_for_id: None,
            preview_params: DevelopParams::default(),
            dirty_develop: false,
            new_collection_name: String::new(),
            grid_thumb_size: 140.0,
        };
        app.reload_lists();
        app
    }

    fn reload_lists(&mut self) {
        self.folders = self.catalog.list_folders().unwrap_or_default();
        self.collections = self.catalog.list_collections().unwrap_or_default();
        self.reload_photos();
    }

    fn reload_photos(&mut self) {
        let result = match &self.sidebar {
            SidebarSel::All => self.catalog.list_photos(),
            SidebarSel::Folder(f) => self.catalog.photos_in_folder(f),
            SidebarSel::Collection(id) => self.catalog.photos_in_collection(*id),
        };
        self.photos = result.unwrap_or_default();
        if let Some(id) = self.selected {
            if !self.photos.iter().any(|p| p.id == id) {
                self.selected = self.photos.first().map(|p| p.id);
            }
        } else {
            self.selected = self.photos.first().map(|p| p.id);
        }
        if let Some(id) = self.selected {
            if let Ok(Some(p)) = self.catalog.get_photo(id) {
                self.preview_params = p.develop.clone();
                self.preview_for_id = None; // force refresh
                self.dirty_develop = false;
            }
        }
    }

    fn selected_photo(&self) -> Option<&Photo> {
        let id = self.selected?;
        self.photos.iter().find(|p| p.id == id)
    }

    fn import_folder(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
            match self.catalog.import_folder(&dir) {
                Ok((n, _)) => {
                    self.status = format!("Imported / linked {} JPEG(s) from {}", n, dir.display());
                    self.reload_lists();
                }
                Err(e) => self.status = format!("Import failed: {e}"),
            }
        }
    }

    fn import_files(&mut self) {
        let files = rfd::FileDialog::new()
            .add_filter("JPEG images", &["jpg", "jpeg", "JPG", "JPEG"])
            .pick_files();
        let Some(files) = files else {
            return;
        };
        if files.is_empty() {
            return;
        }
        let mut ok = 0usize;
        let mut skipped = 0usize;
        for path in &files {
            match self.catalog.import_jpeg(path) {
                Ok(Some(_)) => ok += 1,
                Ok(None) => skipped += 1,
                Err(e) => {
                    self.status = format!("Import failed on {}: {e}", path.display());
                    self.reload_lists();
                    return;
                }
            }
        }
        self.status = if skipped == 0 {
            format!("Imported / linked {ok} JPEG(s)")
        } else {
            format!("Imported / linked {ok} JPEG(s); skipped {skipped} non-JPEG")
        };
        self.reload_lists();
    }

    fn export_selected(&mut self) {
        let Some(photo) = self.selected_photo().cloned() else {
            self.status = "Nothing selected to export".into();
            return;
        };
        // Persist current develop first
        let _ = self.catalog.set_develop(photo.id, &self.preview_params);
        let default_name = format!(
            "{}_glassroom.jpg",
            photo
                .path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "export".into())
        );
        if let Some(dest) = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .add_filter("JPEG", &["jpg", "jpeg"])
            .save_file()
        {
            match develop::export_jpeg(&photo.path, &dest, &self.preview_params, 92) {
                Ok(()) => self.status = format!("Exported {}", dest.display()),
                Err(e) => self.status = format!("Export failed: {e}"),
            }
        }
    }

    fn ensure_thumb(&mut self, ctx: &egui::Context, photo: &Photo) {
        if self.thumbs.contains_key(&photo.id) {
            return;
        }
        if let Ok(color) = load_thumb_color_image(&photo.path, 256) {
            let tex = ctx.load_texture(
                format!("thumb-{}", photo.id),
                color,
                egui::TextureOptions::LINEAR,
            );
            self.thumbs.insert(photo.id, tex);
        }
    }

    fn ensure_preview(&mut self, ctx: &egui::Context) {
        let Some(id) = self.selected else {
            self.preview_tex = None;
            return;
        };
        let needs = self.preview_for_id != Some(id) || self.dirty_develop || self.preview_tex.is_none();
        if !needs {
            return;
        }
        let path = match self.catalog.get_photo(id) {
            Ok(Some(p)) => p.path,
            _ => return,
        };
        match load_image(&path) {
            Ok(img) => {
                let preview = for_preview(&img, 1280);
                let developed = apply_develop(&preview, &self.preview_params);
                let color = to_color_image(&developed);
                let tex = ctx.load_texture("develop-preview", color, egui::TextureOptions::LINEAR);
                self.preview_tex = Some(tex);
                self.preview_for_id = Some(id);
                self.dirty_develop = false;
            }
            Err(e) => {
                self.status = format!("Preview error: {e}");
            }
        }
    }

    fn persist_develop_if_needed(&mut self) {
        if let Some(id) = self.selected {
            let _ = self.catalog.set_develop(id, &self.preview_params);
            if let Some(p) = self.photos.iter_mut().find(|p| p.id == id) {
                p.develop = self.preview_params.clone();
            }
        }
    }

    fn set_rating(&mut self, rating: i32) {
        if let Some(id) = self.selected {
            let _ = self.catalog.set_rating(id, rating);
            if let Some(p) = self.photos.iter_mut().find(|p| p.id == id) {
                p.rating = rating.clamp(0, 5);
            }
        }
    }

    fn set_flag(&mut self, flag: Flag) {
        if let Some(id) = self.selected {
            let _ = self.catalog.set_flag(id, flag);
            if let Some(p) = self.photos.iter_mut().find(|p| p.id == id) {
                p.flag = flag;
            }
        }
    }

    fn top_chrome(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let brand = egui::RichText::new("Glassroom")
                .strong()
                .size(16.0)
                .color(egui::Color32::from_rgb(220, 220, 220));
            ui.label(brand);
            ui.add_space(16.0);

            let lib = ui.selectable_label(self.module == Module::Library, "Library");
            if lib.clicked() {
                self.persist_develop_if_needed();
                self.module = Module::Library;
            }
            let dev = ui.selectable_label(self.module == Module::Develop, "Develop");
            if dev.clicked() {
                self.module = Module::Develop;
                self.dirty_develop = true;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Export…").clicked() {
                    self.export_selected();
                }
                if ui.button("Import Folder…").clicked() {
                    self.import_folder();
                }
                if ui.button("Import Photos…").clicked() {
                    self.import_files();
                }
            });
        });
    }

    fn sidebar_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Catalog");
        ui.separator();

        let all_sel = self.sidebar == SidebarSel::All;
        if ui.selectable_label(all_sel, format!("All Photographs")).clicked() {
            self.sidebar = SidebarSel::All;
            self.reload_photos();
        }

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Folders").weak().small());
        egui::ScrollArea::vertical()
            .id_salt("folders")
            .max_height(200.0)
            .show(ui, |ui| {
                for (folder, count) in self.folders.clone() {
                    let label = PathBuf::from(&folder)
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| folder.clone());
                    let sel = matches!(&self.sidebar, SidebarSel::Folder(f) if f == &folder);
                    if ui
                        .selectable_label(sel, format!("{label}  ({count})"))
                        .on_hover_text(&folder)
                        .clicked()
                    {
                        self.sidebar = SidebarSel::Folder(folder);
                        self.reload_photos();
                    }
                }
            });

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Collections").weak().small());
        for c in self.collections.clone() {
            let sel = matches!(&self.sidebar, SidebarSel::Collection(id) if *id == c.id);
            if ui.selectable_label(sel, &c.name).clicked() {
                self.sidebar = SidebarSel::Collection(c.id);
                self.reload_photos();
            }
        }
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.new_collection_name)
                    .desired_width(100.0)
                    .hint_text("New collection"),
            );
            if ui.button("+").clicked() && !self.new_collection_name.trim().is_empty() {
                if let Ok(id) = self.catalog.create_collection(self.new_collection_name.trim()) {
                    self.new_collection_name.clear();
                    self.reload_lists();
                    self.sidebar = SidebarSel::Collection(id);
                    self.reload_photos();
                }
            }
        });

        if let Some(photo) = self.selected_photo() {
            let pid = photo.id;
            if let SidebarSel::Collection(cid) = self.sidebar {
                if ui.button("Add selected to this collection").clicked() {
                    let _ = self.catalog.add_to_collection(cid, pid);
                    self.status = "Added to collection".into();
                    self.reload_photos();
                }
            } else if !self.collections.is_empty() {
                let first = self.collections[0].id;
                if ui.button("Add selected to Selects").clicked() {
                    let _ = self.catalog.add_to_collection(first, pid);
                    self.status = "Added to Selects".into();
                }
            }
        }
    }

    fn library_grid(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.label(format!("{} photos", self.photos.len()));
            ui.add(egui::Slider::new(&mut self.grid_thumb_size, 80.0..=220.0).text("Size"));
        });
        ui.separator();

        if self.photos.is_empty() {
            ui.add_space(48.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("No photos in the catalog yet")
                        .size(18.0)
                        .color(egui::Color32::from_rgb(200, 200, 200)),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Import individual JPEGs or a whole folder to begin.")
                        .color(egui::Color32::from_rgb(160, 160, 160)),
                );
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if ui.button("Import Photos…").clicked() {
                        self.import_files();
                    }
                    if ui.button("Import Folder…").clicked() {
                        self.import_folder();
                    }
                });
            });
            return;
        }

        let thumb = self.grid_thumb_size;
        let photos = self.photos.clone();
        egui::ScrollArea::vertical().id_salt("grid").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for photo in &photos {
                    self.ensure_thumb(ctx, photo);
                    let selected = self.selected == Some(photo.id);
                    let frame = egui::Frame::none()
                        .fill(if selected {
                            egui::Color32::from_rgb(70, 110, 160)
                        } else {
                            egui::Color32::from_rgb(45, 45, 45)
                        })
                        .inner_margin(4.0)
                        .rounding(2.0);

                    let response = frame.show(ui, |ui| {
                        ui.set_width(thumb);
                        if let Some(tex) = self.thumbs.get(&photo.id) {
                            let size = egui::vec2(thumb - 8.0, thumb - 8.0);
                            ui.add(egui::Image::from_texture(tex).fit_to_exact_size(size));
                        } else {
                            ui.allocate_exact_size(
                                egui::vec2(thumb - 8.0, thumb - 8.0),
                                egui::Sense::hover(),
                            );
                            ui.label("…");
                        }
                        let flag = match photo.flag {
                            Flag::Pick => "▲ ",
                            Flag::Reject => "▼ ",
                            Flag::None => "",
                        };
                        let stars = "★".repeat(photo.rating as usize)
                            + &"☆".repeat((5 - photo.rating).max(0) as usize);
                        ui.label(
                            egui::RichText::new(format!("{flag}{stars}"))
                                .small()
                                .color(egui::Color32::from_rgb(200, 180, 80)),
                        );
                        ui.label(egui::RichText::new(&photo.filename).small());
                    });
                    if response.response.clicked() {
                        self.selected = Some(photo.id);
                        self.preview_params = photo.develop.clone();
                        self.preview_for_id = None;
                        self.dirty_develop = true;
                    }
                    if response.response.double_clicked() {
                        self.selected = Some(photo.id);
                        self.preview_params = photo.develop.clone();
                        self.module = Module::Develop;
                        self.dirty_develop = true;
                    }
                }
            });
        });
    }

    fn filmstrip(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let photos = self.photos.clone();
        egui::ScrollArea::horizontal().id_salt("filmstrip").show(ui, |ui| {
            ui.horizontal(|ui| {
                for photo in &photos {
                    self.ensure_thumb(ctx, photo);
                    let selected = self.selected == Some(photo.id);
                    let size = egui::vec2(72.0, 72.0);
                    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
                    if selected {
                        ui.painter().rect_filled(
                            rect.expand(2.0),
                            2.0,
                            egui::Color32::from_rgb(70, 110, 160),
                        );
                    }
                    if let Some(tex) = self.thumbs.get(&photo.id) {
                        egui::Image::from_texture(tex)
                            .fit_to_exact_size(size - egui::vec2(4.0, 4.0))
                            .paint_at(ui, rect.shrink(2.0));
                    }
                    if resp.clicked() {
                        self.selected = Some(photo.id);
                        self.preview_params = photo.develop.clone();
                        self.preview_for_id = None;
                        self.dirty_develop = true;
                    }
                }
            });
        });
    }

    fn develop_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Basic");
        let mut changed = false;
        changed |= ui
            .add(egui::Slider::new(&mut self.preview_params.exposure, -2.0..=2.0).text("Exposure"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut self.preview_params.contrast, -100.0..=100.0).text("Contrast"))
            .changed();
        changed |= ui
            .add(
                egui::Slider::new(&mut self.preview_params.highlights, -100.0..=100.0)
                    .text("Highlights"),
            )
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut self.preview_params.shadows, -100.0..=100.0).text("Shadows"))
            .changed();

        ui.add_space(8.0);
        ui.heading("White Balance");
        changed |= ui
            .add(egui::Slider::new(&mut self.preview_params.temp, -100.0..=100.0).text("Temp"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut self.preview_params.tint, -100.0..=100.0).text("Tint"))
            .changed();

        ui.add_space(8.0);
        ui.heading("Presence");
        changed |= ui
            .add(
                egui::Slider::new(&mut self.preview_params.saturation, -100.0..=100.0)
                    .text("Saturation"),
            )
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut self.preview_params.clarity, -100.0..=100.0).text("Clarity"))
            .changed();

        ui.add_space(12.0);
        if ui.button("Reset").clicked() {
            self.preview_params = DevelopParams::default();
            changed = true;
        }
        if ui.button("Save settings").clicked() {
            self.persist_develop_if_needed();
            self.status = "Develop settings saved to catalog".into();
        }

        if changed {
            self.dirty_develop = true;
            self.persist_develop_if_needed();
        }
    }

    fn loupe_or_preview(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.ensure_preview(ctx);
        if let Some(tex) = &self.preview_tex {
            let avail = ui.available_size();
            ui.add(egui::Image::from_texture(tex).max_size(avail));
        } else if self.selected.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label("Import a folder of JPEGs to begin.");
            });
        } else {
            ui.spinner();
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Num0) {
                self.set_rating(0);
            }
            if i.key_pressed(egui::Key::Num1) {
                self.set_rating(1);
            }
            if i.key_pressed(egui::Key::Num2) {
                self.set_rating(2);
            }
            if i.key_pressed(egui::Key::Num3) {
                self.set_rating(3);
            }
            if i.key_pressed(egui::Key::Num4) {
                self.set_rating(4);
            }
            if i.key_pressed(egui::Key::Num5) {
                self.set_rating(5);
            }
            if i.key_pressed(egui::Key::P) {
                self.set_flag(Flag::Pick);
            }
            if i.key_pressed(egui::Key::X) {
                self.set_flag(Flag::Reject);
            }
            if i.key_pressed(egui::Key::U) {
                self.set_flag(Flag::None);
            }
            if i.key_pressed(egui::Key::ArrowLeft) {
                self.nudge_selection(-1);
            }
            if i.key_pressed(egui::Key::ArrowRight) {
                self.nudge_selection(1);
            }
            if i.key_pressed(egui::Key::D) && i.modifiers.command {
                self.module = Module::Develop;
                self.dirty_develop = true;
            }
            if i.key_pressed(egui::Key::G) && i.modifiers.command {
                self.persist_develop_if_needed();
                self.module = Module::Library;
            }
        });
    }

    fn nudge_selection(&mut self, delta: i32) {
        if self.photos.is_empty() {
            return;
        }
        let idx = self
            .selected
            .and_then(|id| self.photos.iter().position(|p| p.id == id))
            .unwrap_or(0) as i32;
        let n = self.photos.len() as i32;
        let next = (idx + delta).rem_euclid(n) as usize;
        let photo = &self.photos[next];
        self.selected = Some(photo.id);
        self.preview_params = photo.develop.clone();
        self.preview_for_id = None;
        self.dirty_develop = true;
    }
}

impl eframe::App for GlassroomApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_dark_theme(ctx);
        self.handle_keys(ctx);

        egui::TopBottomPanel::top("modules")
            .frame(dark_panel_frame())
            .show(ctx, |ui| {
                self.top_chrome(ui);
            });

        egui::TopBottomPanel::bottom("status")
            .exact_height(22.0)
            .frame(dark_panel_frame().inner_margin(egui::Margin::symmetric(8.0, 2.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&self.status).small());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("Apache-2.0 · free · local catalog")
                                .small()
                                .weak(),
                        );
                    });
                });
            });

        egui::TopBottomPanel::bottom("filmstrip")
            .exact_height(96.0)
            .frame(dark_panel_frame())
            .show(ctx, |ui| {
                self.filmstrip(ui, ctx);
            });

        egui::SidePanel::left("sidebar")
            .default_width(200.0)
            .resizable(true)
            .frame(dark_panel_frame())
            .show(ctx, |ui| {
                self.sidebar_panel(ui);
            });

        match self.module {
            Module::Library => {
                egui::CentralPanel::default()
                    .frame(dark_panel_frame())
                    .show(ctx, |ui| {
                        self.library_grid(ui, ctx);
                    });
            }
            Module::Develop => {
                egui::SidePanel::right("develop")
                    .default_width(260.0)
                    .resizable(true)
                    .frame(dark_panel_frame())
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            self.develop_panel(ui);
                        });
                    });
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(22, 22, 22))
                            .inner_margin(egui::Margin::same(8.0)),
                    )
                    .show(ctx, |ui| {
                        self.loupe_or_preview(ui, ctx);
                    });
            }
        }
    }
}
