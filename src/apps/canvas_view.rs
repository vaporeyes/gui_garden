// JSON Canvas (jsoncanvas.org / Obsidian) viewer.
//
// Loads a canvas JSON file from disk (native only) and renders its fixed-
// position rectangles and edges with pan + zoom. Reuses the astro-blog's
// canvas schema verbatim so any canvas authored there or in Obsidian can
// be dropped in.

use egui::{Color32, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use serde::Deserialize;
use std::path::{Path, PathBuf};

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
}

pub struct CanvasView {
    loaded: Option<(PathBuf, CanvasDocument)>,
    offset: Vec2,
    scale: f32,
    prev_mouse_pos: Option<Pos2>,
    error: Option<String>,
}

impl Default for CanvasView {
    fn default() -> Self {
        Self {
            loaded: None,
            offset: Vec2::ZERO,
            scale: 1.0,
            prev_mouse_pos: None,
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
                    self.offset = Vec2::ZERO;
                    self.scale = 1.0;
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

    /// Render the canvas. Returns the note id the user clicked on a
    /// `type: "file"` node, if any — the caller can use it to open that
    /// note in the Digital Garden window.
    pub fn ui(&mut self, ui: &mut Ui) -> Option<String> {
        let accent = palette::accent_now();
        let mut clicked_file: Option<String> = None;

        ui.horizontal(|ui| {
            if ui.button("📁 Load canvas…").clicked() {
                self.pick_and_load();
            }
            if self.loaded.is_some() && ui.button("Reset view").clicked() {
                self.offset = Vec2::ZERO;
                self.scale = 1.0;
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
            return clicked_file;
        };

        let available = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(available, Sense::click_and_drag());

        // Pan
        if response.dragged() {
            if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
                if let Some(prev) = self.prev_mouse_pos {
                    self.offset += mouse - prev;
                }
                self.prev_mouse_pos = Some(mouse);
            }
        } else {
            self.prev_mouse_pos = None;
        }
        // Zoom
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let old = self.scale;
                self.scale = (self.scale * (1.0 + scroll * 0.001)).clamp(0.1, 5.0);
                if let Some(hover) = response.hover_pos() {
                    let center = available.center().to_vec2();
                    let mouse_off = hover.to_vec2() - center - self.offset;
                    let scale_change = self.scale / old;
                    self.offset += mouse_off * (1.0 - scale_change);
                }
            }
        }

        let center = available.center().to_vec2();
        let project = |x: f32, y: f32| -> Pos2 {
            let v = center + (Vec2::new(x, y) + self.offset) * self.scale;
            Pos2::new(v.x, v.y)
        };

        let mut shapes: Vec<Shape> = Vec::new();

        // Draw edges first (behind nodes).
        let node_pos: std::collections::HashMap<&str, (Pos2, Pos2)> = doc
            .nodes
            .iter()
            .map(|n| {
                let tl = project(n.x, n.y);
                let br = project(n.x + n.width, n.y + n.height);
                (n.id.as_str(), (tl, br))
            })
            .collect();

        for edge in &doc.edges {
            let (Some(from_id), Some(to_id)) = (edge.from(), edge.to()) else {
                continue;
            };
            let (Some((from_tl, from_br)), Some((to_tl, to_br))) =
                (node_pos.get(from_id), node_pos.get(to_id))
            else {
                continue;
            };
            let from_center = Pos2::new(
                (from_tl.x + from_br.x) / 2.0,
                (from_tl.y + from_br.y) / 2.0,
            );
            let to_center =
                Pos2::new((to_tl.x + to_br.x) / 2.0, (to_tl.y + to_br.y) / 2.0);
            shapes.push(Shape::line_segment(
                [from_center, to_center],
                Stroke::new(1.5, accent.linear_multiply(0.7)),
            ));
        }

        // Draw nodes. `group`-type nodes act as background containers, so
        // paint them first — otherwise they cover any text/file nodes that
        // happen to appear later in the JSON list.
        let mut ordered: Vec<&CanvasNode> = doc.nodes.iter().collect();
        ordered.sort_by_key(|n| match n.node_type.as_str() {
            "group" => 0,
            _ => 1,
        });
        // Track file-node rects for post-loop click detection. We use the
        // painted order (same as `ordered`), then reverse-iterate so the
        // topmost-drawn node wins on overlap — matches visual expectation.
        let mut file_hit_rects: Vec<(Rect, String)> = Vec::new();

        for node in ordered {
            let tl = project(node.x, node.y);
            let br = project(node.x + node.width, node.y + node.height);
            let rect = Rect::from_two_pos(tl, br);

            let is_file = node.node_type == "file";
            let is_file_hovered = is_file
                && response
                    .hover_pos()
                    .map_or(false, |p| rect.contains(p));

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

            shapes.push(Shape::rect_filled(rect, 4.0, fill));
            shapes.push(Shape::rect_stroke(
                rect,
                4.0,
                Stroke::new(if is_file_hovered { 1.5 } else { 1.0 }, stroke_color),
                egui::StrokeKind::Outside,
            ));

            // File nodes are navigable — swap the cursor on hover and
            // remember the rect for click detection after the loop.
            if is_file {
                if let Some(file) = &node.file {
                    if is_file_hovered {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    file_hit_rects.push((rect, file.clone()));
                }
            }

            // Text content inside the node, if any.
            let content = node
                .text
                .as_deref()
                .or(node.label.as_deref())
                .or(node.file.as_deref())
                .or(node.url.as_deref());
            if let Some(text) = content {
                let font = egui::FontId::proportional(12.0 * self.scale.clamp(0.5, 1.5));
                let galley = ui.painter().layout(
                    text.to_string(),
                    font,
                    ui.visuals().text_color(),
                    (rect.width() - 12.0).max(10.0),
                );
                let text_pos = Pos2::new(rect.left() + 6.0, rect.top() + 6.0);
                shapes.push(Shape::galley(text_pos, galley, ui.visuals().text_color()));
            }
        }

        ui.painter().extend(shapes);

        // Click → open in the digital garden. Iterate the topmost-drawn
        // first so overlapping nodes resolve to the node the user sees.
        if response.clicked() {
            if let Some(hp) = response.hover_pos() {
                for (rect, file) in file_hit_rects.iter().rev() {
                    if rect.contains(hp) {
                        clicked_file = Some(file_to_note_id(file));
                        break;
                    }
                }
            }
        }

        clicked_file
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
