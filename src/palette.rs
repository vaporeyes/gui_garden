// Minimal Rust port of Poline's polar-interpolation palette engine,
// used across the app (theme, fractal clock, calculator) to keep a
// coherent time-of-day-aware accent palette.
//
// The astro-blog version does: time-of-day bias + seasonal drift +
// sinusoidal/arc curves between two anchor hues. This port keeps just
// enough of that to produce pleasing N-color palettes deterministically.

use egui::Color32;

#[derive(Clone, Copy, Debug)]
pub struct Hsl {
    /// Hue in degrees, 0..360.
    pub h: f32,
    /// Saturation, 0..1.
    pub s: f32,
    /// Lightness, 0..1.
    pub l: f32,
}

impl Hsl {
    pub const fn new(h: f32, s: f32, l: f32) -> Self {
        Self { h, s, l }
    }

    pub fn to_color32(self) -> Color32 {
        let (r, g, b) = hsl_to_rgb(self.h, self.s, self.l);
        Color32::from_rgb(r, g, b)
    }
}

/// Shape of the interpolation curve between two anchor hues. Names and
/// behaviour mirror Poline's `positionFunctions`. Only `Sinusoidal` is
/// currently used; the other variants are kept for callers that want to
/// pick a different curve without changing this module.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum Curve {
    Linear,
    Sinusoidal,
    Arc,
    Quadratic,
}

impl Curve {
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Curve::Linear => t,
            Curve::Sinusoidal => 0.5 - (t * std::f32::consts::PI).cos() * 0.5,
            Curve::Arc => (1.0 - (1.0 - t).powi(2)).sqrt(),
            Curve::Quadratic => t * t,
        }
    }
}

/// Generate `count` colors interpolated between two anchor HSL points.
/// `count == 1` returns the midpoint; `count == 0` returns empty.
pub fn interpolate(a: Hsl, b: Hsl, count: usize, curve: Curve) -> Vec<Color32> {
    if count == 0 {
        return Vec::new();
    }
    if count == 1 {
        return vec![lerp_hsl(a, b, 0.5).to_color32()];
    }
    (0..count)
        .map(|i| {
            let t = i as f32 / (count - 1) as f32;
            let curved = curve.apply(t);
            lerp_hsl(a, b, curved).to_color32()
        })
        .collect()
}

/// The hue the astro-blog's palette would anchor on right now, based on
/// local wall-clock time. Dawn warm, midday cool, dusk amber, night teal.
pub fn time_of_day_hue(hour: u32) -> f32 {
    match hour {
        5..=8 => 20.0,     // dawn — coral/peach
        9..=13 => 200.0,   // midday — bright blue
        14..=17 => 35.0,   // afternoon — amber/gold
        18..=20 => 270.0,  // dusk — purple/violet
        _ => 180.0,        // night — deep teal
    }
}

/// Small seasonal drift on top of the time-of-day hue.
pub fn season_drift(month0: u32) -> f32 {
    match month0 {
        2..=4 => 10.0,    // spring — slight warm push
        5..=7 => 25.0,    // summer — warmer
        8..=10 => -10.0,  // fall — cooler amber
        _ => -20.0,       // winter — cool
    }
}

/// Two anchor hues tuned for "today", one main and one 40-degree-shifted
/// secondary for palette harmony. Saturation is modulated by time too —
/// muted at night, vivid at midday.
pub fn anchors_for_now() -> (Hsl, Hsl) {
    use chrono::{Datelike, Local, Timelike};
    let now = Local::now();
    let hour = now.hour();
    let month0 = now.month0();
    let base = time_of_day_hue(hour) + season_drift(month0);
    let saturation = time_saturation(hour);
    (
        Hsl::new(normalize_hue(base), saturation, 0.58),
        Hsl::new(normalize_hue(base + 40.0), saturation * 0.85, 0.48),
    )
}

/// The single "accent color" the rest of the app should use — the first
/// anchor of the time-biased palette at full saturation.
pub fn accent_now() -> Color32 {
    anchors_for_now().0.to_color32()
}

fn time_saturation(hour: u32) -> f32 {
    match hour {
        9..=17 => 0.80,
        6..=8 | 18..=20 => 0.70,
        _ => 0.55,
    }
}

fn normalize_hue(h: f32) -> f32 {
    ((h % 360.0) + 360.0) % 360.0
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Interpolate HSL along the shorter path around the hue circle.
fn lerp_hsl(a: Hsl, b: Hsl, t: f32) -> Hsl {
    let dh = {
        let raw = b.h - a.h;
        // Take the shorter direction around the circle.
        if raw > 180.0 {
            raw - 360.0
        } else if raw < -180.0 {
            raw + 360.0
        } else {
            raw
        }
    };
    Hsl {
        h: normalize_hue(a.h + dh * t),
        s: lerp(a.s, b.s, t),
        l: lerp(a.l, b.l, t),
    }
}

/// HSL → sRGB conversion (standard formula). Inputs: h in degrees, s/l in 0..1.
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let h = normalize_hue(h) / 360.0;
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);

    if s == 0.0 {
        let v = (l * 255.0).round() as u8;
        return (v, v, v);
    }

    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;

    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);
    (
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    )
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 1.0 / 2.0 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    }
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_color_eq(a: Color32, b: Color32, tolerance: i32) {
        let ar = a.to_array();
        let br = b.to_array();
        for i in 0..3 {
            assert!(
                (ar[i] as i32 - br[i] as i32).abs() <= tolerance,
                "channel {} differs: {:?} vs {:?}",
                i,
                ar,
                br,
            );
        }
    }

    #[test]
    fn hsl_red_roundtrip() {
        let c = Hsl::new(0.0, 1.0, 0.5).to_color32();
        assert_color_eq(c, Color32::from_rgb(255, 0, 0), 1);
    }

    #[test]
    fn hsl_green_roundtrip() {
        let c = Hsl::new(120.0, 1.0, 0.5).to_color32();
        assert_color_eq(c, Color32::from_rgb(0, 255, 0), 1);
    }

    #[test]
    fn hsl_grayscale_zero_saturation() {
        let c = Hsl::new(200.0, 0.0, 0.5).to_color32();
        assert_color_eq(c, Color32::from_rgb(128, 128, 128), 1);
    }

    #[test]
    fn interpolate_produces_n_colors() {
        let a = Hsl::new(0.0, 1.0, 0.5);
        let b = Hsl::new(120.0, 1.0, 0.5);
        assert_eq!(interpolate(a, b, 5, Curve::Linear).len(), 5);
        assert_eq!(interpolate(a, b, 1, Curve::Linear).len(), 1);
        assert_eq!(interpolate(a, b, 0, Curve::Linear).len(), 0);
    }

    #[test]
    fn interpolate_endpoints_are_anchors() {
        let a = Hsl::new(0.0, 1.0, 0.5);
        let b = Hsl::new(120.0, 1.0, 0.5);
        let p = interpolate(a, b, 3, Curve::Linear);
        assert_color_eq(p[0], a.to_color32(), 1);
        assert_color_eq(p[2], b.to_color32(), 1);
    }

    #[test]
    fn hue_interpolation_takes_short_path() {
        // 350° → 10° should go through 0°, not sweep across 180°.
        let a = Hsl::new(350.0, 1.0, 0.5);
        let b = Hsl::new(10.0, 1.0, 0.5);
        let mid = lerp_hsl(a, b, 0.5);
        // Midpoint should be near 0° / 360°, not 180°.
        assert!(mid.h < 20.0 || mid.h > 340.0, "expected ≈0°, got {}", mid.h);
    }

    #[test]
    fn curves_are_monotonic_and_bounded() {
        for curve in [Curve::Linear, Curve::Sinusoidal, Curve::Arc, Curve::Quadratic] {
            assert_eq!(curve.apply(0.0), 0.0);
            let v1 = curve.apply(1.0);
            assert!((v1 - 1.0).abs() < 1e-5, "curve {:?} end = {}", curve, v1);
            // Monotonic increasing
            let mut last = -1.0;
            for i in 0..=10 {
                let t = i as f32 / 10.0;
                let v = curve.apply(t);
                assert!(v >= last - 1e-5, "{:?} not monotonic at {}", curve, t);
                last = v;
            }
        }
    }

    #[test]
    fn time_of_day_bias_is_stable_over_ranges() {
        // Same band returns same hue.
        assert_eq!(time_of_day_hue(10), time_of_day_hue(12));
        assert_eq!(time_of_day_hue(2), time_of_day_hue(4));
        // Different bands differ.
        assert_ne!(time_of_day_hue(7), time_of_day_hue(15));
    }
}
