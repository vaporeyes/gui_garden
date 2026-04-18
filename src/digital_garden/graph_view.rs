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

    /// Node currently being dragged by the mouse, if any. While `Some`,
    /// drags move just this node; empty-space drags still pan.
    dragged_node: Option<String>,
}

const LAYOUT_AREA: f32 = 1_000_000.0;
const LAYOUT_TOTAL_ITERS: usize = 150;
const LAYOUT_COOLING: f32 = 0.95;
/// Base node radius in graph units (screen radius = this × `scale`).
const NODE_RADIUS: f32 = 5.0;
/// Scale thresholds at which always-on labels fade in/out. Between
/// these values the opacity ramps linearly; below the min, labels are
/// invisible so a zoomed-out overview stays uncluttered.
const LABEL_FADE_IN_SCALE: f32 = 0.75;
const LABEL_FADE_FULL_SCALE: f32 = 1.25;
/// Temperature floor applied while the user is dragging a node, so the
/// rest of the graph reacts instead of sitting frozen.
const DRAG_TEMPERATURE: f32 = 50.0;
/// How many FR iterations to keep queued while a drag is active.
const DRAG_ITERS: usize = 6;

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
            dragged_node: None,
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
    ///
    /// `pinned` is the id of a node the user is actively dragging, if any —
    /// its displacement is computed (so neighbours still get pulled toward
    /// it) but not applied, keeping the drag handle under the cursor.
    fn layout_step(&mut self, pinned: Option<&str>) -> bool {
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

        // Clamp each displacement by the current temperature. The pinned
        // node skips this step — its position is set by the mouse each
        // frame, so physics shouldn't move it.
        for id in &ids {
            if Some(id.as_str()) == pinned {
                continue;
            }
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
        let pinned = self.dragged_node.clone();
        if self.layout_step(pinned.as_deref()) {
            ui.ctx().request_repaint();
        }

        let available_rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(available_rect, Sense::click_and_drag());
        let center = available_rect.center().to_vec2();

        let mut clicked_node = None;

        // ---- Drag interaction ----
        //
        // Node drag vs. canvas pan is decided on `drag_started`: if the
        // pointer is over a node at the moment the drag begins, we latch
        // onto that node for the duration; otherwise we pan.
        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.dragged_node = self.node_at_screen_pos(pos, center);
                self.prev_mouse_pos = Some(pos);
            }
        }
        if response.drag_stopped() {
            self.dragged_node = None;
            self.prev_mouse_pos = None;
        }

        if response.dragged() {
            if let Some(mouse_pos) = ui.input(|i| i.pointer.interact_pos()) {
                match self.dragged_node.clone() {
                    Some(id) => {
                        // Pin the dragged node to the cursor. Convert from
                        // screen space back to graph space first.
                        let graph_pos = self.screen_to_graph(mouse_pos, center);
                        if let Some(node) = self.nodes.get_mut(&id) {
                            node.pos = graph_pos;
                        }
                        // Re-energize the layout so neighbours react.
                        self.layout_temperature = self.layout_temperature.max(DRAG_TEMPERATURE);
                        if self.layout_iters_remaining < DRAG_ITERS {
                            self.layout_iters_remaining = DRAG_ITERS;
                        }
                        ui.ctx().request_repaint();
                    }
                    None => {
                        // Pan the whole canvas.
                        if let Some(prev) = self.prev_mouse_pos {
                            self.offset += mouse_pos - prev;
                        }
                    }
                }
                self.prev_mouse_pos = Some(mouse_pos);
                self.dragging = true;
            }
        } else {
            self.prev_mouse_pos = None;
            self.dragging = false;
        }

        // ---- Zoom ----
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

        // ---- Edges ----
        let mut shapes = Vec::new();
        let node_radius = NODE_RADIUS * self.scale;
        let arrow_size = (5.0 * self.scale).clamp(3.0, 8.0);

        for (node_id, node) in &self.nodes {
            let src_pos = Pos2::new(
                center.x + (node.pos.x + self.offset.x) * self.scale,
                center.y + (node.pos.y + self.offset.y) * self.scale,
            );

            for conn_id in &node.connections {
                if node_id == conn_id {
                    continue;
                }
                let Some(conn_node) = self.nodes.get(conn_id) else {
                    continue;
                };
                let dst_pos = Pos2::new(
                    center.x + (conn_node.pos.x + self.offset.x) * self.scale,
                    center.y + (conn_node.pos.y + self.offset.y) * self.scale,
                );

                let highlighted = self.selected_node.as_deref() == Some(node_id)
                    || self.hovered_node.as_deref() == Some(node_id)
                    || self.selected_node.as_deref() == Some(conn_id)
                    || self.hovered_node.as_deref() == Some(conn_id);
                let color = if highlighted {
                    Color32::from_rgb(200, 200, 100)
                } else {
                    Color32::from_rgb(100, 100, 100)
                };

                draw_directed_edge(
                    &mut shapes,
                    src_pos,
                    dst_pos,
                    node_radius,
                    color,
                    1.0,
                    arrow_size,
                );
            }
        }

        // ---- Nodes + labels ----
        self.hovered_node = None;

        let label_alpha =
            ((self.scale - LABEL_FADE_IN_SCALE) / (LABEL_FADE_FULL_SCALE - LABEL_FADE_IN_SCALE))
                .clamp(0.0, 1.0);

        for (node_id, node) in &self.nodes {
            let node_pos = Pos2::new(
                center.x + (node.pos.x + self.offset.x) * self.scale,
                center.y + (node.pos.y + self.offset.y) * self.scale,
            );
            let node_rect = Rect::from_center_size(
                node_pos,
                Vec2::new(node_radius * 2.0, node_radius * 2.0),
            );

            let accent = palette::accent_now();
            let is_selected = self.selected_node.as_ref() == Some(node_id);
            let is_dragged = self.dragged_node.as_deref() == Some(node_id.as_str());
            let node_color = if is_selected || is_dragged {
                accent
            } else {
                // Deeper/dimmer version of the accent for unselected nodes.
                accent.linear_multiply(0.55)
            };

            shapes.push(Shape::circle_filled(node_pos, node_radius, node_color));

            // Zoom-aware always-on label. Fades in between LABEL_FADE_IN
            // and LABEL_FADE_FULL scales so a wide-out overview stays clean.
            if label_alpha > 0.01 {
                let alpha_u8 = (label_alpha * 200.0) as u8;
                let label_color = Color32::from_rgba_unmultiplied(220, 220, 220, alpha_u8);
                let font = egui::FontId::proportional((11.0 * self.scale).clamp(10.0, 16.0));
                let galley =
                    ui.painter().layout_no_wrap(node.title.clone(), font, label_color);
                let label_pos = Pos2::new(
                    node_pos.x + node_radius + 6.0,
                    node_pos.y - galley.size().y / 2.0,
                );
                shapes.push(Shape::galley(label_pos, galley, label_color));
            }

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

    /// Pointer-hit-test: which node, if any, sits under screen position `p`?
    /// Uses the current `scale` + `offset` to map each node's graph-space
    /// position into screen coordinates and does a circular distance check.
    fn node_at_screen_pos(&self, p: Pos2, center: Vec2) -> Option<String> {
        let r = NODE_RADIUS * self.scale;
        for (id, node) in &self.nodes {
            let screen = Pos2::new(
                center.x + (node.pos.x + self.offset.x) * self.scale,
                center.y + (node.pos.y + self.offset.y) * self.scale,
            );
            if (screen - p).length() <= r + 2.0 {
                // +2px slop — easier to grab a small node at low zoom.
                return Some(id.clone());
            }
        }
        None
    }

    /// Inverse of the forward projection used everywhere else:
    ///   screen = center + (graph + offset) * scale
    /// ⇒ graph  = (screen − center) / scale − offset
    fn screen_to_graph(&self, p: Pos2, center: Vec2) -> Vec2 {
        (p.to_vec2() - center) / self.scale - self.offset
    }
}

/// Draw an edge with a small triangular arrowhead at the destination end.
/// The line is shortened by the node radius on both ends so neither its
/// start nor its tip disappears inside the endpoint circles.
fn draw_directed_edge(
    shapes: &mut Vec<Shape>,
    from: Pos2,
    to: Pos2,
    node_radius: f32,
    color: Color32,
    stroke_width: f32,
    arrow_size: f32,
) {
    let delta = to - from;
    let dist = delta.length();
    // If the endpoints overlap or are very close, skip — there's nothing
    // sensible to draw and arrow math would blow up.
    if dist < node_radius * 2.0 + 1.0 {
        return;
    }
    let dir = delta / dist;
    let perp = Vec2::new(-dir.y, dir.x);
    let line_start = from + dir * node_radius;
    let line_end = to - dir * node_radius;

    shapes.push(Shape::line_segment(
        [line_start, line_end],
        Stroke::new(stroke_width, color),
    ));

    // Arrowhead triangle pointing toward the destination.
    let tip = line_end;
    let back = tip - dir * arrow_size;
    let left = back + perp * arrow_size * 0.5;
    let right = back - perp * arrow_size * 0.5;
    shapes.push(Shape::convex_polygon(
        vec![tip, left, right],
        color,
        Stroke::NONE,
    ));
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
