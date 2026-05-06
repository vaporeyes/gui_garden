// ABOUTME: 2D top-down viewer for parsed Doom WAD maps. Drop a
// ABOUTME: doom1.wad in via the toolbar, pick a level, pan/zoom around.
//
// Rendering: each LINEDEF is a line between its two VERTEXES. 1-sided
// linedefs (solid walls) are drawn thick + bright, 2-sided (portals/
// step changes between sectors) thin + dim. THINGS are colored dots
// keyed by category — player starts green, monsters red, etc.
//
// Pan/zoom is delegated to `egui::Scene` so the viewer can fit the
// whole map initially and let the user drag/scroll to explore.
// Setting `scene_rect = Rect::ZERO` triggers Scene's auto-fit; we use
// that on map switch and on the "Fit to map" button.

use egui::{Color32, Pos2, Rangef, Rect, Scene, Stroke, Ui};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::wad::{Linedef, Map, ThingCategory, Wad};

pub struct WadViewer {
    state: ViewerState,
    /// Scene's view bounds in map coordinates. `Rect::ZERO` triggers
    /// auto-fit-to-content on the next frame, which is how the
    /// "Fit to map" button and post-load auto-fit work.
    scene_rect: Rect,
    /// Persisted across map switches so the user's view doesn't reset
    /// when they pick a new level.
    show_things: bool,
    show_decorations: bool,
}

enum ViewerState {
    Empty {
        error: Option<String>,
    },
    Loaded {
        path: PathBuf,
        wad: Arc<Wad>,
        map_names: Vec<String>,
        selected: String,
        map: Map,
    },
}

impl Default for WadViewer {
    fn default() -> Self {
        Self {
            state: ViewerState::Empty { error: None },
            scene_rect: Rect::ZERO,
            show_things: true,
            show_decorations: false,
        }
    }
}

impl WadViewer {
    /// Reload the viewer from a known path. Used at app launch to
    /// rehydrate the last-loaded WAD without forcing the user back
    /// through the file picker.
    pub fn load_from_path<P: Into<PathBuf>>(&mut self, path: P) {
        let path = path.into();
        match Wad::from_path(&path) {
            Ok(wad) => {
                let map_names = wad.map_names();
                if map_names.is_empty() {
                    self.state = ViewerState::Empty {
                        error: Some(format!(
                            "{} contains no maps (E#M# / MAP## lumps)",
                            path.display()
                        )),
                    };
                    return;
                }
                let selected = map_names[0].clone();
                let map = match wad.load_map(&selected) {
                    Ok(m) => m,
                    Err(e) => {
                        self.state = ViewerState::Empty {
                            error: Some(format!("failed to read {}: {}", selected, e)),
                        };
                        return;
                    }
                };
                self.state = ViewerState::Loaded {
                    path,
                    wad: Arc::new(wad),
                    map_names,
                    selected,
                    map,
                };
                // Trip Scene's auto-fit on the next frame.
                self.scene_rect = Rect::ZERO;
            }
            Err(e) => {
                self.state = ViewerState::Empty {
                    error: Some(format!("failed to load {}: {}", path.display(), e)),
                };
            }
        }
    }

    /// Path of the currently-loaded WAD, if any. Used by the outer
    /// app to persist the last-used path across launches.
    pub fn loaded_path(&self) -> Option<&Path> {
        match &self.state {
            ViewerState::Loaded { path, .. } => Some(path.as_path()),
            ViewerState::Empty { .. } => None,
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui.button("📁 Load WAD…").clicked() {
                self.pick_and_load();
            }
            // Map dropdown — only meaningful once a WAD is loaded.
            let mut switch_to: Option<String> = None;
            let mut fit_clicked = false;
            if let ViewerState::Loaded {
                selected,
                map_names,
                map,
                ..
            } = &self.state
            {
                egui::ComboBox::from_label("Map")
                    .selected_text(selected.clone())
                    .show_ui(ui, |ui| {
                        for name in map_names {
                            if ui
                                .selectable_label(name == selected, name)
                                .clicked()
                            {
                                switch_to = Some(name.clone());
                            }
                        }
                    });
                ui.separator();
                ui.label(
                    egui::RichText::new(format!(
                        "{} verts · {} lines · {} things",
                        map.vertexes.len(),
                        map.linedefs.len(),
                        map.things.len()
                    ))
                    .small()
                    .weak(),
                );
                ui.separator();
                if ui.button("Fit to map").clicked() {
                    fit_clicked = true;
                }
            }
            if let Some(name) = switch_to {
                self.switch_map(&name);
            }
            if fit_clicked {
                self.scene_rect = Rect::ZERO;
            }
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.show_things, "Things");
            if self.show_things {
                ui.checkbox(&mut self.show_decorations, "Show decorations");
            }
        });
        ui.separator();

        match &self.state {
            ViewerState::Empty { error } => {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        if let Some(err) = error {
                            ui.colored_label(
                                Color32::from_rgb(220, 80, 80),
                                err,
                            );
                            ui.add_space(6.0);
                        }
                        ui.label(
                            egui::RichText::new(
                                "Click \"Load WAD…\" to open a Doom \
                                 .wad file (e.g. shareware doom1.wad).",
                            )
                            .weak(),
                        );
                    });
                });
            }
            ViewerState::Loaded { map, .. } => {
                let map = map.clone();
                let show_things = self.show_things;
                let show_decorations = self.show_decorations;
                // Carve out the rect Scene will paint into so we can
                // hit-test cursor-over-viewport for the plain-scroll
                // zoom override below. We don't actually allocate the
                // rect (Scene does that), we just predict its bounds.
                let viewport = ui.available_rect_before_wrap();
                self.handle_wheel_zoom(ui, viewport);
                Scene::new()
                    .zoom_range(Rangef::new(0.005, 5.0))
                    .show(ui, &mut self.scene_rect, |scene_ui| {
                        draw_map(scene_ui, &map, show_things, show_decorations);
                    });
            }
        }
    }

    /// Plain scroll-wheel = zoom (instead of Scene's default behavior
    /// where plain scroll pans and only Ctrl+scroll zooms). Scales
    /// `scene_rect` around the cursor's scene-space position so the
    /// point under the cursor stays put across the zoom step. We zero
    /// out the wheel delta after consuming it so Scene doesn't also
    /// pan from the same scroll event.
    fn handle_wheel_zoom(&mut self, ui: &Ui, viewport: Rect) {
        // Read the scroll + cursor position once, then mutate to consume.
        let (scroll, pointer) = ui.input(|i| {
            (i.smooth_scroll_delta.y, i.pointer.hover_pos())
        });
        if scroll.abs() < 0.5 {
            return;
        }
        let Some(cursor) = pointer else {
            return;
        };
        if !viewport.contains(cursor) {
            return;
        }
        // Scale factor: positive scroll (wheel up) zooms in. Tunable.
        let factor = (1.0 + scroll * 0.0015).clamp(0.1, 10.0);

        // We need scene_rect in *scene coords*. Map the cursor from
        // viewport-pixel coords to scene coords by linear interp on
        // the current scene_rect. If scene_rect is degenerate (e.g.
        // ZERO before first auto-fit), bail — the user's first
        // scroll lands right after the auto-fit produces a real rect.
        if self.scene_rect.size().min_elem() <= 0.0 {
            return;
        }
        let tx = (cursor.x - viewport.left()) / viewport.width();
        let ty = (cursor.y - viewport.top()) / viewport.height();
        let cursor_scene = Pos2::new(
            self.scene_rect.left() + tx * self.scene_rect.width(),
            self.scene_rect.top() + ty * self.scene_rect.height(),
        );

        // Scale scene_rect's size by 1/factor (zooming in *shrinks*
        // the visible scene rect) while keeping cursor_scene fixed.
        let new_w = (self.scene_rect.width() / factor).max(1.0);
        let new_h = (self.scene_rect.height() / factor).max(1.0);
        let new_min = Pos2::new(
            cursor_scene.x - tx * new_w,
            cursor_scene.y - ty * new_h,
        );
        self.scene_rect =
            Rect::from_min_size(new_min, egui::vec2(new_w, new_h));

        // Consume the scroll so Scene doesn't double-handle it.
        ui.ctx().input_mut(|i| {
            i.smooth_scroll_delta = egui::Vec2::ZERO;
        });
    }

    fn switch_map(&mut self, name: &str) {
        if let ViewerState::Loaded {
            wad,
            selected,
            map,
            ..
        } = &mut self.state
        {
            if name == selected {
                return;
            }
            match wad.load_map(name) {
                Ok(m) => {
                    *map = m;
                    *selected = name.to_string();
                    self.scene_rect = Rect::ZERO;
                }
                Err(e) => {
                    eprintln!("WAD viewer: failed to switch to {}: {}", name, e);
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn pick_and_load(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Doom WAD", &["wad", "WAD"])
            .set_title("Load a Doom WAD")
            .pick_file()
        else {
            return;
        };
        self.load_from_path(path);
    }

    #[cfg(target_arch = "wasm32")]
    fn pick_and_load(&mut self) {
        self.state = ViewerState::Empty {
            error: Some("File picker unavailable on web.".into()),
        };
    }
}

/// Render the map inside the Scene's transformed sub-ui. All coords
/// are *map-space* — Scene applies the visible transform on its own
/// layer. Doom's Y axis points up; egui's points down, so we flip Y.
fn draw_map(ui: &mut Ui, map: &Map, show_things: bool, show_decorations: bool) {
    // Force Scene's auto-fit to see the actual map extent. Without an
    // allocate_rect call inside the closure, `ui.min_rect()` stays at
    // its empty default and Scene's `Rect::ZERO` auto-fit path
    // produces a degenerate transform, leaving the camera centered on
    // (0, 0) with the geometry far off-screen. Painting alone doesn't
    // grow the layout rect.
    let bbox = scene_bbox(map);
    let _ = ui.allocate_rect(bbox, egui::Sense::hover());

    let painter = ui.painter();

    // Linedefs.
    for ld in &map.linedefs {
        let (Some(va), Some(vb)) = (
            map.vertexes.get(ld.v1 as usize),
            map.vertexes.get(ld.v2 as usize),
        ) else {
            continue;
        };
        let p1 = Pos2::new(va.x as f32, -va.y as f32);
        let p2 = Pos2::new(vb.x as f32, -vb.y as f32);
        let stroke = linedef_stroke(ld);
        painter.line_segment([p1, p2], stroke);
    }

    // Things on top of geometry.
    if show_things {
        for t in &map.things {
            let cat = ThingCategory::classify(t.doom_type);
            if matches!(cat, ThingCategory::Decoration) && !show_decorations {
                continue;
            }
            let p = Pos2::new(t.x as f32, -t.y as f32);
            let (color, radius) = thing_visual(cat);
            painter.circle_filled(p, radius, color);
            // Small facing tick on player starts so orientation reads.
            if matches!(cat, ThingCategory::PlayerStart) {
                let ang = (t.angle as f32).to_radians();
                let tip = Pos2::new(
                    p.x + ang.cos() * radius * 2.5,
                    // Doom angle is CCW in map space; map y is flipped
                    // so the screen-space tick has its sin negated.
                    p.y - ang.sin() * radius * 2.5,
                );
                painter.line_segment([p, tip], Stroke::new(1.0, color));
            }
        }
    }
}

/// Map's bounding box in *scene coordinates* (y-flipped). Includes a
/// small margin so walls at the very edge don't get clipped right
/// against the viewport border.
fn scene_bbox(map: &Map) -> Rect {
    let bb = map.bbox;
    // Empty bbox guard — return a small unit rect so Scene still gets
    // a non-degenerate area to fit to.
    if bb.min_x == bb.max_x || bb.min_y == bb.max_y {
        return Rect::from_min_size(Pos2::new(-1.0, -1.0), egui::vec2(2.0, 2.0));
    }
    let margin = 32.0;
    let min = Pos2::new(bb.min_x as f32 - margin, -(bb.max_y as f32) - margin);
    let max = Pos2::new(bb.max_x as f32 + margin, -(bb.min_y as f32) + margin);
    Rect::from_min_max(min, max)
}

fn linedef_stroke(ld: &Linedef) -> Stroke {
    if ld.two_sided() {
        // Step / portal between sectors — dim, thin.
        Stroke::new(0.6, Color32::from_rgb(100, 95, 90))
    } else {
        // Solid wall.
        Stroke::new(1.4, Color32::from_rgb(220, 215, 200))
    }
}

fn thing_visual(cat: ThingCategory) -> (Color32, f32) {
    match cat {
        ThingCategory::PlayerStart => (Color32::from_rgb(80, 220, 110), 8.0),
        ThingCategory::Monster => (Color32::from_rgb(220, 80, 80), 6.0),
        ThingCategory::Weapon => (Color32::from_rgb(220, 200, 80), 5.0),
        ThingCategory::Ammo => (Color32::from_rgb(180, 160, 60), 4.0),
        ThingCategory::HealthArmor => (Color32::from_rgb(80, 200, 220), 5.0),
        ThingCategory::Key => (Color32::from_rgb(220, 130, 220), 5.0),
        ThingCategory::Decoration => (Color32::from_rgb(120, 120, 120), 3.0),
        ThingCategory::Other => (Color32::from_rgb(160, 160, 160), 3.0),
    }
}
