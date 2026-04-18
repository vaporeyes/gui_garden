use super::note_directory::NoteDirectory;
use crate::palette;
use egui::{Color32, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use std::collections::{HashMap, HashSet};
use std::ops::AddAssign;

/// Node in the graph
#[derive(Clone, Debug)]
pub struct GraphNode {
    /// ID of the note. Preserved as the canonical identity of the node even
    /// though the current minimal renderer doesn't read it directly —
    /// clicking a node still returns `connections` info keyed by id.
    #[allow(dead_code)]
    pub id: String,

    /// Title to display
    pub title: String,

    /// Position in the graph
    pub pos: Vec2,

    /// Connections to other nodes
    pub connections: HashSet<String>,
}

/// Graph view of notes and their connections
pub struct GraphView {
    /// Nodes in the graph
    pub nodes: HashMap<String, GraphNode>,

    /// Currently selected node
    pub selected_node: Option<String>,

    /// Currently hovered node
    pub hovered_node: Option<String>,

    /// View offset
    pub offset: Vec2,

    /// View scale
    pub scale: f32,

    /// Graph is being dragged
    pub dragging: bool,

    /// Previous mouse position for dragging
    pub prev_mouse_pos: Option<Pos2>,

    /// Remaining Fruchterman-Reingold iterations to run. Decremented by
    /// one per frame in `ui()` so the layout visibly settles instead of
    /// snapping into place at build time.
    layout_iters_remaining: usize,

    /// Current temperature. Cooled by `LAYOUT_COOLING` on every step.
    layout_temperature: f32,

    /// Ideal edge length derived from the graph area + node count.
    layout_k: f32,
}

const LAYOUT_AREA: f32 = 1_000_000.0;
const LAYOUT_TOTAL_ITERS: usize = 150;
const LAYOUT_COOLING: f32 = 0.95;

impl Default for GraphView {
    fn default() -> Self {
        Self {
            nodes: HashMap::new(),
            selected_node: None,
            hovered_node: None,
            offset: Vec2::new(0.0, 0.0),
            scale: 1.0,
            dragging: false,
            prev_mouse_pos: None,
            layout_iters_remaining: 0,
            layout_temperature: 0.0,
            layout_k: 0.0,
        }
    }
}

impl GraphView {
    /// Create a new graph view from a note directory
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the graph from scratch — drops all existing nodes and seeds
    /// every position fresh. Used on initial directory load.
    pub fn build_graph(&mut self, note_directory: &NoteDirectory) {
        self.nodes.clear();
        self.refresh_from_directory(note_directory);
    }

    /// Rebuild node data from the directory while preserving positions
    /// of nodes that already existed. Used by hot-reload: a content-only
    /// edit to an existing note shouldn't yank every node back to a random
    /// position and re-run 150 FR iterations.
    ///
    /// Link targets are canonicalized via `resolve_link` so wiki-links
    /// written in title or slug form correctly contribute edges.
    pub fn refresh_from_directory(&mut self, note_directory: &NoteDirectory) {
        // Snapshot current positions before rebuilding.
        let old_positions: HashMap<String, Vec2> = self
            .nodes
            .iter()
            .map(|(id, n)| (id.clone(), n.pos))
            .collect();
        let old_ids: HashSet<String> = old_positions.keys().cloned().collect();

        self.nodes.clear();

        for note in note_directory.published_notes() {
            let connections: HashSet<String> = note
                .links
                .iter()
                .filter_map(|link| {
                    note_directory
                        .resolve_link(&link.target_id)
                        .map(|n| n.id.clone())
                })
                .collect();

            // Reuse the old position for returning ids; drop newcomers at
            // random near the origin.
            let pos = old_positions
                .get(&note.id)
                .copied()
                .unwrap_or_else(random_position);

            self.nodes.insert(
                note.id.clone(),
                GraphNode {
                    id: note.id.clone(),
                    title: note.title(),
                    pos,
                    connections,
                },
            );
        }

        // Only re-layout when the *set* of ids changed. A content-only
        // edit (same ids, possibly new link targets) leaves nodes where
        // the user has already arranged them.
        let new_ids: HashSet<String> = self.nodes.keys().cloned().collect();
        if new_ids != old_ids && self.nodes.len() >= 2 {
            self.layout_k = (LAYOUT_AREA / self.nodes.len() as f32).sqrt();
            self.layout_temperature = LAYOUT_AREA.sqrt() / 10.0;
            self.layout_iters_remaining = LAYOUT_TOTAL_ITERS;
        }
    }

    /// Advance one Fruchterman-Reingold iteration. Returns `true` if more
    /// iterations remain after this call (the caller should `request_repaint`).
    fn layout_step(&mut self) -> bool {
        if self.layout_iters_remaining == 0 || self.nodes.len() < 2 {
            return false;
        }
        let k = self.layout_k;
        let temperature = self.layout_temperature;
        let ids: Vec<String> = self.nodes.keys().cloned().collect();

        let mut disp: HashMap<String, Vec2> =
            ids.iter().map(|id| (id.clone(), Vec2::ZERO)).collect();

        // Pairwise repulsion: f_r(d) = k² / d, along (u - v).
        for i in 0..ids.len() {
            for j in i + 1..ids.len() {
                let pi = self.nodes[&ids[i]].pos;
                let pj = self.nodes[&ids[j]].pos;
                let delta = pi - pj;
                let dist = delta.length().max(0.01);
                let magnitude = (k * k) / dist;
                let force = delta.normalized() * magnitude;
                disp.get_mut(&ids[i]).unwrap().add_assign(force);
                disp.get_mut(&ids[j]).unwrap().add_assign(-force);
            }
        }

        // Edge attraction: f_a(d) = d² / k, pulling endpoints together.
        for id in &ids {
            let node = &self.nodes[id];
            for conn in &node.connections {
                if conn == id {
                    continue;
                }
                let Some(other) = self.nodes.get(conn) else {
                    continue;
                };
                let delta = node.pos - other.pos;
                let dist = delta.length().max(0.01);
                let magnitude = (dist * dist) / k;
                let force = delta.normalized() * magnitude;
                disp.get_mut(id).unwrap().add_assign(-force);
                if let Some(d) = disp.get_mut(conn) {
                    d.add_assign(force);
                }
            }
        }

        // Clamp each displacement by the current temperature.
        for id in &ids {
            let d = disp[id];
            let dist = d.length();
            if dist > 0.0 {
                let scale = dist.min(temperature) / dist;
                self.nodes.get_mut(id).unwrap().pos += d * scale;
            }
        }

        self.layout_temperature *= LAYOUT_COOLING;
        self.layout_iters_remaining -= 1;
        self.layout_iters_remaining > 0
    }

    /// Show the graph view. `_note_directory` is reserved for future use
    /// (e.g. hovering a node to preview its title / tags); the current
    /// renderer works purely from `self.nodes` which was already populated
    /// from the directory at build time.
    pub fn ui(&mut self, ui: &mut Ui, note_directory: &NoteDirectory) -> Option<String> {
        // Run one FR iteration per frame until the queue drains. Keeps the
        // UI responsive and lets the user watch the graph relax into shape.
        if self.layout_step() {
            ui.ctx().request_repaint();
        }

        let available_rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(available_rect, Sense::click_and_drag());

        let mut clicked_node = None;

        // Handle interactions
        if response.dragged() {
            if let Some(mouse_pos) = ui.input(|i| i.pointer.interact_pos()) {
                if let Some(prev_pos) = self.prev_mouse_pos {
                    self.offset += mouse_pos - prev_pos;
                }
                self.prev_mouse_pos = Some(mouse_pos);
                self.dragging = true;
            }
        } else {
            self.prev_mouse_pos = None;
            self.dragging = false;
        }

        // Handle zooming
        if let Some(hover_pos) = response.hover_pos() {
            let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_delta != 0.0 {
                let old_scale = self.scale;
                self.scale = (self.scale * (1.0 + scroll_delta * 0.001)).clamp(0.1, 5.0);

                // Adjust offset to zoom toward mouse position
                let zoom_center = hover_pos.to_vec2();
                let screen_center = available_rect.center().to_vec2();
                let mouse_offset = zoom_center - screen_center - self.offset;
                let scale_change = self.scale / old_scale;
                self.offset += mouse_offset * (1.0 - scale_change);
            }
        }

        let center = available_rect.center().to_vec2();

        // Draw connections first (so they're behind nodes)
        let mut shapes = Vec::new();

        for (node_id, node) in &self.nodes {
            let node_pos = center + (node.pos + self.offset) * self.scale;

            for conn_id in &node.connections {
                if let Some(conn_node) = self.nodes.get(conn_id) {
                    let conn_pos = center + (conn_node.pos + self.offset) * self.scale;

                    // Skip self-connections
                    if node_id == conn_id {
                        continue;
                    }

                    // Determine color based on selection/hover state
                    let color = if self.selected_node.as_ref() == Some(node_id)
                        || self.hovered_node.as_ref() == Some(node_id)
                        || self.selected_node.as_ref() == Some(conn_id)
                        || self.hovered_node.as_ref() == Some(conn_id)
                    {
                        Color32::from_rgb(200, 200, 100)
                    } else {
                        Color32::from_rgb(100, 100, 100)
                    };

                    shapes.push(Shape::line_segment(
                        [
                            Pos2::new(node_pos.x, node_pos.y),
                            Pos2::new(conn_pos.x, conn_pos.y),
                        ],
                        Stroke::new(1.0, color),
                    ));
                }
            }
        }

        // Draw nodes
        self.hovered_node = None;

        for (node_id, node) in &self.nodes {
            let node_pos = center + (node.pos + self.offset) * self.scale;
            let node_radius = 5.0 * self.scale;
            let node_rect = Rect::from_center_size(
                Pos2::new(node_pos.x, node_pos.y),
                Vec2::new(node_radius * 2.0, node_radius * 2.0),
            );

            let accent = palette::accent_now();
            let is_selected = self.selected_node.as_ref() == Some(node_id);
            let node_color = if is_selected {
                accent
            } else {
                // Deeper/dimmer version of the accent for unselected nodes.
                accent.linear_multiply(0.55)
            };

            shapes.push(Shape::circle_filled(
                Pos2::new(node_pos.x, node_pos.y),
                node_radius,
                node_color,
            ));

            // Check for hover
            if response
                .hover_pos()
                .map_or(false, |p| node_rect.contains(p))
            {
                self.hovered_node = Some(node_id.clone());

                // Preview card: title in accent, followed by the first
                // ~160 chars of the note body, wrapped to a reasonable
                // width. Previously we showed only a one-line title —
                // this turns hover into an actual at-a-glance preview.
                let title_font = egui::FontId::proportional(14.0);
                let body_font = egui::FontId::proportional(11.0);
                let preview_text = note_directory
                    .get_note(node_id)
                    .map(|n| first_paragraph(&n.content, 160))
                    .unwrap_or_default();

                let accent = palette::accent_now();
                let title_galley = ui.painter().layout_no_wrap(
                    node.title.clone(),
                    title_font,
                    accent,
                );
                let max_width = (title_galley.size().x + 40.0).max(240.0);
                let body_galley = if preview_text.is_empty() {
                    None
                } else {
                    Some(ui.painter().layout(
                        preview_text,
                        body_font,
                        Color32::from_rgb(220, 220, 220),
                        max_width,
                    ))
                };

                let body_h = body_galley
                    .as_ref()
                    .map(|g| g.size().y + 6.0)
                    .unwrap_or(0.0);
                let card_size = egui::vec2(
                    max_width + 12.0,
                    title_galley.size().y + body_h + 12.0,
                );
                let card_rect = Rect::from_min_size(
                    Pos2::new(
                        node_pos.x - card_size.x / 2.0,
                        node_pos.y - node_radius - card_size.y - 6.0,
                    ),
                    card_size,
                );
                shapes.push(Shape::rect_filled(
                    card_rect,
                    4.0,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 220),
                ));
                shapes.push(Shape::rect_stroke(
                    card_rect,
                    4.0,
                    Stroke::new(1.0, accent.linear_multiply(0.5)),
                    egui::StrokeKind::Outside,
                ));
                shapes.push(Shape::galley(
                    card_rect.min + egui::vec2(6.0, 6.0),
                    title_galley,
                    accent,
                ));
                if let Some(g) = body_galley {
                    shapes.push(Shape::galley(
                        card_rect.min + egui::vec2(6.0, 6.0 + body_h - g.size().y),
                        g,
                        Color32::from_rgb(220, 220, 220),
                    ));
                }

                // Check for click
                if response.clicked() && node_rect.contains(response.hover_pos().unwrap()) {
                    self.selected_node = Some(node_id.clone());
                    clicked_node = Some(node_id.clone());
                }
            }
        }

        // Paint all shapes
        ui.painter().extend(shapes);

        clicked_node
    }
}

/// Generate a random position for a node
fn random_position() -> Vec2 {
    let x = (rand::random::<f32>() - 0.5) * 400.0;
    let y = (rand::random::<f32>() - 0.5) * 400.0;
    Vec2::new(x, y)
}

/// Pull the first non-frontmatter paragraph from a markdown string, up to
/// `max_chars`. Used by the graph's hover preview. Skips YAML frontmatter
/// and any leading blank lines, and trims trailing whitespace / `…` so the
/// preview reads cleanly.
fn first_paragraph(content: &str, max_chars: usize) -> String {
    // Skip YAML frontmatter block if present.
    let body = if let Some(stripped) = content.strip_prefix("---\n") {
        stripped
            .find("\n---\n")
            .or_else(|| stripped.find("\n---\r\n"))
            .map(|end| &stripped[end + 5..])
            .unwrap_or(content)
    } else {
        content
    };

    let mut out = String::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !out.is_empty() {
                break; // blank line after content → paragraph end
            }
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(trimmed);
        if out.chars().count() >= max_chars {
            break;
        }
    }
    // Clamp to max_chars on char boundary, appending an ellipsis if we cut.
    if out.chars().count() > max_chars {
        let truncated: String = out.chars().take(max_chars).collect();
        format!("{}…", truncated.trim_end())
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_paragraph_strips_frontmatter() {
        let content = "---\ntitle: X\n---\n\nHello world.\n\nSecond paragraph.";
        assert_eq!(first_paragraph(content, 200), "Hello world.");
    }

    #[test]
    fn first_paragraph_truncates_long_content() {
        let content = "x".repeat(500);
        let preview = first_paragraph(&content, 50);
        assert!(preview.chars().count() <= 51); // 50 + ellipsis
        assert!(preview.ends_with('…'));
    }

    #[test]
    fn first_paragraph_handles_no_frontmatter() {
        assert_eq!(first_paragraph("Just a line.", 200), "Just a line.");
    }

    #[test]
    fn first_paragraph_joins_wrapped_lines_within_paragraph() {
        let content = "wrapped line one\nwrapped line two\n\nsecond para";
        assert_eq!(
            first_paragraph(content, 200),
            "wrapped line one wrapped line two"
        );
    }
}
