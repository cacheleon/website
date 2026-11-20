// cpu port of src/blackhole/scene.frag.
// units are G = c = M = 1: horizon at r = 2, isco at r = 6.

use crate::vec3::{Rgb, Vec3};
use std::f32::consts::PI;

const HORIZON: f32 = 2.0;
const DISK_IN: f32 = 6.0;
const DISK_OUT: f32 = 13.0;
const MAX_STEPS: usize = 400;

// further out and higher up than the homepage so the whole disk fits
pub const DIST: f32 = 33.0;
pub const TILT: f32 = 0.35;

// at 48 columns the dust lights up every cell and buries the disk
const SKY_DUST: f32 = 0.08;

pub struct Camera {
    pub pos: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub fwd: Vec3,
}

impl Camera {
    pub fn orbit(dist: f32, tilt: f32, yaw: f32) -> Camera {
        let pos = Vec3::new(
            dist * tilt.cos() * yaw.sin(),
            dist * tilt.sin(),
            -dist * tilt.cos() * yaw.cos(),
        );
        let fwd = (-pos).normalize();
        let right = Vec3::new(0.0, 1.0, 0.0).cross(fwd).normalize();
        let up = fwd.cross(right);
        Camera {
            pos,
            right,
            up,
            fwd,
        }
    }
}

pub fn trace(cam: &Camera, uv: (f32, f32), zoom: f32, time: f32) -> Rgb {
    let mut pos = cam.pos;
    let mut dir = (cam.right * (uv.0 * zoom) + cam.up * (uv.1 * zoom) + cam.fwd).normalize();

    // null geodesic as a newtonian orbit with an extra -1.5 h^2 / r^5 term
    let h2 = {
        let h = pos.cross(dir);
        h.dot(h)
    };
    let escape = cam.pos.length() * 1.3;

    let mut color = Vec3::ZERO;
    let mut transmittance = 1.0;
    let mut escaped = false;

    for _ in 0..MAX_STEPS {
        let r = pos.length();
        if r < HORIZON {
            break;
        }
        if r > escape && pos.dot(dir) > 0.0 {
            escaped = true;
            break;
        }

        let dt = (0.04 * r * (r - 2.5).max(1.0)).min(1.5);
        dir -= pos * (1.5 * h2 / r.powi(5) * dt);
        let next = pos + dir * dt;

        if pos.y * next.y < 0.0 {
            let hit = pos.mix(next, pos.y / (pos.y - next.y));
            let (gas, alpha) = disk(hit, dir, time);
            color += gas * (transmittance * alpha);
            transmittance *= 1.0 - alpha;
            if transmittance < 0.02 {
                break;
            }
        }
        pos = next;
    }
    if escaped {
        color += sky(dir.normalize(), time) * transmittance;
    }

    color.map(|c| 1.0 - (-c * 1.3).exp())
}

fn disk(p: Vec3, dir: Vec3, time: f32) -> (Rgb, f32) {
    let r = (p.x * p.x + p.z * p.z).sqrt();
    if !(DISK_IN..=DISK_OUT).contains(&r) {
        return (Vec3::ZERO, 0.0);
    }

    // keplerian rotation, so the streaks shear over time
    let a = (p.z.atan2(p.x) - time * 9.0 * r.powf(-1.5)) / (2.0 * PI);
    let gas = 0.5 * noise2((a * 16.0, r * 1.6), 16.0)
        + 0.3 * noise2((a * 40.0, r * 4.0), 40.0)
        + 0.2 * noise2((a * 90.0, r * 9.0), 90.0);
    let gas = smoothstep(0.2, 0.9, gas);

    // doppler + gravitational redshift
    let v = (1.0 / (r - 2.0)).sqrt().min(0.99);
    let gamma = 1.0 / (1.0 - v * v).sqrt();
    let cos_theta = Vec3::new(-p.z, 0.0, p.x).normalize().dot(-dir.normalize());
    let g = (1.0 - 2.0 / r).sqrt() / (gamma * (1.0 - v * cos_theta));

    let edge =
        smoothstep(DISK_IN, DISK_IN + 0.6, r) * (1.0 - smoothstep(DISK_OUT - 5.0, DISK_OUT, r));
    let light = (DISK_IN / r).powi(2) * (0.25 + 1.6 * gas) * g.powi(3) * 2.6;
    (gas_color(g) * light, edge * (0.75 + 0.25 * gas))
}

// g < 1 redshifted, g > 1 blueshifted
fn gas_color(g: f32) -> Rgb {
    let c = Vec3::new(0.45, 0.05, 0.02).mix(Vec3::new(1.0, 0.35, 0.08), smoothstep(0.3, 0.75, g));
    let c = c.mix(Vec3::new(1.0, 0.72, 0.38), smoothstep(0.7, 1.05, g));
    let c = c.mix(Vec3::new(1.0, 0.95, 0.85), smoothstep(1.0, 1.35, g));
    c.mix(Vec3::new(0.7, 0.82, 1.0), smoothstep(1.35, 1.8, g))
}

fn sky(d: Vec3, time: f32) -> Rgb {
    let mut c = Vec3::ZERO;

    let h = hash3(d.map(|v| (v * 140.0).floor()));
    if h > 0.992 {
        let twinkle = 0.7 + 0.3 * (time * 3.0 + h * 900.0).sin();
        c += Vec3::new(0.85, 0.9, 1.0) * ((h - 0.992) * 125.0 * twinkle);
    }

    let band = (-d.dot(Vec3::new(0.3, 1.0, 0.45).normalize()).powi(2) * 6.0).exp();
    let dust = noise3(d * 4.0) * 0.6 + noise3(d * 11.0) * 0.4;
    let tint = Vec3::new(0.3, 0.22, 0.6).mix(
        Vec3::new(0.15, 0.25, 0.65),
        noise3(d * 2.0 + Vec3::splat(7.0)),
    );
    c + tint * (smoothstep(0.35, 0.85, dust) * (0.22 + 0.4 * band) * SKY_DUST)
}

fn fract(x: f32) -> f32 {
    x - x.floor()
}

// glsl mod, takes the sign of y
fn modulo(x: f32, y: f32) -> f32 {
    x - y * (x / y).floor()
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn hash2(p: (f32, f32)) -> f32 {
    let (mut x, mut y) = (fract(p.0 * 123.34), fract(p.1 * 456.21));
    let d = x * (x + 45.32) + y * (y + 45.32);
    x += d;
    y += d;
    fract(x * y)
}

fn hash3(p: Vec3) -> f32 {
    let mut p = Vec3::new(
        fract(p.x * 0.1031),
        fract(p.y * 0.1030),
        fract(p.z * 0.0973),
    );
    let d = p.dot(Vec3::new(p.y, p.x, p.z) + Vec3::splat(33.33));
    p += Vec3::splat(d);
    fract((p.x + p.y) * p.z)
}

// value noise, x wraps every `period` cells
fn noise2(p: (f32, f32), period: f32) -> f32 {
    let (ix, iy) = (p.0.floor(), p.1.floor());
    let (fx, fy) = (
        smoothstep(0.0, 1.0, fract(p.0)),
        smoothstep(0.0, 1.0, fract(p.1)),
    );
    let x0 = modulo(ix, period);
    let x1 = modulo(ix + 1.0, period);
    lerp(
        lerp(hash2((x0, iy)), hash2((x1, iy)), fx),
        lerp(hash2((x0, iy + 1.0)), hash2((x1, iy + 1.0)), fx),
        fy,
    )
}

fn noise3(p: Vec3) -> f32 {
    let i = p.map(f32::floor);
    let f = p.map(|v| smoothstep(0.0, 1.0, fract(v)));
    let h = |x: f32, y: f32, z: f32| hash3(i + Vec3::new(x, y, z));
    lerp(
        lerp(
            lerp(h(0., 0., 0.), h(1., 0., 0.), f.x),
            lerp(h(0., 1., 0.), h(1., 1., 0.), f.x),
            f.y,
        ),
        lerp(
            lerp(h(0., 0., 1.), h(1., 0., 1.), f.x),
            lerp(h(0., 1., 1.), h(1., 1., 1.), f.x),
            f.y,
        ),
        f.z,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3::{Rgb, Vec3};

    fn lum(c: Rgb) -> f32 {
        c.max_elem()
    }

    fn grid(cols: usize, rows: usize) -> Vec<Rgb> {
        let cam = Camera::orbit(DIST, TILT, 0.0);
        let aspect = rows as f32 * 2.0 / cols as f32;
        let zoom = 0.62 / (aspect / 0.65).clamp(1.0, 1.5);
        let mut out = Vec::with_capacity(cols * rows);
        for r in 0..rows {
            for c in 0..cols {
                let u = (c as f32 + 0.5) / cols as f32 * 2.0 - 1.0;
                let v = (1.0 - (r as f32 + 0.5) / rows as f32 * 2.0) * aspect;
                out.push(trace(&cam, (u, v), zoom, 0.0));
            }
        }
        out
    }

    #[test]
    fn shadow_is_dark() {
        let g = grid(48, 20);
        assert!(lum(g[10 * 48 + 24]) < 0.07);
    }

    #[test]
    fn disk_is_visible() {
        let g = grid(48, 20);
        let lit = g.iter().filter(|c| lum(**c) >= 0.07).count();
        assert!(lit >= 48 * 20 * 15 / 100, "only {lit} lit cells");
    }

    #[test]
    fn brightest_cell_is_warm() {
        let g = grid(48, 20);
        let b = g
            .iter()
            .max_by(|a, b| lum(**a).total_cmp(&lum(**b)))
            .unwrap();
        assert!(b.x > b.z, "{b:?}");
    }

    #[test]
    fn doppler_makes_one_side_brighter() {
        let g = grid(48, 20);
        let (l, r) = (0..48 * 20).fold((0.0f32, 0.0f32), |(l, r), i| {
            if i % 48 < 24 {
                (l + lum(g[i]), r)
            } else {
                (l, r + lum(g[i]))
            }
        });
        assert!((l - r).abs() / l.max(r) > 0.2, "left {l} right {r}");
    }

    #[test]
    fn deterministic() {
        assert_eq!(grid(12, 5), grid(12, 5));
    }

    #[test]
    fn vec3_basics() {
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(0.0, 1.0, 0.0);
        assert_eq!(a.cross(b), Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(Vec3::new(3.0, 4.0, 0.0).length(), 5.0);
    }
}
