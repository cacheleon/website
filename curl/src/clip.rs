use crate::ansi::{CLEAR, encode_frame};
use crate::scene::{Camera, DIST, TILT, trace};
use crate::vec3::Rgb;
use std::thread;

// terminal cells are about twice as tall as they are wide
const CELL_ASPECT: f32 = 2.0;

pub struct Spec {
    pub cols: usize,
    pub rows: usize,
    pub frames: usize,
    pub intro_frames: usize,
    pub fps: f32,
    pub fade_secs: f32,
}

// 48x20, a 2.5s fly-in, then an 8s loop
pub const SPEC: Spec = Spec {
    cols: 48,
    rows: 20,
    frames: 96,
    intro_frames: 30,
    fps: 12.0,
    fade_secs: 2.0,
};

// (dist, tilt, yaw) where the fly-in starts
const INTRO_FROM: (f32, f32, f32) = (95.0, 0.7, 0.6);
const SWAY_YAW: f32 = 0.3;
const SWAY_TILT: f32 = 0.08;

pub struct Clip {
    pub intro: Vec<String>,
    pub frames: Vec<String>,
    pub still: String,
}

impl Clip {
    pub fn render(spec: &Spec) -> Clip {
        let encode = |grids: Vec<Vec<Rgb>>| -> Vec<String> {
            grids
                .iter()
                .map(|cells| encode_frame(cells, spec.cols))
                .collect()
        };
        let intro = encode(render_intro(spec));
        let frames = encode(render_loop(spec));
        let still = format!("{CLEAR}{}\n", frames[0]);
        Clip {
            intro,
            frames,
            still,
        }
    }
}

fn loop_length(spec: &Spec) -> f32 {
    spec.frames as f32 / spec.fps
}

pub fn loop_camera(spec: &Spec, time: f32) -> Camera {
    let phase = std::f32::consts::TAU * time / loop_length(spec);
    let yaw = SWAY_YAW * phase.sin();
    let tilt = TILT + SWAY_TILT * (1.0 - phase.cos());
    Camera::orbit(DIST, tilt, yaw)
}

// eases out from INTRO_FROM to exactly where the loop starts
fn intro_camera(progress: f32) -> Camera {
    let e = 1.0 - (1.0 - progress).powi(3);
    let (dist, tilt, yaw) = INTRO_FROM;
    let mix = |from: f32, to: f32| from + (to - from) * e;
    Camera::orbit(mix(dist, DIST), mix(tilt, TILT), mix(yaw, 0.0))
}

pub fn render_frame(spec: &Spec, cam: &Camera, time: f32) -> Vec<Rgb> {
    let aspect = spec.rows as f32 * CELL_ASPECT / spec.cols as f32;
    // same zoom rule as the homepage
    let zoom = 0.62 / (aspect / 0.65).clamp(1.0, 1.5);

    let (w, h) = (spec.cols as f32 * 2.0, spec.rows as f32 * 2.0);
    let ray = |sx: usize, sy: usize| {
        let u = (sx as f32 + 0.5) / w * 2.0 - 1.0;
        let v = (1.0 - (sy as f32 + 0.5) / h * 2.0) * aspect;
        trace(cam, (u, v), zoom, time)
    };

    let mut cells = Vec::with_capacity(spec.cols * spec.rows);
    for row in 0..spec.rows {
        for col in 0..spec.cols {
            let (x, y) = (col * 2, row * 2);
            let sum = ray(x, y) + ray(x + 1, y) + ray(x, y + 1) + ray(x + 1, y + 1);
            cells.push(sum / 4.0);
        }
    }
    cells
}

// the disk's shear never repeats, so the last fade_secs blend into the
// frame one loop earlier. at the very end that's frame 0 and the loop closes.
pub fn render_loop(spec: &Spec) -> Vec<Vec<Rgb>> {
    let length = loop_length(spec);
    let fade_start = length - spec.fade_secs;

    parallel_map(spec.frames, |i| {
        let t = i as f32 / spec.fps;
        let cam = loop_camera(spec, t);
        let now = render_frame(spec, &cam, t);
        if t < fade_start {
            return now;
        }
        let w = (t - fade_start) / spec.fade_secs;
        let before = render_frame(spec, &cam, t - length);
        now.iter().zip(&before).map(|(a, b)| a.mix(*b, w)).collect()
    })
}

// runs in negative time so the disk keeps turning into frame 0
pub fn render_intro(spec: &Spec) -> Vec<Vec<Rgb>> {
    let n = spec.intro_frames;
    parallel_map(n, |i| {
        let t = (i as f32 - n as f32) / spec.fps;
        render_frame(spec, &intro_camera(i as f32 / n as f32), t)
    })
}

fn parallel_map<T: Send>(n: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let workers = thread::available_parallelism()
        .map_or(1, |p| p.get())
        .min(n.max(1));
    let mut out: Vec<Option<T>> = (0..n).map(|_| None).collect();
    let per_worker = n.div_ceil(workers).max(1);
    thread::scope(|s| {
        for (w, chunk) in out.chunks_mut(per_worker).enumerate() {
            let f = &f;
            let first = w * per_worker;
            s.spawn(move || {
                for (k, slot) in chunk.iter_mut().enumerate() {
                    *slot = Some(f(first + k));
                }
            });
        }
    });
    out.into_iter()
        .map(|x| x.expect("every frame rendered"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3::Rgb;

    fn small() -> Spec {
        Spec {
            cols: 24,
            rows: 10,
            frames: 24,
            intro_frames: 12,
            fps: 12.0,
            fade_secs: 0.5,
        }
    }

    fn lit(cells: &[Rgb]) -> usize {
        cells.iter().filter(|c| c.max_elem() >= 0.07).count()
    }

    fn median_step(f: &[Vec<Rgb>]) -> f32 {
        let mut steps: Vec<f32> = f.windows(2).map(|w| diff(&w[0], &w[1])).collect();
        steps.sort_by(f32::total_cmp);
        steps[steps.len() / 2]
    }

    fn diff(a: &[Rgb], b: &[Rgb]) -> f32 {
        a.iter()
            .zip(b)
            .map(|(a, b)| (*a - *b).length())
            .sum::<f32>()
            / a.len() as f32
    }

    #[test]
    fn loop_has_requested_frames() {
        let f = render_loop(&small());
        assert_eq!(f.len(), 24);
        assert!(f.iter().all(|g| g.len() == 240));
    }

    #[test]
    fn loop_seam_is_smooth() {
        let f = render_loop(&small());
        let median = median_step(&f);
        let seam = diff(f.last().unwrap(), &f[0]);
        assert!(seam <= 1.5 * median, "seam {seam} vs median step {median}");
    }

    #[test]
    fn clip_still_is_one_frame() {
        let clip = Clip::render(&small());
        assert_eq!(clip.frames.len(), 24);
        assert!(clip.still.starts_with(crate::ansi::CLEAR));
        assert_eq!(
            clip.still.matches("\x1b[H").count(),
            2,
            "one in CLEAR, one in the frame"
        );
    }

    #[test]
    fn intro_has_requested_frames() {
        assert_eq!(render_intro(&small()).len(), 12);
    }

    #[test]
    fn intro_starts_far_away() {
        let (intro, f) = (render_intro(&small()), render_loop(&small()));
        assert!(
            lit(&intro[0]) < lit(&f[0]),
            "{} vs {}",
            lit(&intro[0]),
            lit(&f[0])
        );
    }

    #[test]
    fn intro_ends_where_loop_starts() {
        let (intro, f) = (render_intro(&small()), render_loop(&small()));
        let join = diff(intro.last().unwrap(), &f[0]);
        assert!(join <= 1.5 * median_step(&f), "join {join}");
    }

    #[test]
    fn camera_sways_and_comes_back() {
        let spec = small();
        let length = spec.frames as f32 / spec.fps;
        let at = |t: f32| loop_camera(&spec, t).pos;
        assert!((at(0.0) - at(length / 4.0)).length() > 1.0);
        assert!((at(0.0) - at(length)).length() < 1e-3);
    }

    #[test]
    fn clip_keeps_intro() {
        assert_eq!(Clip::render(&small()).intro.len(), 12);
    }

    #[test]
    fn frames_actually_move() {
        let f = render_loop(&small());
        assert!(diff(&f[0], &f[6]) > 0.0);
    }
}
