// ABOUTME: Wolfenstein 3D-style first-person raycaster mini-app.
// ABOUTME: WASD/arrows to move + turn, Q/E to strafe. Pure Rust, no WAD.
//
// Renders a 320x200 framebuffer into an egui texture each frame using
// the classic DDA grid-walk raycasting algorithm. The framebuffer is
// uploaded with NEAREST filtering so the chunky pixels read as
// intentional retro rather than blurred.
//
// Walls are flat-colored with two cheap shading effects: a darker tint
// on N/S faces (so 90-degree corners read in 3D) and an inverse-square
// fog falloff so distant walls fade into the ceiling. Wall color #2 is
// pulled from the active Poline scheme, so switching the color scheme
// (Cmd+K → "Tide" / "Forest" / etc.) repaints the level.

use egui::{Color32, ColorImage, Key, TextureHandle, TextureOptions, Ui};
use std::f32::consts::FRAC_PI_3;

use crate::palette;

const MAP_W: usize = 16;
const MAP_H: usize = 16;
const FB_W: usize = 320;
const FB_H: usize = 200;
/// 60° horizontal field of view — matches the doom default.
const FOV: f32 = FRAC_PI_3;
/// Hard cap on grid steps before we declare the ray "lost" and skip
/// drawing a wall slice. Bounds worst-case cost per column.
const MAX_RAY_STEPS: usize = 64;

/// Test level. 0 = empty, 1 = standard wall, 2 = accent-colored wall.
/// Borders are walls so the player can't walk off the map.
const MAP: [[u8; MAP_W]; MAP_H] = [
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 1, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1, 1, 0, 1],
    [1, 0, 1, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 1],
    [1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1],
    [1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 2, 0, 0, 2, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 2, 2, 0, 0, 2, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 1],
    [1, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 0, 0, 0, 1],
    [1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
];

pub struct Raycaster {
    /// Player position in grid units. (1.5, 1.5) = center of the
    /// bottom-left interior cell, which is always empty by map design.
    px: f32,
    py: f32,
    /// View angle in radians, 0 = +x (east), CCW positive.
    angle: f32,
    /// CPU framebuffer; uploaded to a `TextureHandle` once per frame.
    framebuffer: Vec<Color32>,
    texture: Option<TextureHandle>,
    paused: bool,
    /// Field captured to size the displayed image; defaults match
    /// the framebuffer aspect.
    show_minimap: bool,
}

impl Default for Raycaster {
    fn default() -> Self {
        Self {
            px: 1.5,
            py: 1.5,
            angle: 0.0,
            framebuffer: vec![Color32::BLACK; FB_W * FB_H],
            texture: None,
            paused: false,
            show_minimap: true,
        }
    }
}

impl Raycaster {
    pub fn ui(&mut self, ui: &mut Ui) {
        // Top toolbar — keep tight so most of the window goes to the view.
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.paused, "Paused");
            ui.checkbox(&mut self.show_minimap, "Minimap");
            if ui.button("Reset").clicked() {
                self.px = 1.5;
                self.py = 1.5;
                self.angle = 0.0;
            }
            ui.separator();
            ui.label(
                egui::RichText::new("WASD / arrows  ·  Q/E strafe")
                    .small()
                    .weak(),
            );
        });

        let dt = ui.input(|i| i.stable_dt).clamp(0.0, 0.1);
        if !self.paused {
            self.handle_input(ui, dt);
        }
        let accent = palette::accent_now();
        self.render(accent);

        // Upload framebuffer to a persistent texture and draw it.
        let image = ColorImage::new([FB_W, FB_H], self.framebuffer.clone());
        let texture: &mut TextureHandle = match self.texture.as_mut() {
            Some(t) => {
                t.set(image, TextureOptions::NEAREST);
                t
            }
            None => {
                self.texture = Some(ui.ctx().load_texture(
                    "raycaster_fb",
                    image,
                    TextureOptions::NEAREST,
                ));
                self.texture.as_mut().expect("just inserted")
            }
        };

        // Fit the framebuffer into available space at integer scale-ish
        // proportions. We compute the largest rect that preserves the
        // 320:200 aspect ratio.
        let avail = ui.available_size();
        let aspect = FB_W as f32 / FB_H as f32;
        let mut w = avail.x;
        let mut h = w / aspect;
        if h > avail.y {
            h = avail.y;
            w = h * aspect;
        }
        let response = ui.add(
            egui::Image::from_texture(&*texture)
                .fit_to_exact_size(egui::vec2(w, h)),
        );

        // Optional top-down minimap overlay drawn at the corner of the
        // image rect so the player can orient themselves.
        if self.show_minimap {
            self.draw_minimap(ui, response.rect, accent);
        }

        // Repaint while running so the loop is continuous; pausing
        // halts the request, which lets the rest of the app idle.
        if !self.paused {
            ui.ctx().request_repaint();
        }
    }

    fn handle_input(&mut self, ui: &Ui, dt: f32) {
        const MOVE_SPEED: f32 = 3.0; // grid units per second
        const ROT_SPEED: f32 = 2.5;  // radians per second
        let (fwd, bwd, turn_l, turn_r, strafe_l, strafe_r) = ui.input(|i| {
            (
                i.key_down(Key::W) || i.key_down(Key::ArrowUp),
                i.key_down(Key::S) || i.key_down(Key::ArrowDown),
                i.key_down(Key::A) || i.key_down(Key::ArrowLeft),
                i.key_down(Key::D) || i.key_down(Key::ArrowRight),
                i.key_down(Key::Q),
                i.key_down(Key::E),
            )
        });

        if turn_l {
            self.angle -= ROT_SPEED * dt;
        }
        if turn_r {
            self.angle += ROT_SPEED * dt;
        }

        let cos = self.angle.cos();
        let sin = self.angle.sin();
        let mut dx = 0.0_f32;
        let mut dy = 0.0_f32;
        if fwd {
            dx += cos;
            dy += sin;
        }
        if bwd {
            dx -= cos;
            dy -= sin;
        }
        // Strafe = perpendicular to facing.
        if strafe_l {
            dx += sin;
            dy -= cos;
        }
        if strafe_r {
            dx -= sin;
            dy += cos;
        }

        let len = (dx * dx + dy * dy).sqrt();
        if len > 1e-6 {
            dx /= len;
            dy /= len;
            let step = MOVE_SPEED * dt;
            // Wall-sliding collision: try each axis independently so
            // running diagonally into a wall slides along it.
            let nx = self.px + dx * step;
            if !is_wall(nx, self.py) {
                self.px = nx;
            }
            let ny = self.py + dy * step;
            if !is_wall(self.px, ny) {
                self.py = ny;
            }
        }
    }

    fn render(&mut self, accent: Color32) {
        // Sky / ground fill — solid colors are cheap and read as 3D
        // once walls are drawn over them.
        let ceil = Color32::from_rgb(35, 38, 48);
        let floor = Color32::from_rgb(60, 52, 44);
        for y in 0..FB_H {
            let color = if y < FB_H / 2 { ceil } else { floor };
            let row = y * FB_W;
            for x in 0..FB_W {
                self.framebuffer[row + x] = color;
            }
        }

        // Per-column raycast.
        for col in 0..FB_W {
            // camera_x in [-1, 1] across the screen width.
            let camera_x = 2.0 * col as f32 / FB_W as f32 - 1.0;
            let ray_angle = self.angle + camera_x * (FOV * 0.5);
            let rcos = ray_angle.cos();
            let rsin = ray_angle.sin();

            let mut map_x = self.px as i32;
            let mut map_y = self.py as i32;

            // Distance the ray covers for a one-unit step in x or y.
            // Guard against division near zero — picks a sentinel that
            // makes that axis lose every comparison instead.
            let delta_x = if rcos.abs() < 1e-6 {
                f32::INFINITY
            } else {
                (1.0 / rcos).abs()
            };
            let delta_y = if rsin.abs() < 1e-6 {
                f32::INFINITY
            } else {
                (1.0 / rsin).abs()
            };

            let (step_x, mut side_x) = if rcos < 0.0 {
                (-1, (self.px - map_x as f32) * delta_x)
            } else {
                (1, ((map_x as f32 + 1.0) - self.px) * delta_x)
            };
            let (step_y, mut side_y) = if rsin < 0.0 {
                (-1, (self.py - map_y as f32) * delta_y)
            } else {
                (1, ((map_y as f32 + 1.0) - self.py) * delta_y)
            };

            let mut hit_side = 0; // 0 = vertical (E/W face), 1 = horizontal (N/S face)
            let mut hit_cell = 0_u8;
            for _ in 0..MAX_RAY_STEPS {
                if side_x < side_y {
                    side_x += delta_x;
                    map_x += step_x;
                    hit_side = 0;
                } else {
                    side_y += delta_y;
                    map_y += step_y;
                    hit_side = 1;
                }
                if map_x < 0
                    || map_y < 0
                    || map_x >= MAP_W as i32
                    || map_y >= MAP_H as i32
                {
                    break;
                }
                let c = MAP[map_y as usize][map_x as usize];
                if c != 0 {
                    hit_cell = c;
                    break;
                }
            }
            if hit_cell == 0 {
                continue;
            }

            // Perpendicular distance to the wall — using the camera
            // plane projection to remove fisheye.
            let perp = if hit_side == 0 {
                side_x - delta_x
            } else {
                side_y - delta_y
            };
            let perp = perp.max(0.0001);

            let line_h = (FB_H as f32 / perp) as i32;
            let half = FB_H as i32 / 2;
            let draw_start = (-line_h / 2 + half).max(0) as usize;
            let draw_end = (line_h / 2 + half).min(FB_H as i32 - 1) as usize;

            // Base color per wall type. Type 2 picks up the active
            // Poline accent so the level reskins with the theme.
            let base = match hit_cell {
                1 => Color32::from_rgb(170, 170, 180),
                2 => accent,
                _ => Color32::from_rgb(120, 120, 120),
            };
            // N/S faces darkened so the geometry reads.
            let side_factor = if hit_side == 1 { 0.65 } else { 1.0 };
            // Inverse falloff fog.
            let fog = (1.0 / (1.0 + perp * 0.18)).clamp(0.2, 1.0);
            let factor = side_factor * fog;
            let color = scale_color(base, factor);

            for y in draw_start..=draw_end {
                self.framebuffer[y * FB_W + col] = color;
            }
        }
    }

    fn draw_minimap(&self, ui: &Ui, image_rect: egui::Rect, accent: Color32) {
        const CELL: f32 = 6.0;
        let pad = 8.0;
        let map_w = MAP_W as f32 * CELL;
        let map_h = MAP_H as f32 * CELL;
        let origin = egui::pos2(image_rect.left() + pad, image_rect.top() + pad);
        let bg_rect = egui::Rect::from_min_size(
            origin - egui::vec2(2.0, 2.0),
            egui::vec2(map_w + 4.0, map_h + 4.0),
        );
        let painter = ui.painter().with_clip_rect(image_rect);
        painter.rect_filled(
            bg_rect,
            2.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, 180),
        );
        for (y, row) in MAP.iter().enumerate() {
            for (x, &c) in row.iter().enumerate() {
                if c == 0 {
                    continue;
                }
                let cell_color = match c {
                    2 => accent,
                    _ => Color32::from_rgb(170, 170, 180),
                };
                let cell_rect = egui::Rect::from_min_size(
                    egui::pos2(origin.x + x as f32 * CELL, origin.y + y as f32 * CELL),
                    egui::vec2(CELL - 1.0, CELL - 1.0),
                );
                painter.rect_filled(cell_rect, 0.0, cell_color);
            }
        }
        // Player dot + facing tick.
        let pp = egui::pos2(
            origin.x + self.px * CELL,
            origin.y + self.py * CELL,
        );
        painter.circle_filled(pp, 2.0, Color32::WHITE);
        let tip = egui::pos2(
            pp.x + self.angle.cos() * CELL * 1.4,
            pp.y + self.angle.sin() * CELL * 1.4,
        );
        painter.line_segment([pp, tip], egui::Stroke::new(1.0, Color32::WHITE));
    }
}

fn is_wall(x: f32, y: f32) -> bool {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    if xi < 0 || yi < 0 || xi >= MAP_W as i32 || yi >= MAP_H as i32 {
        return true;
    }
    MAP[yi as usize][xi as usize] != 0
}

/// Multiply each RGB channel by `f` (0..=1), preserving alpha. Cheap
/// distance-shading helper.
fn scale_color(c: Color32, f: f32) -> Color32 {
    let f = f.clamp(0.0, 1.0);
    Color32::from_rgb(
        (c.r() as f32 * f) as u8,
        (c.g() as f32 * f) as u8,
        (c.b() as f32 * f) as u8,
    )
}
