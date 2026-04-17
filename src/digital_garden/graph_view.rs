use super::note::Note;
use super::note_directory::NoteDirectory;
use egui::{Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2};
use std::collections::{HashMap, HashSet};
use std::ops::AddAssign;
use std::sync::Arc;

/// Node in the graph
#[derive(Clone, Debug)]
pub struct GraphNode {
    /// ID of the note
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
}

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
        }
    }
}

impl GraphView {
    /// Create a new graph view from a note directory
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the graph from a note directory
    pub fn build_graph(&mut self, note_directory: &NoteDirectory) {
        self.nodes.clear();

        // Create nodes for all published notes
        for note in note_directory.published_notes() {
            let connections: HashSet<String> = note
                .links
                .iter()
                .map(|link| link.target_id.clone())
                .collect();

            self.nodes.insert(
                note.id.clone(),
                GraphNode {
                    id: note.id.clone(),
                    title: note.title(),
                    pos: random_position(), // Random initial position
                    connections,
                },
            );
        }

        // Run a simple force-directed layout algorithm
        self.layout_graph();
    }

    /// Update the graph with a simple force-directed layout
    pub fn layout_graph(&mut self) {
        // Simple layout algorithm - in a real implementation, we'd use a more sophisticated approach
        const REPULSION: f32 = 500.0;
        const ATTRACTION: f32 = 0.05;
        const MAX_ITERATIONS: usize = 100;

        // Copy node IDs for iteration
        let node_ids: Vec<String> = self.nodes.keys().cloned().collect();

        // Run layout iterations
        for _ in 0..MAX_ITERATIONS {
            // Calculate forces
            let mut forces: HashMap<String, Vec2> = HashMap::new();

            // Repulsive forces between all nodes
            for i in 0..node_ids.len() {
                let node_id_1 = &node_ids[i];
                let node_1 = &self.nodes[node_id_1];

                for j in i + 1..node_ids.len() {
                    let node_id_2 = &node_ids[j];
                    let node_2 = &self.nodes[node_id_2];

                    let delta = node_1.pos - node_2.pos;
                    let distance = delta.length().max(0.1); // Avoid division by zero
                    let force = delta.normalized() * REPULSION / distance.powi(2);

                    forces
                        .entry(node_id_1.clone())
                        .or_insert_with(|| Vec2::ZERO)
                        .add_assign(force);

                    forces
                        .entry(node_id_2.clone())
                        .or_insert_with(|| Vec2::ZERO)
                        .add_assign(-force);
                }
            }

            // Attractive forces along edges
            for (node_id, node) in &self.nodes {
                for conn_id in &node.connections {
                    if let Some(conn_node) = self.nodes.get(conn_id) {
                        let delta = conn_node.pos - node.pos;
                        let force = delta * ATTRACTION;

                        forces
                            .entry(node_id.clone())
                            .or_insert_with(|| Vec2::ZERO)
                            .add_assign(force);
                    }
                }
            }

            // Apply forces to update positions
            for (node_id, force) in forces {
                if let Some(node) = self.nodes.get_mut(&node_id) {
                    node.pos += force;
                }
            }
        }
    }

    /// Show the graph view
    pub fn ui(&mut self, ui: &mut Ui, note_directory: &NoteDirectory) -> Option<String> {
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

            let is_selected = self.selected_node.as_ref() == Some(node_id);
            let node_color = if is_selected {
                Color32::from_rgb(255, 200, 0)
            } else {
                Color32::from_rgb(100, 150, 250)
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

                // Draw label on hover
                let font_id = egui::FontId::proportional(14.0);
                let galley =
                    ui.painter()
                        .layout_no_wrap(node.title.clone(), font_id, Color32::WHITE);

                let label_rect = Rect::from_min_size(
                    Pos2::new(
                        node_pos.x - galley.size().x / 2.0,
                        node_pos.y - node_radius - galley.size().y - 4.0,
                    ),
                    galley.size(),
                );

                shapes.push(Shape::rect_filled(
                    label_rect.expand(4.0),
                    4.0,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 180),
                ));

                shapes.push(Shape::galley(label_rect.min, galley, Color32::WHITE));

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
