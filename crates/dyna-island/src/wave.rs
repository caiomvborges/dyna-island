//! The Light theme's wave: a small great wave, after Hokusai's print, in
//! the places the island has room for a picture (beside the clock, as the
//! idle look, in the music pill and beside the pin on the pages).
//!
//! It lives in a 100 x 64 box whose bottom edge sits on a line near the
//! island's bottom. The body is one closed path of six cubic curves: a
//! slope rising on the left, the top, the lip curling over to the right,
//! its underside, the hollow's inner face and the foot on the right. Two
//! smaller copies of it, paler, make the bands on its back; white foam with
//! a blue rim runs along the top and the lip, and Fuji stands far off in
//! the hollow.
//!
//! It breaks over and over: the body moves through five poses (calm, rising,
//! curled, plunging, spent in foam) and back, eased between them, over
//! [`CYCLE`] seconds, throwing spray as the lip comes down. Moving costs
//! frames, so it only moves where the caller says (`moving`): in the music
//! pill while something plays, and elsewhere unless battery saver is on;
//! ambient frames already stop while the PC is locked or the display is off.
//! Standing still it shows the curled pose.

use crate::gfx::Canvas;
use dyna_core::geom::{Color, Point, Rect};
use dyna_core::settings::Theme;
use dyna_core::svgpath::Seg;

pub const DEEP: Color = Color::hex(0x1F4E8C);
const MID: Color = Color::hex(0x5B8DC4);
const PALE: Color = Color::hex(0xB9D2EA);
const FOAM: Color = Color::hex(0xFFFFFF);

const BOX_W: f32 = 100.0;
const BOX_H: f32 = 64.0;

/// A pose of the body: the start point, then six cubics (two controls and
/// an end each).
type Pose = [(f32, f32); 19];

const CALM: Pose = [
    (0.0, 64.0),
    (14.0, 62.0), (26.0, 56.0), (36.0, 52.0),
    (44.0, 48.0), (52.0, 46.0), (60.0, 46.0),
    (68.0, 46.0), (74.0, 48.0), (80.0, 52.0),
    (79.5, 52.5), (79.0, 53.0), (79.0, 54.0),
    (79.0, 55.5), (80.0, 57.0), (82.0, 59.0),
    (88.0, 62.0), (94.0, 63.0), (100.0, 64.0),
];
const RISING: Pose = [
    (0.0, 64.0),
    (12.0, 60.0), (22.0, 50.0), (30.0, 38.0),
    (38.0, 24.0), (48.0, 12.0), (60.0, 10.0),
    (70.0, 9.0), (78.0, 14.0), (82.0, 22.0),
    (81.0, 23.0), (79.0, 24.0), (78.0, 26.0),
    (76.0, 31.0), (77.0, 40.0), (80.0, 48.0),
    (86.0, 56.0), (92.0, 61.0), (100.0, 64.0),
];
/// The print's pose, and the one shown while the wave stands still.
const CURLED: Pose = [
    (0.0, 64.0),
    (10.0, 60.0), (22.0, 46.0), (30.0, 32.0),
    (38.0, 16.0), (52.0, 4.0), (68.0, 4.0),
    (82.0, 4.0), (94.0, 12.0), (97.0, 23.0),
    (91.0, 17.0), (81.0, 16.0), (75.0, 21.0),
    (67.0, 28.0), (67.0, 41.0), (75.0, 51.0),
    (81.0, 58.0), (90.0, 62.0), (100.0, 64.0),
];
const PLUNGING: Pose = [
    (0.0, 64.0),
    (10.0, 61.0), (22.0, 52.0), (32.0, 42.0),
    (40.0, 32.0), (52.0, 22.0), (66.0, 22.0),
    (80.0, 22.0), (94.0, 34.0), (98.0, 54.0),
    (92.0, 47.0), (84.0, 43.0), (78.0, 44.0),
    (71.0, 46.0), (71.0, 53.0), (76.0, 57.0),
    (82.0, 61.0), (90.0, 63.0), (100.0, 64.0),
];
const SPENT: Pose = [
    (0.0, 64.0),
    (14.0, 63.0), (28.0, 59.0), (40.0, 57.0),
    (50.0, 55.0), (62.0, 54.0), (74.0, 54.0),
    (84.0, 54.0), (92.0, 57.0), (96.0, 61.0),
    (94.5, 60.5), (93.0, 60.0), (92.0, 60.5),
    (91.5, 61.0), (92.0, 62.0), (93.0, 62.5),
    (95.0, 63.0), (97.5, 63.5), (100.0, 64.0),
];
const POSES: [Pose; 5] = [CALM, RISING, CURLED, PLUNGING, SPENT];
/// How much foam each pose carries.
const FOAMY: [f32; 5] = [0.3, 0.5, 1.0, 1.4, 1.0];

/// One break, in seconds, and when it reaches each pose: a moment calm,
/// then it grows, curls, plunges, spends itself in foam and settles.
pub const CYCLE: f64 = 3.2;
const KEYS: [(f64, usize); 7] = [(0.0, 0), (0.4, 0), (1.2, 1), (1.7, 2), (2.15, 3), (2.7, 4), (CYCLE, 0)];
/// The spray thrown as the lip comes down: when it starts and how long it flies.
const SPRAY_AT: f64 = 1.85;
const SPRAY_LIFE: f64 = 1.0;
/// Each drop's push (right, up) in box units.
const DROPS: [(f32, f32); 5] = [(6.0, 16.0), (12.0, 12.0), (2.0, 20.0), (16.0, 8.0), (9.0, 18.0)];

/// The bands: the body shrunk towards its left foot by these.
const BAND_MID: f32 = 0.78;
const BAND_PALE: f32 = 0.5;
/// Fuji, far off in the hollow (still), and its snow.
const FUJI: [(f32, f32); 3] = [(76.0, 61.0), (86.0, 49.0), (97.0, 61.0)];
const SNOW: [(f32, f32); 6] = [(83.4, 52.1), (86.0, 49.0), (88.8, 52.4), (87.2, 51.6), (86.0, 52.9), (84.8, 51.5)];

pub fn on(theme: Theme) -> bool {
    theme == Theme::Light
}

/// The wave's height for a width of `w` DIPs.
pub fn height(w: f32) -> f32 {
    w * BOX_H / BOX_W
}

fn smooth(x: f64) -> f64 {
    let v = x.clamp(0.0, 1.0);
    v * v * (3.0 - 2.0 * v)
}

/// The body and its foam at `t` seconds into a break.
fn pose_at(t: f64) -> (Pose, f32) {
    let t = t.rem_euclid(CYCLE);
    for w in KEYS.windows(2) {
        let ((t0, a), (t1, b)) = (w[0], w[1]);
        if t <= t1 {
            let u = if t1 > t0 { smooth((t - t0) / (t1 - t0)) as f32 } else { 0.0 };
            let mut p = POSES[a];
            for (q, r) in p.iter_mut().zip(POSES[b]) {
                q.0 += (r.0 - q.0) * u;
                q.1 += (r.1 - q.1) * u;
            }
            return (p, FOAMY[a] + (FOAMY[b] - FOAMY[a]) * u);
        }
    }
    (POSES[0], FOAMY[0])
}

/// The body's `i`-th curve: start, two controls, end.
fn curve(p: &Pose, i: usize) -> [(f32, f32); 4] {
    [p[3 * i], p[3 * i + 1], p[3 * i + 2], p[3 * i + 3]]
}

fn bezier(c: [(f32, f32); 4], t: f32) -> (f32, f32) {
    let m = 1.0 - t;
    let (a, b, d, e) = (m * m * m, 3.0 * m * m * t, 3.0 * m * t * t, t * t * t);
    (a * c[0].0 + b * c[1].0 + d * c[2].0 + e * c[3].0, a * c[0].1 + b * c[1].1 + d * c[2].1 + e * c[3].1)
}

/// Foam along the body: (centre, radius) in box units. Small on the back of
/// the top, full along the top, thinning into claws down the lip.
fn foam(p: &Pose, amount: f32) -> Vec<((f32, f32), f32)> {
    let mut out = Vec::with_capacity(17);
    for (i, t0, t1, n, r0, r1) in [(1, 0.5, 1.0, 5, 1.6, 2.4), (2, 0.0, 1.0, 8, 2.4, 2.6), (3, 0.0, 0.75, 4, 2.4, 1.0)] {
        let c = curve(p, i);
        for k in 0..n {
            let u = k as f32 / (n - 1) as f32;
            out.push((bezier(c, t0 + (t1 - t0) * u), (r0 + (r1 - r0) * u) * amount));
        }
    }
    out
}

/// The spray at `t` into a break, thrown from the lip's tip: (centre,
/// radius, opacity) in box units.
fn spray(p: &Pose, t: f64) -> Vec<((f32, f32), f32, f32)> {
    let t = t.rem_euclid(CYCLE);
    if !(SPRAY_AT..=SPRAY_AT + SPRAY_LIFE).contains(&t) {
        return Vec::new();
    }
    let tip = p[9];
    let age = ((t - SPRAY_AT) / SPRAY_LIFE) as f32;
    DROPS
        .iter()
        .enumerate()
        .filter_map(|(i, &(vx, vy))| {
            let q = age * (0.8 + 0.1 * i as f32);
            (q <= 1.0).then(|| ((tip.0 + vx * q - 4.0, tip.1 - vy * q + 22.0 * q * q), 2.0 * (1.0 - 0.5 * q), 1.0 - q))
        })
        .collect()
}

/// The body as path segments, shrunk by `s` towards its left foot, through `map`.
fn body(p: &Pose, s: f32, map: &impl Fn(f32, f32) -> (f32, f32)) -> Vec<Seg> {
    let at = |(x, y): (f32, f32)| map(x * s, BOX_H - (BOX_H - y) * s);
    let mut segs = Vec::with_capacity(8);
    let (x, y) = at(p[0]);
    segs.push(Seg::MoveTo(x, y));
    for i in 0..6 {
        let [_, c1, c2, e] = curve(p, i);
        let ((ax, ay), (bx, by), (ex, ey)) = (at(c1), at(c2), at(e));
        segs.push(Seg::CubicTo(ax, ay, bx, by, ex, ey));
    }
    segs.push(Seg::Close);
    segs
}

fn polygon(pts: &[(f32, f32)], map: &impl Fn(f32, f32) -> (f32, f32)) -> Vec<Seg> {
    let mut segs: Vec<Seg> = pts
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| {
            let (x, y) = map(x, y);
            if i == 0 { Seg::MoveTo(x, y) } else { Seg::LineTo(x, y) }
        })
        .collect();
    segs.push(Seg::Close);
    segs
}

/// The wave `w` DIPs wide, centred on `cx`, its foot on `base_y`, inside
/// `clip`. While `moving` it breaks with the time `now`; otherwise it
/// stands curled, as in the print.
pub fn draw(c: &mut Canvas, cx: f32, base_y: f32, w: f32, clip: Rect, now: f64, moving: bool) {
    let k = w / BOX_W;
    let map = |x: f32, y: f32| (cx + (x - BOX_W / 2.0) * k, base_y + (y - BOX_H) * k);
    let (p, amount) = if moving { pose_at(now) } else { (CURLED, 1.0) };
    let rim = (0.7 * k).max(0.5);
    c.clip(clip);
    c.unthemed(|c| {
        if w >= 34.0 {
            c.path(&polygon(&FUJI, &map), Some(MID), None);
            c.path(&polygon(&SNOW, &map), Some(FOAM), None);
        }
        c.path(&body(&p, 1.0, &map), Some(DEEP), None);
        c.path(&body(&p, BAND_MID, &map), Some(MID), None);
        c.path(&body(&p, BAND_PALE, &map), Some(PALE), None);
        // Foam: every bubble's blue rim first, then the white over them,
        // so only the outer edge of the crest shows a line.
        let dots = foam(&p, amount);
        for &((x, y), r) in &dots {
            let (px, py) = map(x, y);
            c.fill_circle(Point::new(px, py), r * k + rim, DEEP);
        }
        for &((x, y), r) in &dots {
            let (px, py) = map(x, y);
            c.fill_circle(Point::new(px, py), r * k, FOAM);
        }
        if moving {
            for ((x, y), r, a) in spray(&p, now) {
                let (px, py) = map(x, y);
                c.with_alpha(a, |c| {
                    c.fill_circle(Point::new(px, py), r * k + rim, DEEP);
                    c.fill_circle(Point::new(px, py), r * k, FOAM);
                });
            }
        }
    });
    c.unclip();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pose_keeps_to_its_box_and_its_feet() {
        for p in POSES {
            for (x, y) in p {
                assert!((0.0..=BOX_W).contains(&x) && (0.0..=BOX_H).contains(&y), "{x},{y}");
            }
            assert_eq!(p[0], (0.0, BOX_H));
            assert_eq!(p[18], (BOX_W, BOX_H));
        }
    }

    #[test]
    fn it_breaks_and_comes_back() {
        // Calm at the start, the print's pose mid-way, calm again at the end.
        assert_eq!(pose_at(0.0).0, CALM);
        assert_eq!(pose_at(1.7).0, CURLED);
        assert_eq!(pose_at(2.15).0, PLUNGING);
        assert_eq!(pose_at(CYCLE - 1e-9).0, pose_at(0.0).0);
        // The top is highest while curled, lowest while calm.
        assert!(pose_at(1.7).0[6].1 < pose_at(0.2).0[6].1);
    }

    #[test]
    fn spray_flies_only_as_the_lip_comes_down() {
        assert!(spray(&CURLED, 1.0).is_empty());
        assert!(!spray(&PLUNGING, SPRAY_AT + 0.2).is_empty());
        assert!(spray(&PLUNGING, SPRAY_AT + SPRAY_LIFE + 0.1).is_empty());
    }
}
