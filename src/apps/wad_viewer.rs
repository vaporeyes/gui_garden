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

use crate::wad::{Linedef, Map, Sector, ThingCategory, Wad};

/// Which structural element the viewer is highlighting. Each mode
/// keeps a faint backdrop of the geometry so the user always has
/// spatial context, then emphasizes the chosen element on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// Default — full geometry + things, balanced.
    Map,
    /// Things prominent, geometry dimmed.
    Things,
    /// Linedefs prominent, color-coded by special vs. plain.
    Linedefs,
    /// Sector heatmap — linedef color comes from the sector's floor
    /// height (cool = low, warm = high).
    Sectors,
    /// Raw vertex points only.
    Vertices,
}

impl ViewMode {
    const ALL: &'static [(ViewMode, &'static str)] = &[
        (ViewMode::Map, "Map"),
        (ViewMode::Things, "Things"),
        (ViewMode::Linedefs, "Linedefs"),
        (ViewMode::Sectors, "Sectors"),
        (ViewMode::Vertices, "Vertices"),
    ];
}

pub struct WadViewer {
    state: ViewerState,
    /// Scene's view bounds in map coordinates. `Rect::ZERO` triggers
    /// auto-fit-to-content on the next frame, which is how the
    /// "Fit to map" button and post-load auto-fit work.
    scene_rect: Rect,
    /// Persisted across map switches so the user's view doesn't reset
    /// when they pick a new level.
    show_decorations: bool,
    /// Active visualization mode — gates which elements are
    /// foregrounded vs. dimmed in `draw_map`.
    mode: ViewMode,
}

// `Loaded` carries the full parsed Map (vertexes/linedefs/sidedefs/
// sectors/things — easily a few KB on a real level), while `Empty`
// holds at most an error string. There's exactly one `ViewerState`
// per `WadViewer` instance so the size disparity has no allocation
// cost; we silence the lint rather than boxing.
#[allow(clippy::large_enum_variant)]
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
            show_decorations: false,
            mode: ViewMode::Map,
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
            ui.label(egui::RichText::new("View:").small().weak());
            for (mode, label) in ViewMode::ALL {
                ui.selectable_value(&mut self.mode, *mode, *label);
            }
            // Decoration toggle is only meaningful when things are
            // foregrounded — it'd be confusing to expose in Sectors /
            // Linedefs / Vertices modes where we don't draw things at
            // full strength.
            if matches!(self.mode, ViewMode::Map | ViewMode::Things) {
                ui.separator();
                ui.checkbox(&mut self.show_decorations, "Decorations");
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
                let show_decorations = self.show_decorations;
                let mode = self.mode;
                // Carve out the rect Scene will paint into so we can
                // hit-test cursor-over-viewport for the plain-scroll
                // zoom override below. We don't actually allocate the
                // rect (Scene does that), we just predict its bounds.
                let viewport = ui.available_rect_before_wrap();
                self.handle_wheel_zoom(ui, viewport);
                Scene::new()
                    .zoom_range(Rangef::new(0.005, 5.0))
                    .show(ui, &mut self.scene_rect, |scene_ui| {
                        draw_map(scene_ui, &map, mode, show_decorations);
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
fn draw_map(ui: &mut Ui, map: &Map, mode: ViewMode, show_decorations: bool) {
    // Force Scene's auto-fit to see the actual map extent. See
    // `scene_bbox` rationale below.
    let bbox = scene_bbox(map);
    let _ = ui.allocate_rect(bbox, egui::Sense::hover());

    let style = mode.geometry_style();
    draw_linedefs(ui, map, mode, &style);

    match mode {
        ViewMode::Map | ViewMode::Things => {
            draw_things(ui, map, show_decorations, 1.0);
        }
        ViewMode::Linedefs | ViewMode::Sectors => {
            // Mode emphasis comes from the linedef pass above; no
            // overlay needed.
        }
        ViewMode::Vertices => draw_vertices(ui, map),
    }
}

/// Per-mode rendering knobs for the linedef pass — the only geometry
/// layer that's always drawn. Modes that foreground something else
/// (Things, Vertices) dim the geometry so the foreground reads.
struct GeoStyle {
    alpha: f32,
    solid_w: f32,
    portal_w: f32,
}

impl ViewMode {
    fn geometry_style(self) -> GeoStyle {
        match self {
            ViewMode::Map | ViewMode::Linedefs | ViewMode::Sectors => GeoStyle {
                alpha: 1.0,
                solid_w: 1.4,
                portal_w: 0.6,
            },
            ViewMode::Things => GeoStyle {
                alpha: 0.45,
                solid_w: 1.0,
                portal_w: 0.4,
            },
            ViewMode::Vertices => GeoStyle {
                alpha: 0.25,
                solid_w: 0.7,
                portal_w: 0.3,
            },
        }
    }
}

fn draw_linedefs(ui: &Ui, map: &Map, mode: ViewMode, style: &GeoStyle) {
    let painter = ui.painter();
    for ld in &map.linedefs {
        let (Some(va), Some(vb)) = (
            map.vertexes.get(ld.v1 as usize),
            map.vertexes.get(ld.v2 as usize),
        ) else {
            continue;
        };
        let p1 = Pos2::new(va.x as f32, -va.y as f32);
        let p2 = Pos2::new(vb.x as f32, -vb.y as f32);
        let stroke = linedef_stroke(map, ld, mode, style);
        painter.line_segment([p1, p2], stroke);
    }
}

fn draw_things(ui: &Ui, map: &Map, show_decorations: bool, alpha: f32) {
    let painter = ui.painter();
    for t in &map.things {
        let cat = ThingCategory::classify(t.doom_type);
        if matches!(cat, ThingCategory::Decoration) && !show_decorations {
            continue;
        }
        let p = Pos2::new(t.x as f32, -t.y as f32);
        let (color, radius) = thing_visual(cat);
        painter.circle_filled(p, radius, with_alpha(color, alpha));
        if matches!(cat, ThingCategory::PlayerStart) {
            let ang = (t.angle as f32).to_radians();
            let tip = Pos2::new(
                p.x + ang.cos() * radius * 2.5,
                p.y - ang.sin() * radius * 2.5,
            );
            painter.line_segment(
                [p, tip],
                Stroke::new(1.0, with_alpha(color, alpha)),
            );
        }
    }
}

fn draw_vertices(ui: &Ui, map: &Map) {
    let painter = ui.painter();
    let color = Color32::from_rgb(255, 220, 130);
    for v in &map.vertexes {
        painter.circle_filled(
            Pos2::new(v.x as f32, -v.y as f32),
            1.6,
            color,
        );
    }
}

fn with_alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        c.r(),
        c.g(),
        c.b(),
        (a.clamp(0.0, 1.0) * 255.0) as u8,
    )
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

fn linedef_stroke(map: &Map, ld: &Linedef, mode: ViewMode, style: &GeoStyle) -> Stroke {
    let two_sided = ld.two_sided();
    let width = if two_sided { style.portal_w } else { style.solid_w };
    let base = match mode {
        ViewMode::Linedefs => {
            // Triggers / switches color-coded so interactive lines pop.
            if ld.special != 0 {
                Color32::from_rgb(220, 180, 90)
            } else if two_sided {
                Color32::from_rgb(120, 115, 110)
            } else {
                Color32::from_rgb(230, 225, 215)
            }
        }
        ViewMode::Sectors => sector_color(map, ld),
        _ => {
            if two_sided {
                Color32::from_rgb(100, 95, 90)
            } else {
                Color32::from_rgb(220, 215, 200)
            }
        }
    };
    Stroke::new(width, with_alpha(base, style.alpha))
}

/// Pick the color for a linedef in Sectors mode by walking through
/// its right-side sidedef to the sector. Falls back to dim gray if
/// either ref is missing.
fn sector_color(map: &Map, ld: &Linedef) -> Color32 {
    let Some(sec_idx) = map.sector_of_right_side(ld) else {
        return Color32::from_rgb(120, 120, 130);
    };
    let Some(sector) = map.sectors.get(sec_idx as usize) else {
        return Color32::from_rgb(120, 120, 130);
    };
    floor_height_color(sector, &map.sectors)
}

/// Cool→warm 3-stop gradient over the level's observed floor-height
/// range. Cheap heatmap that doesn't require building polygons.
fn floor_height_color(sector: &Sector, all: &[Sector]) -> Color32 {
    if all.is_empty() {
        return Color32::from_rgb(180, 180, 180);
    }
    let (mut lo, mut hi) = (i16::MAX, i16::MIN);
    for s in all {
        lo = lo.min(s.floor_height);
        hi = hi.max(s.floor_height);
    }
    let range = (hi - lo).max(1) as f32;
    let t = ((sector.floor_height - lo) as f32 / range).clamp(0.0, 1.0);
    let (lo_c, mid_c, hi_c) = (
        (60.0_f32, 110.0, 140.0),
        (180.0_f32, 160.0, 110.0),
        (220.0_f32, 110.0, 80.0),
    );
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let (r, g, b) = if t < 0.5 {
        let u = t * 2.0;
        (
            lerp(lo_c.0, mid_c.0, u),
            lerp(lo_c.1, mid_c.1, u),
            lerp(lo_c.2, mid_c.2, u),
        )
    } else {
        let u = (t - 0.5) * 2.0;
        (
            lerp(mid_c.0, hi_c.0, u),
            lerp(mid_c.1, hi_c.1, u),
            lerp(mid_c.2, hi_c.2, u),
        )
    };
    Color32::from_rgb(r as u8, g as u8, b as u8)
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
