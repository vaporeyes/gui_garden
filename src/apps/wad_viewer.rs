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

use crate::wad::{
    linedef_flag_descriptions, linedef_special_name, sector_special_name,
    thing_flag_descriptions, thing_type_name, Linedef, Map, Sector, Thing,
    ThingCategory, Wad,
};

/// What the user has clicked on. Each variant carries the index back
/// into the relevant `Map` collection so the inspector can look up
/// fresh data each frame (re-rendering on map switch is a no-op).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Selection {
    Thing(usize),
    Linedef(usize),
    Sector(u16),
}

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
    /// Currently inspected element. Set by clicking; cleared by
    /// clicking empty space, switching to an incompatible mode, or
    /// pressing Escape.
    selected: Option<Selection>,
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
            selected: None,
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
            // After the row updates, drop the selection if it doesn't
            // belong in the new mode (e.g. a Thing selected, then user
            // jumps to Sectors).
            if !mode_keeps_selection(self.mode, self.selected) {
                self.selected = None;
            }
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
                let selected = self.selected;
                // Right-side info panel — branches by selection variant.
                if let Some(sel) = selected {
                    let mut close = false;
                    egui::Panel::right("wad_inspector")
                        .resizable(false)
                        .default_size(240.0)
                        .show_inside(ui, |ui| {
                            close = draw_inspector(ui, &map, sel);
                        });
                    if close {
                        self.selected = None;
                    }
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.selected = None;
                }
                // Predict the scene viewport before Scene grabs it,
                // so the wheel-zoom intercept can hit-test
                // cursor-over-area.
                let viewport = ui.available_rect_before_wrap();
                self.handle_wheel_zoom(ui, viewport);
                let inner = Scene::new()
                    .zoom_range(Rangef::new(0.005, 5.0))
                    .show(ui, &mut self.scene_rect, |scene_ui| {
                        draw_map(scene_ui, &map, mode, show_decorations, selected);
                    });
                if inner.response.clicked() {
                    self.selected =
                        hit_test_for_mode(&map, &inner.response, self.scene_rect, mode);
                }
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
fn draw_map(
    ui: &mut Ui,
    map: &Map,
    mode: ViewMode,
    show_decorations: bool,
    selected: Option<Selection>,
) {
    // Force Scene's auto-fit to see the actual map extent. See
    // `scene_bbox` rationale below.
    let bbox = scene_bbox(map);
    let _ = ui.allocate_rect(bbox, egui::Sense::hover());

    let style = mode.geometry_style();
    draw_linedefs(ui, map, mode, &style);

    let selected_thing = match selected {
        Some(Selection::Thing(i)) => Some(i),
        _ => None,
    };
    match mode {
        ViewMode::Map | ViewMode::Things => {
            draw_things(ui, map, show_decorations, 1.0, selected_thing);
        }
        ViewMode::Linedefs | ViewMode::Sectors => {}
        ViewMode::Vertices => draw_vertices(ui, map),
    }

    // Selection overlay — drawn last so it sits above everything.
    match selected {
        Some(Selection::Linedef(i)) => draw_selected_linedef(ui, map, i),
        Some(Selection::Sector(s)) => draw_selected_sector(ui, map, s),
        _ => {}
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

fn draw_things(
    ui: &Ui,
    map: &Map,
    show_decorations: bool,
    alpha: f32,
    selected: Option<usize>,
) {
    let painter = ui.painter();
    for (i, t) in map.things.iter().enumerate() {
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
        // Selection ring — drawn after the dot so it sits on top.
        // Cyan matches the linedef/sector highlight for a consistent
        // "this is selected" cue across all three modes.
        if Some(i) == selected {
            painter.circle_stroke(
                p,
                radius + 4.0,
                Stroke::new(2.5, SELECT_COLOR),
            );
            painter.circle_stroke(
                p,
                radius + 7.0,
                Stroke::new(
                    1.5,
                    Color32::from_rgba_unmultiplied(0, 220, 255, 140),
                ),
            );
        }
    }
}

/// Did the new mode permit keeping `sel` as the active selection?
/// Map allows everything (it's the catch-all view); the specialized
/// modes only keep their own kind.
fn mode_keeps_selection(mode: ViewMode, sel: Option<Selection>) -> bool {
    let Some(sel) = sel else {
        return true;
    };
    matches!(
        (mode, sel),
        (ViewMode::Map, _)
            | (ViewMode::Things, Selection::Thing(_))
            | (ViewMode::Linedefs, Selection::Linedef(_))
            | (ViewMode::Sectors, Selection::Sector(_))
    )
}

/// Top-level click dispatcher. Each mode picks the right kind of
/// hit-test; Map mode prefers things over linedefs (smaller targets
/// should win on overlap), Sectors mode looks up the right-side
/// sector of whatever linedef the cursor is closest to.
fn hit_test_for_mode(
    map: &Map,
    response: &egui::Response,
    scene_rect: Rect,
    mode: ViewMode,
) -> Option<Selection> {
    match mode {
        ViewMode::Map => hit_test_thing(map, response, scene_rect)
            .map(Selection::Thing)
            .or_else(|| {
                hit_test_linedef(map, response, scene_rect).map(Selection::Linedef)
            }),
        ViewMode::Things => {
            hit_test_thing(map, response, scene_rect).map(Selection::Thing)
        }
        ViewMode::Linedefs => {
            hit_test_linedef(map, response, scene_rect).map(Selection::Linedef)
        }
        ViewMode::Sectors => {
            let ld_idx = hit_test_linedef(map, response, scene_rect)?;
            let ld = map.linedefs.get(ld_idx)?;
            map.sector_of_right_side(ld).map(Selection::Sector)
        }
        ViewMode::Vertices => None,
    }
}

/// Cursor position in scene coords, plus the zoom-aware click radius
/// in scene units. Shared between the thing/linedef hit tests so the
/// math doesn't drift between them.
fn cursor_and_radius(
    response: &egui::Response,
    scene_rect: Rect,
    pixel_radius: f32,
) -> Option<(Pos2, f32)> {
    let cursor = response.interact_pointer_pos()?;
    let outer = response.rect;
    if outer.width() <= 0.0 || outer.height() <= 0.0 {
        return None;
    }
    let tx = (cursor.x - outer.left()) / outer.width();
    let ty = (cursor.y - outer.top()) / outer.height();
    let scene_pt = Pos2::new(
        scene_rect.left() + tx * scene_rect.width(),
        scene_rect.top() + ty * scene_rect.height(),
    );
    let scale = scene_rect.width() / outer.width().max(1.0);
    let radius = (pixel_radius * scale).max(pixel_radius * 0.5);
    Some((scene_pt, radius))
}

/// Closest linedef within the zoom-aware click radius, or `None` for
/// empty-space clicks. Distance is point-to-segment in scene coords.
fn hit_test_linedef(
    map: &Map,
    response: &egui::Response,
    scene_rect: Rect,
) -> Option<usize> {
    let (scene_pt, radius) = cursor_and_radius(response, scene_rect, 6.0)?;
    let radius_sq = radius * radius;
    let mut best: Option<(usize, f32)> = None;
    for (i, ld) in map.linedefs.iter().enumerate() {
        let (Some(va), Some(vb)) = (
            map.vertexes.get(ld.v1 as usize),
            map.vertexes.get(ld.v2 as usize),
        ) else {
            continue;
        };
        let a = Pos2::new(va.x as f32, -va.y as f32);
        let b = Pos2::new(vb.x as f32, -vb.y as f32);
        let d2 = dist_point_segment_sq(scene_pt, a, b);
        if d2 <= radius_sq && best.is_none_or(|(_, bd2)| d2 < bd2) {
            best = Some((i, d2));
        }
    }
    best.map(|(i, _)| i)
}

fn dist_point_segment_sq(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let abx = b.x - a.x;
    let aby = b.y - a.y;
    let len_sq = abx * abx + aby * aby;
    if len_sq < 1e-6 {
        let dx = p.x - a.x;
        let dy = p.y - a.y;
        return dx * dx + dy * dy;
    }
    let t = ((p.x - a.x) * abx + (p.y - a.y) * aby) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let projx = a.x + t * abx;
    let projy = a.y + t * aby;
    let dx = p.x - projx;
    let dy = p.y - projy;
    dx * dx + dy * dy
}

/// Editor-convention "selected" color: a saturated cyan that's
/// distinct from any wall color (whites, dim grays, amber specials,
/// the floor-height heatmap) and reads as "I clicked this".
const SELECT_COLOR: Color32 = Color32::from_rgb(0, 220, 255);

/// Highlight a single linedef as a thick, bright overlay on top of
/// the geometry pass. Three layers — a wide soft halo, a medium
/// translucent halo, and a crisp inner stroke — so the line stays
/// visible against any backdrop and at any zoom level.
fn draw_selected_linedef(ui: &Ui, map: &Map, idx: usize) {
    let Some(ld) = map.linedefs.get(idx) else {
        return;
    };
    let (Some(va), Some(vb)) = (
        map.vertexes.get(ld.v1 as usize),
        map.vertexes.get(ld.v2 as usize),
    ) else {
        return;
    };
    let p1 = Pos2::new(va.x as f32, -va.y as f32);
    let p2 = Pos2::new(vb.x as f32, -vb.y as f32);
    let painter = ui.painter();
    // Outer halo — wide and faint, anchors the eye.
    painter.line_segment(
        [p1, p2],
        Stroke::new(
            10.0,
            Color32::from_rgba_unmultiplied(0, 220, 255, 60),
        ),
    );
    // Mid halo — narrower and stronger.
    painter.line_segment(
        [p1, p2],
        Stroke::new(
            6.0,
            Color32::from_rgba_unmultiplied(0, 220, 255, 140),
        ),
    );
    // Crisp solid inner stroke.
    painter.line_segment([p1, p2], Stroke::new(3.0, SELECT_COLOR));
    // Endpoint dots so it's clear which vertices anchor the line.
    painter.circle_filled(p1, 5.0, SELECT_COLOR);
    painter.circle_filled(p2, 5.0, SELECT_COLOR);
    painter.circle_stroke(
        p1,
        5.0,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 200)),
    );
    painter.circle_stroke(
        p2,
        5.0,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 200)),
    );
}

/// Highlight every linedef whose right-side sector matches the
/// selected sector index.
fn draw_selected_sector(ui: &Ui, map: &Map, sector: u16) {
    let painter = ui.painter();
    for ld in &map.linedefs {
        if map.sector_of_right_side(ld) != Some(sector) {
            continue;
        }
        let (Some(va), Some(vb)) = (
            map.vertexes.get(ld.v1 as usize),
            map.vertexes.get(ld.v2 as usize),
        ) else {
            continue;
        };
        let p1 = Pos2::new(va.x as f32, -va.y as f32);
        let p2 = Pos2::new(vb.x as f32, -vb.y as f32);
        painter.line_segment(
            [p1, p2],
            Stroke::new(
                7.0,
                Color32::from_rgba_unmultiplied(0, 220, 255, 70),
            ),
        );
        painter.line_segment([p1, p2], Stroke::new(2.4, SELECT_COLOR));
    }
}

/// Map cursor pos to scene coords, then find the closest thing within
/// a zoom-aware click radius. Returns `None` for an empty-space click,
/// which the caller uses to deselect.
fn hit_test_thing(map: &Map, response: &egui::Response, scene_rect: Rect) -> Option<usize> {
    let (scene_pt, radius) = cursor_and_radius(response, scene_rect, 8.0)?;
    let radius_sq = radius * radius;
    let mut best: Option<(usize, f32)> = None;
    for (i, t) in map.things.iter().enumerate() {
        let tp = Pos2::new(t.x as f32, -t.y as f32);
        let dx = scene_pt.x - tp.x;
        let dy = scene_pt.y - tp.y;
        let d2 = dx * dx + dy * dy;
        if d2 <= radius_sq && best.is_none_or(|(_, bd2)| d2 < bd2) {
            best = Some((i, d2));
        }
    }
    best.map(|(i, _)| i)
}

/// Inspector dispatcher — branches on the selection variant. Each
/// arm renders into the same right-rail panel and returns `true` when
/// the close button was clicked.
fn draw_inspector(ui: &mut Ui, map: &Map, sel: Selection) -> bool {
    match sel {
        Selection::Thing(idx) => match map.things.get(idx) {
            Some(t) => draw_thing_info(ui, t),
            None => placeholder_inspector(ui, "Thing index out of range"),
        },
        Selection::Linedef(idx) => match map.linedefs.get(idx) {
            Some(ld) => draw_linedef_info(ui, map, idx, ld),
            None => placeholder_inspector(ui, "Linedef index out of range"),
        },
        Selection::Sector(idx) => match map.sectors.get(idx as usize) {
            Some(s) => draw_sector_info(ui, map, idx, s),
            None => placeholder_inspector(ui, "Sector index out of range"),
        },
    }
}

fn placeholder_inspector(ui: &mut Ui, msg: &str) -> bool {
    let mut close = false;
    egui::Sides::new().show(
        ui,
        |ui| {
            ui.heading("Inspector");
        },
        |ui| {
            if ui.button("✕").clicked() {
                close = true;
            }
        },
    );
    ui.label(egui::RichText::new(msg).weak());
    close
}

/// Linedef inspector — vertex coords, length in map units, decoded
/// flags, special name, sidedef→sector resolution.
fn draw_linedef_info(ui: &mut Ui, map: &Map, idx: usize, ld: &Linedef) -> bool {
    let mut close = false;
    egui::Sides::new().show(
        ui,
        |ui| {
            ui.heading(
                egui::RichText::new(format!("Linedef #{}", idx))
                    .color(Color32::from_rgb(220, 215, 200)),
            );
        },
        |ui| {
            if ui.button("✕").on_hover_text("Close (Esc)").clicked() {
                close = true;
            }
        },
    );
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new(if ld.two_sided() {
            "Two-sided portal"
        } else {
            "One-sided wall"
        })
        .small()
        .weak(),
    );
    ui.add_space(8.0);

    // Compute length from vertex positions for a useful at-a-glance
    // metric.
    let length = match (
        map.vertexes.get(ld.v1 as usize),
        map.vertexes.get(ld.v2 as usize),
    ) {
        (Some(va), Some(vb)) => {
            let dx = (vb.x - va.x) as f32;
            let dy = (vb.y - va.y) as f32;
            (dx * dx + dy * dy).sqrt()
        }
        _ => 0.0,
    };

    egui::Grid::new("wad_linedef_info_grid")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("v1").weak());
            ui.label(format!(
                "{} {}",
                ld.v1,
                vertex_summary(map, ld.v1)
            ));
            ui.end_row();

            ui.label(egui::RichText::new("v2").weak());
            ui.label(format!(
                "{} {}",
                ld.v2,
                vertex_summary(map, ld.v2)
            ));
            ui.end_row();

            ui.label(egui::RichText::new("Length").weak());
            ui.label(format!("{:.0} units", length));
            ui.end_row();

            ui.label(egui::RichText::new("Special").weak());
            let special_label = linedef_special_name(ld.special)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("#{}", ld.special));
            ui.label(special_label);
            ui.end_row();

            ui.label(egui::RichText::new("Tag").weak());
            ui.label(format!("{}", ld.tag));
            ui.end_row();

            ui.label(egui::RichText::new("Flags").weak());
            ui.label(format!("0x{:04X}", ld.flags));
            ui.end_row();

            ui.label(egui::RichText::new("Right side").weak());
            ui.label(sidedef_summary(map, ld.right_sidedef));
            ui.end_row();

            ui.label(egui::RichText::new("Left side").weak());
            ui.label(sidedef_summary(map, ld.left_sidedef));
            ui.end_row();
        });

    let descs = linedef_flag_descriptions(ld.flags);
    if !descs.is_empty() {
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Flag bits").small().weak());
        for d in descs {
            ui.label(format!("• {}", d));
        }
    }

    close
}

fn vertex_summary(map: &Map, idx: u16) -> String {
    match map.vertexes.get(idx as usize) {
        Some(v) => format!("({}, {})", v.x, v.y),
        None => "(invalid)".to_string(),
    }
}

fn sidedef_summary(map: &Map, idx: u16) -> String {
    if idx == 0xFFFF {
        return "(none)".to_string();
    }
    match map.sidedefs.get(idx as usize) {
        Some(sd) => format!("#{} → sector {}", idx, sd.sector),
        None => format!("#{} (invalid)", idx),
    }
}

/// Sector inspector — heights, light, special, plus a small count of
/// the linedefs that face into this sector (a cheap proxy for "how
/// big is this region").
fn draw_sector_info(ui: &mut Ui, map: &Map, idx: u16, sector: &Sector) -> bool {
    let mut close = false;
    let color = floor_height_color(sector, &map.sectors);
    egui::Sides::new().show(
        ui,
        |ui| {
            ui.heading(
                egui::RichText::new(format!("Sector #{}", idx)).color(color),
            );
        },
        |ui| {
            if ui.button("✕").on_hover_text("Close (Esc)").clicked() {
                close = true;
            }
        },
    );
    ui.add_space(2.0);
    let special_label = sector_special_name(sector.special)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Special #{}", sector.special));
    ui.label(egui::RichText::new(special_label).small().weak());
    ui.add_space(8.0);

    let referencing = map
        .linedefs
        .iter()
        .filter(|ld| {
            map.sector_of_right_side(ld) == Some(idx)
                || map
                    .sidedefs
                    .get(ld.left_sidedef as usize)
                    .is_some_and(|sd| sd.sector == idx)
        })
        .count();

    egui::Grid::new("wad_sector_info_grid")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Floor h").weak());
            ui.label(format!("{}", sector.floor_height));
            ui.end_row();

            ui.label(egui::RichText::new("Ceil h").weak());
            ui.label(format!("{}", sector.ceiling_height));
            ui.end_row();

            ui.label(egui::RichText::new("Height").weak());
            ui.label(format!(
                "{} units",
                sector.ceiling_height as i32 - sector.floor_height as i32
            ));
            ui.end_row();

            ui.label(egui::RichText::new("Floor tex").weak());
            ui.label(&sector.floor_tex);
            ui.end_row();

            ui.label(egui::RichText::new("Ceil tex").weak());
            ui.label(&sector.ceiling_tex);
            ui.end_row();

            ui.label(egui::RichText::new("Light").weak());
            ui.label(format!("{}", sector.light));
            ui.end_row();

            ui.label(egui::RichText::new("Tag").weak());
            ui.label(format!("{}", sector.tag));
            ui.end_row();

            ui.label(egui::RichText::new("Linedefs").weak());
            ui.label(format!("{}", referencing));
            ui.end_row();
        });

    close
}

/// Thing inspector — right-rail panel content. Returns `true` when
/// the user clicked the close button.
fn draw_thing_info(ui: &mut Ui, thing: &Thing) -> bool {
    let category = ThingCategory::classify(thing.doom_type);
    let (color, _radius) = thing_visual(category);
    let mut close = false;

    egui::Sides::new().show(
        ui,
        |ui| {
            ui.heading(
                egui::RichText::new(
                    thing_type_name(thing.doom_type)
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("Type {}", thing.doom_type)),
                )
                .color(color),
            );
        },
        |ui| {
            if ui.button("✕").on_hover_text("Close (Esc)").clicked() {
                close = true;
            }
        },
    );
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new(format!("{:?}", category))
            .small()
            .weak(),
    );
    ui.add_space(8.0);

    egui::Grid::new("wad_thing_info_grid")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Type").weak());
            ui.label(format!("{}", thing.doom_type));
            ui.end_row();

            ui.label(egui::RichText::new("Position").weak());
            ui.label(format!("({}, {})", thing.x, thing.y));
            ui.end_row();

            ui.label(egui::RichText::new("Angle").weak());
            ui.label(format!("{}°", thing.angle));
            ui.end_row();

            ui.label(egui::RichText::new("Flags").weak());
            ui.label(format!("0x{:04X}", thing.flags));
            ui.end_row();
        });

    let descs = thing_flag_descriptions(thing.flags);
    if !descs.is_empty() {
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Spawn conditions").small().weak());
        for d in descs {
            ui.label(format!("• {}", d));
        }
    }

    close
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
