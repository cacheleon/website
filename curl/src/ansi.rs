use crate::vec3::{Rgb, Vec3};
use std::fmt::Write;

pub const GLYPHS: &str = " .`:-~=+*xo%#&@";

pub const CLEAR: &str = "\x1b[2J\x1b[H";
pub const RESET_END: &str = "\x1b[0m\n";

const HOME: &str = "\x1b[H";
const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";
const CAPTION: &str = "cacheleon.dev";

// same rule as ascii.frag
pub fn glyph_index(lum: f32) -> usize {
    if lum < 0.07 {
        return 0;
    }
    let steps = GLYPHS.len() as f32;
    (((lum - 0.07) / 0.85).min(0.999) * steps).floor().max(1.0) as usize
}

// the 256 palette is coarse, without this the oranges go beige
const SATURATION: f32 = 1.3;

pub fn tint(c: Rgb) -> Rgb {
    let lum = c.max_elem();
    let t = c / lum.max(0.001) * (0.45 + 0.75 * lum);
    let grey = Vec3::splat((t.x + t.y + t.z) / 3.0);
    grey.mix(t, SATURATION).map(|v| v.clamp(0.0, 1.0))
}

// nearest xterm-256 colour, from the 6x6x6 cube or the grey ramp
pub fn color256(c: Rgb) -> u8 {
    const CUBE: [f32; 6] = [0.0, 95.0, 135.0, 175.0, 215.0, 255.0];
    let rgb = [c.x, c.y, c.z].map(|v| v.clamp(0.0, 1.0) * 255.0);
    let nearest_level = |v: f32| {
        (0..6)
            .min_by(|&a, &b| (CUBE[a] - v).abs().total_cmp(&(CUBE[b] - v).abs()))
            .unwrap()
    };
    let dist2 = |p: [f32; 3]| (0..3).map(|i| (p[i] - rgb[i]).powi(2)).sum::<f32>();

    let [r, g, b] = rgb.map(nearest_level);
    let cube = (16 + 36 * r + 6 * g + b) as u8;
    let cube_dist = dist2([CUBE[r], CUBE[g], CUBE[b]]);

    let mean = (rgb[0] + rgb[1] + rgb[2]) / 3.0;
    let k = ((mean - 8.0) / 10.0).round().clamp(0.0, 23.0);
    let grey_level = 8.0 + 10.0 * k;
    let grey_dist = dist2([grey_level; 3]);

    if grey_dist < cube_dist {
        232 + k as u8
    } else {
        cube
    }
}

// colour codes only go out when the colour changes. ends with a reset so
// ctrl-c doesn't leave the terminal coloured.
pub fn encode_frame(cells: &[Rgb], cols: usize) -> String {
    let glyphs = GLYPHS.as_bytes();
    let mut out = String::with_capacity(cells.len() * 8);
    out.push_str(HOME);

    let mut current = None;
    for row in cells.chunks(cols) {
        for &c in row {
            let glyph = glyph_index(c.max_elem());
            if glyph == 0 {
                out.push(' ');
                continue;
            }
            let color = color256(tint(c));
            if current != Some(color) {
                write!(out, "\x1b[38;5;{color}m").unwrap();
                current = Some(color);
            }
            out.push(glyphs[glyph] as char);
        }
        out.push('\n');
    }

    let pad = cols.saturating_sub(CAPTION.len()) / 2;
    write!(out, "{RESET}{DIM}{:pad$}{CAPTION}{RESET}", "").unwrap();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3::Vec3;

    #[test]
    fn glyph_rule_matches_shader() {
        assert_eq!(glyph_index(0.0), 0);
        assert_eq!(glyph_index(0.069), 0);
        assert_eq!(glyph_index(0.07), 1);
        assert_eq!(glyph_index(1.0), 14);
    }

    #[test]
    fn color256_corners() {
        assert_eq!(color256(Vec3::new(0., 0., 0.)), 16);
        assert_eq!(color256(Vec3::new(1., 1., 1.)), 231);
        assert_eq!(color256(Vec3::new(1., 0., 0.)), 196);
        assert_eq!(color256(Vec3::new(0.5, 0.5, 0.5)), 244);
    }

    #[test]
    fn tint_is_a_bit_more_vivid_than_the_homepage() {
        let c = Vec3::new(1.0, 0.5, 0.1);
        let homepage = c / 1.0 * (0.45 + 0.75 * 1.0);
        let spread = |c: Vec3| c.max_elem() - c.x.min(c.y).min(c.z);
        assert!(spread(tint(c)) > spread(homepage.map(|v| v.clamp(0.0, 1.0))));
    }

    #[test]
    fn frame_shape() {
        let cells = [Vec3::new(1., 0.5, 0.1); 4 * 2];
        let f = encode_frame(&cells, 4);
        assert!(f.starts_with("\x1b[H"));
        assert!(f.ends_with("\x1b[0m"));
        assert_eq!(
            f.matches("\x1b[38;5;").count(),
            1,
            "same colour everywhere: set once"
        );
        assert_eq!(f.matches('\n').count(), 2, "2 rows, then the caption");
    }

    #[test]
    fn dark_frame_has_no_colour_codes() {
        let f = encode_frame(&[Vec3::new(0., 0., 0.); 8], 4);
        assert!(!f.contains("\x1b[38;5;"));
    }

    #[test]
    fn caption_is_centred_and_dim() {
        let f = encode_frame(&[Vec3::ZERO; 33], 33);
        assert!(f.contains("\x1b[2m          cacheleon.dev\x1b[0m"));
    }
}
