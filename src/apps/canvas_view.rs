// JSON Canvas (jsoncanvas.org / Obsidian) viewer.
//
// Loads a canvas JSON file from disk (native only) and renders its fixed-
// position rectangles and edges with pan + zoom. Reuses the astro-blog's
// canvas schema verbatim so any canvas authored there or in Obsidian can
// be dropped in.

use egui::epaint::CubicBezierShape;
use egui::{Color32, Pos2, Rangef, Rect, Scene, Sense, Shape, Stroke, Ui, UiBuilder, Vec2};
use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::digital_garden::NoteDirectory;
use crate::palette;

#[derive(Debug, Deserialize, Clone)]
pub struct CanvasDocument {
    pub title: Option<String>,
    /// Canvas description from the JSON; not yet surfaced in the UI but
    /// retained so it's available for future tooltip / header use.
    #[allow(dead_code)]
    pub description: Option<String>,
    #[serde(default)]
    pub nodes: Vec<CanvasNode>,
    #[serde(default)]
    pub edges: Vec<CanvasEdge>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CanvasNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub text: Option<String>,
    pub url: Option<String>,
    pub file: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CanvasEdge {
    /// Schema id, preserved but unused by the renderer (no per-edge state).
    #[allow(dead_code)]
    pub id: String,
    pub from_node: Option<String>,
    pub to_node: Option<String>,
    #[serde(rename = "fromNode")]
    pub from_node_alt: Option<String>,
    #[serde(rename = "toNode")]
    pub to_node_alt: Option<String>,
    pub from_side: Option<String>,
    pub to_side: Option<String>,
    #[serde(rename = "fromSide")]
    pub from_side_alt: Option<String>,
    #[serde(rename = "toSide")]
    pub to_side_alt: Option<String>,
    /// Edge label; reserved for a future hover affordance.
    #[allow(dead_code)]
    pub label: Option<String>,
}

impl CanvasEdge {
    fn from(&self) -> Option<&str> {
        self.from_node
            .as_deref()
            .or(self.from_node_alt.as_deref())
    }
    fn to(&self) -> Option<&str> {
        self.to_node.as_deref().or(self.to_node_alt.as_deref())
    }
    #[allow(clippy::wrong_self_convention)]
    fn from_side_resolved(&self) -> Option<Side> {
        Side::parse(
            self.from_side
                .as_deref()
                .or(self.from_side_alt.as_deref()),
        )
    }
    fn to_side_resolved(&self) -> Option<Side> {
        Side::parse(self.to_side.as_deref().or(self.to_side_alt.as_deref()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

impl Side {
    fn parse(s: Option<&str>) -> Option<Self> {
        match s? {
            "top" => Some(Side::Top),
            "right" => Some(Side::Right),
            "bottom" => Some(Side::Bottom),
            "left" => Some(Side::Left),
            _ => None,
        }
    }

    /// Unit vector pointing *outward* from the node on this side.
    fn outward(self) -> Vec2 {
        match self {
            Side::Top => Vec2::new(0.0, -1.0),
            Side::Right => Vec2::new(1.0, 0.0),
            Side::Bottom => Vec2::new(0.0, 1.0),
            Side::Left => Vec2::new(-1.0, 0.0),
        }
    }

    /// Point on the given rect where an edge with this side should attach.
    fn anchor(self, rect: Rect) -> Pos2 {
        match self {
            Side::Top => Pos2::new(rect.center().x, rect.top()),
            Side::Right => Pos2::new(rect.right(), rect.center().y),
            Side::Bottom => Pos2::new(rect.center().x, rect.bottom()),
            Side::Left => Pos2::new(rect.left(), rect.center().y),
        }
    }
}

pub struct CanvasView {
    loaded: Option<(PathBuf, CanvasDocument)>,
    /// View bounds in scene coordinates. `egui::Scene` mutates this in
    /// response to pan/zoom; assigning `Rect::ZERO` triggers Scene's
    /// auto-fit-to-content path on the next frame, which is how we
    /// implement "Reset view".
    scene_rect: Rect,
    error: Option<String>,
}

impl Default for CanvasView {
    fn default() -> Self {
        Self {
            loaded: None,
            scene_rect: Rect::ZERO,
            error: None,
        }
    }
}

impl CanvasView {
    #[cfg(not(target_arch = "wasm32"))]
    fn pick_and_load(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON Canvas", &["json", "canvas"])
            .set_title("Load a JSON Canvas")
            .pick_file()
        else {
            return;
        };
        self.load_from_path(path);
    }

    #[cfg(target_arch = "wasm32")]
    fn pick_and_load(&mut self) {
        self.error = Some("File picker unavailable on web.".into());
    }

    /// Load a canvas JSON file from an explicit path. Used at startup to
    /// auto-rehydrate the last file the user picked.
    pub fn load_from_path<P: Into<PathBuf>>(&mut self, path: P) {
        let path = path.into();
        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<CanvasDocument>(&content) {
                Ok(doc) => {
                    self.loaded = Some((path, doc));
                    self.error = None;
                    // Rect::ZERO triggers Scene's auto-fit on the next frame.
                    self.scene_rect = Rect::ZERO;
                }
                Err(e) => self.error = Some(format!("parse error: {}", e)),
            },
            Err(e) => self.error = Some(format!("read error: {}", e)),
        }
    }

    /// Path of the currently-loaded canvas (empty if nothing is loaded).
    pub fn loaded_path(&self) -> Option<&Path> {
        self.loaded.as_ref().map(|(p, _)| p.as_path())
    }

    /// Render the canvas. Returns the note id the user clicked either on
    /// a `type: "file"` node OR on an internal wiki-link inside a text
    /// node — the caller uses it to open that note in the Digital Garden.
    ///
    /// `directory` is the currently-loaded notes directory, if any. When
    /// present, text nodes are rendered via the full markdown parser
    /// (bold, italics, code blocks, wiki-links, images). When absent, or
    /// when a node's rect is too small to bother with markdown, a plain
    /// galley is used instead.
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        directory: Option<&NoteDirectory>,
    ) -> Option<String> {
        let accent = palette::accent_now();

        let mut reset_view = false;
        ui.horizontal(|ui| {
            if ui.button("📁 Load canvas…").clicked() {
                self.pick_and_load();
            }
            if self.loaded.is_some() && ui.button("Reset view").clicked() {
                reset_view = true;
            }
            if let Some((path, doc)) = &self.loaded {
                ui.label(
                    egui::RichText::new(format!(
                        "{}  ({} nodes, {} edges)",
                        doc.title
                            .clone()
                            .unwrap_or_else(|| path
                                .file_name()
                                .map(|f| f.to_string_lossy().to_string())
                                .unwrap_or_default()),
                        doc.nodes.len(),
                        doc.edges.len(),
                    ))
                    .weak(),
                );
            }
        });
        if reset_view {
            // Rect::ZERO trips Scene's auto-fit-to-content path on the
            // next frame, which is exactly the "fit everything in view"
            // behavior the old custom code emulated.
            self.scene_rect = Rect::ZERO;
        }
        if let Some(err) = &self.error {
            ui.colored_label(Color32::from_rgb(220, 80, 80), err);
        }
        ui.separator();

        let Some((_, doc)) = self.loaded.clone() else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new(
                        "No canvas loaded. Click \"Load canvas\" to open a .json / .canvas file.",
                    )
                    .weak(),
                );
            });
            return None;
        };

        // Hand off pan/zoom to `egui::Scene`. The closure draws in scene
        // coordinates directly — no manual offset/scale projection. We
        // widen the default `0.0..=1.0` zoom range so users can zoom in
        // past 1:1 to read text-heavy nodes.
        let mut clicked_file: Option<String> = None;
        let scene_rect_snapshot = self.scene_rect;
        Scene::new()
            .zoom_range(Rangef::new(0.05, 5.0))
            .show(ui, &mut self.scene_rect, |scene_ui| {
                draw_canvas(
                    scene_ui,
                    &doc,
                    directory,
                    accent,
                    scene_rect_snapshot,
                    &mut clicked_file,
                );
            });

        clicked_file
    }
}

/// Draw the canvas inside the Scene's transformed sub-ui. All coordinates
/// here are *scene coords* — Scene applies the visible transform on its
/// own layer, so we draw nodes at their literal `(x, y, w, h)` positions
/// from the JSON.
fn draw_canvas(
    ui: &mut Ui,
    doc: &CanvasDocument,
    directory: Option<&NoteDirectory>,
    accent: Color32,
    visible: Rect,
    clicked_file: &mut Option<String>,
) {
    // Infinite dot grid backdrop, rendered in scene coords. Only draw
    // dots that fall inside the visible scene rect, snapped to a fixed
    // grid spacing so the pattern doesn't slide as the user pans.
    if visible.is_finite() && visible.size() != Vec2::ZERO {
        const GRID_STEP: f32 = 40.0;
        let dot_color =
            Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 22);
        let x_start = (visible.left() / GRID_STEP).floor() * GRID_STEP;
        let y_start = (visible.top() / GRID_STEP).floor() * GRID_STEP;
        let mut x = x_start;
        while x <= visible.right() {
            let mut y = y_start;
            while y <= visible.bottom() {
                ui.painter().circle_filled(Pos2::new(x, y), 1.0, dot_color);
                y += GRID_STEP;
            }
            x += GRID_STEP;
        }
    }

    let mut shapes: Vec<Shape> = Vec::new();

    // Edges — drawn first so nodes paint on top.
    let node_rects: std::collections::HashMap<&str, Rect> = doc
        .nodes
        .iter()
        .map(|n| {
            let tl = Pos2::new(n.x, n.y);
            let br = Pos2::new(n.x + n.width, n.y + n.height);
            (n.id.as_str(), Rect::from_two_pos(tl, br))
        })
        .collect();

    for edge in &doc.edges {
        let (Some(from_id), Some(to_id)) = (edge.from(), edge.to()) else {
            continue;
        };
        let (Some(from_rect), Some(to_rect)) =
            (node_rects.get(from_id), node_rects.get(to_id))
        else {
            continue;
        };
        let from_rect = *from_rect;
        let to_rect = *to_rect;

        // Resolve sides: prefer explicit `from_side` / `to_side` from
        // the JSON; fall back to whichever side faces the other node.
        let from_side = edge
            .from_side_resolved()
            .unwrap_or_else(|| infer_side(from_rect, to_rect.center()));
        let to_side = edge
            .to_side_resolved()
            .unwrap_or_else(|| infer_side(to_rect, from_rect.center()));

        let p0 = from_side.anchor(from_rect);
        let p3 = to_side.anchor(to_rect);
        let straight = (p3 - p0).length();
        // Control-point distance scales with the straight-line distance
        // so short edges curve tightly and long edges sweep gently.
        let control_dist = (straight * 0.5).clamp(30.0, 240.0);
        let p1 = p0 + from_side.outward() * control_dist;
        let p2 = p3 + to_side.outward() * control_dist;

        shapes.push(Shape::CubicBezier(CubicBezierShape::from_points_stroke(
            [p0, p1, p2, p3],
            false,
            Color32::TRANSPARENT,
            Stroke::new(1.5, accent.linear_multiply(0.7)),
        )));
    }

    // Nodes. Render `group` types first so they sit behind everything else.
    let mut ordered: Vec<&CanvasNode> = doc.nodes.iter().collect();
    ordered.sort_by_key(|n| match n.node_type.as_str() {
        "group" => 0,
        _ => 1,
    });

    // Flush edge shapes before nodes so per-node sub-uis paint on top.
    ui.painter().extend(std::mem::take(&mut shapes));

    for node in ordered {
        let rect = Rect::from_min_size(
            Pos2::new(node.x, node.y),
            Vec2::new(node.width, node.height),
        );

        // File nodes are clickable — allocate a real interactive rect so
        // egui handles hover/cursor/click in the natural way and the
        // Scene's background pan still works on empty space.
        let is_file = node.node_type == "file";
        let is_file_hovered = if is_file && node.file.is_some() {
            let resp = ui.allocate_rect(rect, Sense::click());
            if resp.clicked() {
                if let Some(file) = &node.file {
                    *clicked_file = Some(file_to_note_id(file));
                }
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            resp.hovered()
        } else {
            false
        };

        let (fill, stroke_color) = match node.node_type.as_str() {
            "group" => (
                accent.linear_multiply(0.06),
                accent.linear_multiply(0.4),
            ),
            "link" => (
                Color32::from_rgba_unmultiplied(60, 100, 160, 40),
                Color32::from_rgb(100, 150, 220),
            ),
            "file" if is_file_hovered => (
                accent.linear_multiply(0.18),
                accent,
            ),
            "file" => (
                Color32::from_rgba_unmultiplied(90, 90, 90, 60),
                Color32::from_rgb(160, 160, 160),
            ),
            _ => (
                ui.visuals().faint_bg_color,
                accent.linear_multiply(0.8),
            ),
        };

        // Drop shadow: a dark, semi-transparent rect offset down-right
        // behind the node, with slightly larger corner radius. egui has
        // no native drop-shadow primitive, so this is the cheap fake.
        // `group` nodes act as background containers and are big — a
        // shadow on them looks heavy, so skip those.
        if node.node_type != "group" {
            let shadow_rect =
                rect.translate(Vec2::new(3.0, 4.0)).expand(0.5);
            ui.painter().rect_filled(
                shadow_rect,
                5.0,
                Color32::from_black_alpha(60),
            );
        }
        ui.painter().rect_filled(rect, 4.0, fill);
        ui.painter().rect_stroke(
            rect,
            4.0,
            Stroke::new(if is_file_hovered { 1.5 } else { 1.0 }, stroke_color),
            egui::StrokeKind::Outside,
        );

        // Text content inside the node, if any.
        let content = node
            .text
            .as_deref()
            .or(node.label.as_deref())
            .or(node.file.as_deref())
            .or(node.url.as_deref());
        if let Some(text) = content {
            // For text-type nodes with a loaded notes directory and
            // enough room to be worth it, render through the full
            // markdown parser. Otherwise fall back to a fixed-size
            // galley — Scene scales it visually for us.
            let inner = rect.shrink(6.0);
            let can_render_markdown = node.node_type == "text"
                && directory.is_some()
                && inner.width() > 80.0
                && inner.height() > 32.0;
            if can_render_markdown {
                let dir = directory.unwrap();
                render_markdown_in_node(ui, inner, text, dir, clicked_file);
            } else {
                let font = egui::FontId::proportional(12.0);
                let galley = ui.painter().layout(
                    text.to_string(),
                    font,
                    ui.visuals().text_color(),
                    (rect.width() - 12.0).max(10.0),
                );
                let text_pos = Pos2::new(rect.left() + 6.0, rect.top() + 6.0);
                ui.painter().add(Shape::galley(
                    text_pos,
                    galley,
                    ui.visuals().text_color(),
                ));
            }
        }
    }
}

/// Map a JSON Canvas `file` attribute (e.g. `notes/elegy-campaign-player.md`,
/// or a bare `elegy-campaign-player`) to a Digital Garden note id, which is
/// the filename stem. Accepts both with and without an `.md` extension.
fn file_to_note_id(file: &str) -> String {
    std::path::Path::new(file)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file)
        .to_string()
}

/// Render a text node's body through the full markdown parser inside a
/// sub-UI clipped to the node's inner rect. Wiki-link clicks are routed
/// into `clicked_file` so the caller can open the target note in the
/// Digital Garden alongside file-node clicks.
fn render_markdown_in_node(
    ui: &mut Ui,
    inner: Rect,
    text: &str,
    directory: &NoteDirectory,
    clicked_file: &mut Option<String>,
) {
    use crate::digital_garden::markdown_parser;

    let mut wiki_clicked: Option<String> = None;
    let mut on_link = |target: &str| {
        // The markdown renderer calls this for both external URLs and
        // wiki-links. Only wiki-links should map to "open in garden" —
        // external URLs are handled by egui's hyperlink_to automatically.
        if directory.resolve_link(target).is_some() {
            wiki_clicked = Some(target.to_string());
        }
    };
    // Task toggles inside a canvas node have no on-disk target (the
    // markdown lives in a JSON blob, not a .md file), so we ignore them.
    let mut on_task = |_idx: usize, _checked: bool| {};

    let builder = UiBuilder::new().max_rect(inner);
    ui.scope_builder(builder, |sub_ui| {
        sub_ui.set_clip_rect(inner);
        markdown_parser::render(sub_ui, text, directory, &mut on_link, &mut on_task);
    });

    if let Some(target) = wiki_clicked {
        // Normalise wiki-link target → canonical note id, same as file nodes.
        if let Some(note) = directory.resolve_link(&target) {
            *clicked_file = Some(note.id.clone());
        }
    }
}

/// Which side of `from_rect` is closest (by angle) to `to_center`?
/// Used when the canvas JSON doesn't specify an explicit `fromSide`/`toSide`.
fn infer_side(from_rect: Rect, to_center: Pos2) -> Side {
    let dx = to_center.x - from_rect.center().x;
    let dy = to_center.y - from_rect.center().y;
    if dx.abs() >= dy.abs() {
        if dx >= 0.0 {
            Side::Right
        } else {
            Side::Left
        }
    } else if dy >= 0.0 {
        Side::Bottom
    } else {
        Side::Top
    }
}
