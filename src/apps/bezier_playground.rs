// ABOUTME: Bézier curve playground - drag control points to shape the
// ABOUTME: curve, switch quadratic/cubic, tweak strokes & fill live.
//
// Adapted from `egui_demo_lib::demo::paint_bezier` with the toolbar
// reflowed to match the rest of the garden's mini-apps and the theme
// accent threaded through the default colors.

use egui::epaint::{CubicBezierShape, PathShape, QuadraticBezierShape, RectShape};
use egui::{
    emath, pos2, Color32, Frame, Grid, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2,
};

use crate::palette;

pub struct BezierPlayground {
    /// Curve degree: 3 = quadratic, 4 = cubic. Controls how many of the
    /// four `control_points` are actually rendered.
    degree: usize,
    control_points: [Pos2; 4],
    stroke: Stroke,
    fill: Color32,
    aux_stroke: Stroke,
    bounding_box_stroke: Stroke,
}

impl Default for BezierPlayground {
    fn default() -> Self {
        let accent = palette::accent_now();
        Self {
            degree: 4,
            control_points: [
                pos2(50.0, 50.0),
                pos2(60.0, 250.0),
                pos2(200.0, 200.0),
                pos2(250.0, 50.0),
            ],
            stroke: Stroke::new(1.5, accent),
            fill: accent.linear_multiply(0.20),
            aux_stroke: Stroke::new(1.0, Color32::from_gray(180).linear_multiply(0.4)),
            bounding_box_stroke: Stroke::new(0.5, Color32::from_gray(120).linear_multiply(0.4)),
        }
    }
}

impl BezierPlayground {
    pub fn ui(&mut self, ui: &mut Ui) {
        self.ui_controls(ui);
        ui.separator();
        Frame::canvas(ui.style()).show(ui, |ui| {
            self.ui_canvas(ui);
        });
    }

    fn ui_controls(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.degree, 3, "Quadratic");
            ui.radio_value(&mut self.degree, 4, "Cubic");
            ui.separator();
            if ui.button("Reset").clicked() {
                *self = Self::default();
            }
        });
        ui.collapsing("Colors & strokes", |ui| {
            Grid::new("bezier_colors")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Fill");
                    ui.color_edit_button_srgba(&mut self.fill);
                    ui.end_row();

                    ui.label("Curve");
                    ui.add(&mut self.stroke);
                    ui.end_row();

                    ui.label("Aux line");
                    ui.add(&mut self.aux_stroke);
                    ui.end_row();

                    ui.label("Bounding box");
                    ui.add(&mut self.bounding_box_stroke);
                    ui.end_row();
                });
        });
        ui.small("Drag the control points to reshape the curve.");
    }

    fn ui_canvas(&mut self, ui: &mut Ui) -> egui::Response {
        let (response, painter) = ui
            .allocate_painter(Vec2::new(ui.available_width(), 320.0), Sense::hover());

        // Map our `(0..w, 0..h)` "logical" control-point coords into the
        // painter's screen rect so the curve fills the available canvas.
        let to_screen = emath::RectTransform::from_to(
            Rect::from_min_size(Pos2::ZERO, response.rect.size()),
            response.rect,
        );

        let control_point_radius = 7.0;

        // Per-handle interaction: each control point gets its own draggable
        // hit region, scoped to a unique id derived from the painter's id.
        let handle_shapes: Vec<Shape> = self
            .control_points
            .iter_mut()
            .enumerate()
            .take(self.degree)
            .map(|(i, point)| {
                let size = Vec2::splat(2.0 * control_point_radius);
                let point_in_screen = to_screen.transform_pos(*point);
                let point_rect = Rect::from_center_size(point_in_screen, size);
                let point_id = response.id.with(i);
                let point_response = ui.interact(point_rect, point_id, Sense::drag());

                *point += point_response.drag_delta();
                *point = to_screen.from().clamp(*point);

                let point_in_screen = to_screen.transform_pos(*point);
                let stroke = ui.style().interact(&point_response).fg_stroke;
                Shape::circle_stroke(point_in_screen, control_point_radius, stroke)
            })
            .collect();

        let points_in_screen: Vec<Pos2> = self
            .control_points
            .iter()
            .take(self.degree)
            .map(|p| to_screen * *p)
            .collect();

        match self.degree {
            3 => {
                let points = points_in_screen.clone().try_into().unwrap();
                let shape = QuadraticBezierShape::from_points_stroke(
                    points,
                    true,
                    self.fill,
                    self.stroke,
                );
                painter.add(RectShape::stroke(
                    shape.visual_bounding_rect(),
                    0.0,
                    self.bounding_box_stroke,
                    StrokeKind::Outside,
                ));
                painter.add(shape);
            }
            4 => {
                let points = points_in_screen.clone().try_into().unwrap();
                let shape =
                    CubicBezierShape::from_points_stroke(points, true, self.fill, self.stroke);
                painter.add(RectShape::stroke(
                    shape.visual_bounding_rect(),
                    0.0,
                    self.bounding_box_stroke,
                    StrokeKind::Outside,
                ));
                painter.add(shape);
            }
            _ => {
                // `degree` is only mutated through the radio buttons above
                // which are constrained to {3, 4}.
                unreachable!("degree must be 3 or 4");
            }
        }

        painter.add(PathShape::line(points_in_screen, self.aux_stroke));
        painter.extend(handle_shapes);

        response
    }
}
