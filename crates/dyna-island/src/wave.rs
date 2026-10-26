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
//! curled, plunging, spent in foam) and back over [`CYCLE`] seconds, throwing
//! spray as the lip comes down. It never stops at a pose: each point runs
//! along a smooth curve through all five (a cardinal spline in time), so it
//! slows as it gathers and speeds up as it falls, the way water does. On top
//! of that the crest leans forward as it builds, a swell keeps running along
//! the water and the foam fizzes and runs forward along the crest. No two
//! breaks are alike: each one is a little taller or lower and leans more or
//! less (drawn from its number, so the same moment always looks the same).
//! The paler layers follow the deep one a moment late, as the water inside a
//! wave does, and the spray flies off on its own once thrown. Moving costs
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

/// One break, in seconds, and when it passes each pose: it grows slowly,
/// curls, plunges quickly, spends itself in foam and settles into the next.
pub const CYCLE: f64 = 3.2;
const AT: [f64; 5] = [0.0, 1.05, 1.65, 2.1, 2.65];
/// How far (box units) the crest leans forward at each pose; the feet stay.
const LEAN: [f32; 5] = [0.0, 2.5, 4.0, 5.0, 2.0];
/// A swell running along the water all the time: its height and length
/// (box units) and period (seconds).
const SWELL_H: f32 = 1.1;
const SWELL_LEN: f64 = 48.0;
const SWELL_T: f64 = 1.4;
/// The spray thrown as the lip comes down: when it starts and how long it
/// flies at most, how many drops and how hard gravity pulls (box units/s²).
const SPRAY_AT: f64 = 1.85;
const SPRAY_LIFE: f64 = 1.0;
const DROPS: usize = 8;
const GRAVITY: f32 = 60.0;
/// How much a break's height and lean may differ from the drawn poses.
const HEIGHT_RANGE: (f32, f32) = (0.84, 1.04);
const LEAN_RANGE: (f32, f32) = (0.6, 1.4);
/// How far behind the deep body the paler layers move (seconds).
const MID_LAG: f64 = 0.07;
const PALE_LAG: f64 = 0.14;
/// How many foam bubbles ride the crest, and how fast they run along it
/// (crests a second).
const CREST_BUBBLES: usize = 13;
const FOAM_RUN: f64 = 0.7;

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

/// Where a time falls between two poses: the pose before it and the cubic
/// Hermite weights for (value before, slope before, value after, slope
/// after), the slopes already scaled to the span.
struct Span {
    i: usize,
    w: [f32; 4],
}

/// The time of the `i`-th pose counted from the start of this break (`i`
/// may run past either end: the poses repeat every [`CYCLE`]).
fn time_of(i: i64) -> f64 {
    AT[i.rem_euclid(5) as usize] + CYCLE * i.div_euclid(5) as f64
}

fn span(t: f64) -> Span {
    let t = t.rem_euclid(CYCLE);
    let i = AT.iter().rposition(|&a| a <= t).unwrap_or(0);
    let (t0, t1) = (time_of(i as i64), time_of(i as i64 + 1));
    let h = t1 - t0;
    let u = ((t - t0) / h) as f32;
    let (u2, u3) = (u * u, u * u * u);
    Span { i, w: [2.0 * u3 - 3.0 * u2 + 1.0, (u3 - 2.0 * u2 + u) * h as f32, -2.0 * u3 + 3.0 * u2, (u3 - u2) * h as f32] }
}

/// One value through the five poses' `vals`: Catmull-Rom slopes (the
/// neighbours' difference over their time apart), so it never halts.
fn through(sp: &Span, vals: impl Fn(usize) -> f32) -> f32 {
    let at = |k: i64| vals(k.rem_euclid(5) as usize);
    let i = sp.i as i64;
    let slope = |k: i64| (at(k + 1) - at(k - 1)) / (time_of(k + 1) - time_of(k - 1)) as f32;
    sp.w[0] * at(i) + sp.w[1] * slope(i) + sp.w[2] * at(i + 1) + sp.w[3] * slope(i + 1)
}

/// The body, its foam and the crest's lean at `t` seconds into a break.
fn pose_at(t: f64) -> (Pose, f32, f32) {
    let sp = span(t);
    let mut p = CALM;
    for (j, q) in p.iter_mut().enumerate() {
        q.0 = through(&sp, |k| POSES[k][j].0).clamp(0.0, BOX_W);
        q.1 = through(&sp, |k| POSES[k][j].1).clamp(0.0, BOX_H);
    }
    (p, through(&sp, |k| FOAMY[k]).max(0.2), through(&sp, |k| LEAN[k]))
}

/// A number in 0..1 for the `n`-th break, the same every time (SplitMix64).
fn rnd(n: i64, salt: u64) -> f32 {
    let mut z = (n as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(salt.wrapping_mul(0xBF58_476D_1CE4_E5B9));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

fn in_range((lo, hi): (f32, f32), u: f32) -> f32 {
    lo + (hi - lo) * u
}

/// Which break `t` (seconds, any) falls in, and how far into it. Worked out
/// together: `div_euclid` and `rem_euclid` on their own can disagree by a
/// whole break right at its edge.
fn break_of(t: f64) -> (i64, f64) {
    let n = (t / CYCLE).floor();
    (n as i64, (t - n * CYCLE).clamp(0.0, CYCLE * (1.0 - 1e-12)))
}

/// The moving wave at `t` (seconds, any): its body and its foam. Each break
/// gets its own height and lean, blended into the next over the break so
/// there is never a jump.
fn wave_at(t: f64) -> (Pose, f32) {
    let (n, phase) = break_of(t);
    let blend = {
        let x = (phase / CYCLE) as f32;
        x * x * (3.0 - 2.0 * x)
    };
    let mix = |salt: u64, range: (f32, f32)| {
        let (a, b) = (in_range(range, rnd(n, salt)), in_range(range, rnd(n + 1, salt)));
        a + (b - a) * blend
    };
    let (height, lean_k) = (mix(1, HEIGHT_RANGE), mix(2, LEAN_RANGE));
    let (mut p, amount, lean) = pose_at(phase);
    for q in p.iter_mut() {
        q.1 = (BOX_H - (BOX_H - q.1) * height).clamp(0.0, BOX_H);
    }
    (swell(&p, lean * lean_k, t), amount)
}

/// The pose with the crest leaning `lean` forward and the swell at `t`
/// running along it; the feet stay put.
fn swell(p: &Pose, lean: f32, t: f64) -> Pose {
    let mut out = *p;
    for q in out.iter_mut() {
        let up = (BOX_H - q.1) / BOX_H;
        let wave = (std::f64::consts::TAU * (q.0 as f64 / SWELL_LEN - t / SWELL_T)).sin() as f32;
        q.0 = (q.0 + lean * up).min(BOX_W);
        q.1 = (q.1 - SWELL_H * wave * up.sqrt()).clamp(0.0, BOX_H);
    }
    out
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
/// the top, full along the top, thinning into claws down the lip. While it
/// moves (`moving`: the time) the crest's bubbles run forward along it, a
/// new one swelling in at the back as each reaches the lip, and every bubble
/// fizzes, out of step with the others.
fn foam(p: &Pose, amount: f32, moving: Option<f64>) -> Vec<((f32, f32), f32)> {
    let mut out = Vec::with_capacity(CREST_BUBBLES + 4);
    let fizz = |t: f64, k: usize| 1.0 + 0.12 * (t * 7.0 + k as f64 * 1.9).sin() as f32;
    // The crest: the back half of the top curve, then the curve over to the
    // lip's tip, as one track (its first part is the shorter).
    let (back, over) = (curve(p, 1), curve(p, 2));
    let flow = moving.map_or(0.0, |t| (t * FOAM_RUN * CREST_BUBBLES as f64).rem_euclid(1.0) as f32);
    for k in 0..CREST_BUBBLES {
        let u = (k as f32 + flow) / CREST_BUBBLES as f32;
        let at = if u < 0.38 { bezier(back, 0.5 + 0.5 * u / 0.38) } else { bezier(over, (u - 0.38) / 0.62) };
        let r = if u < 0.38 { 1.6 + 0.8 * u / 0.38 } else { 2.4 + 0.2 * (u - 0.38) / 0.62 };
        // The newest bubble swells in from nothing at the back.
        let grow = (u * CREST_BUBBLES as f32).min(1.0);
        out.push((at, r * amount * grow * moving.map_or(1.0, |t| fizz(t, k))));
    }
    // The claws down the lip.
    let lip = curve(p, 3);
    for k in 0..4 {
        let u = k as f32 / 3.0;
        out.push((bezier(lip, 0.75 * u), (2.4 - 1.4 * u) * amount * moving.map_or(1.0, |t| fizz(t, 20 + k))));
    }
    out
}

/// The spray at `t` (seconds, any): drops thrown from where the lip's tip
/// was as it came down, each with its own push, size and life, then left to
/// fall on their own. (centre, radius, opacity) in box units.
fn spray(t: f64) -> Vec<((f32, f32), f32, f32)> {
    let n = break_of(t).0;
    let start = n as f64 * CYCLE + SPRAY_AT;
    let age = (t - start) as f32;
    if !(0.0..=SPRAY_LIFE as f32).contains(&age) {
        return Vec::new();
    }
    let tip = wave_at(start).0[9];
    (0..DROPS as u64)
        .filter_map(|i| {
            let r = |salt: u64| rnd(n, 10 + i * 5 + salt);
            let (vx, vy) = (in_range((-4.0, 22.0), r(0)), in_range((14.0, 34.0), r(1)));
            let life = SPRAY_LIFE as f32 * in_range((0.55, 1.0), r(2));
            let q = age / life;
            (q <= 1.0).then(|| {
                let at = (tip.0 + vx * age, tip.1 - vy * age + 0.5 * GRAVITY * age * age);
                (at, in_range((1.1, 2.3), r(3)) * (1.0 - 0.5 * q), 1.0 - q * q)
            })
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
    let (p, amount) = if moving { wave_at(now) } else { (CURLED, 1.0) };
    // The paler layers move a moment behind the deep body.
    let layer = |lag: f64| if moving { wave_at(now - lag).0 } else { CURLED };
    let (p_mid, p_pale) = (layer(MID_LAG), layer(PALE_LAG));
    let rim = (0.7 * k).max(0.5);
    c.clip(clip);
    c.unthemed(|c| {
        if w >= 34.0 {
            c.path(&polygon(&FUJI, &map), Some(MID), None);
            c.path(&polygon(&SNOW, &map), Some(FOAM), None);
        }
        c.path(&body(&p, 1.0, &map), Some(DEEP), None);
        c.path(&body(&p_mid, BAND_MID, &map), Some(MID), None);
        c.path(&body(&p_pale, BAND_PALE, &map), Some(PALE), None);
        // Foam: every bubble's blue rim first, then the white over them,
        // so only the outer edge of the crest shows a line.
        let dots = foam(&p, amount, moving.then_some(now));
        for &((x, y), r) in &dots {
            let (px, py) = map(x, y);
            c.fill_circle(Point::new(px, py), r * k + rim, DEEP);
        }
        for &((x, y), r) in &dots {
            let (px, py) = map(x, y);
            c.fill_circle(Point::new(px, py), r * k, FOAM);
        }
        if moving {
            for ((x, y), r, a) in spray(now) {
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
        // It passes through every pose exactly, at its time.
        for (k, pose) in POSES.iter().enumerate() {
            assert_eq!(&pose_at(AT[k]).0, pose, "pose {k}");
        }
        // The end of one break runs into the start of the next.
        let (a, b) = (pose_at(CYCLE - 1e-6).0, pose_at(0.0).0);
        for (p, q) in a.iter().zip(b) {
            assert!((p.0 - q.0).abs() < 0.01 && (p.1 - q.1).abs() < 0.01);
        }
        // The top is highest while curled, lowest while calm.
        assert!(pose_at(AT[2]).0[6].1 < pose_at(0.2).0[6].1);
    }

    #[test]
    fn it_never_stops_between_poses() {
        // The crest's top keeps moving through each pose (no easing to a halt).
        for &t in &AT[1..4] {
            let (a, b) = (pose_at(t - 0.02).0[6], pose_at(t + 0.02).0[6]);
            assert!((a.0 - b.0).abs() + (a.1 - b.1).abs() > 0.05, "stops at {t}");
        }
    }

    #[test]
    fn spray_flies_only_as_the_lip_comes_down() {
        assert!(spray(1.0).is_empty());
        assert!(!spray(SPRAY_AT + 0.2).is_empty());
        assert!(spray(SPRAY_AT + SPRAY_LIFE + 0.1).is_empty());
        // Once thrown, a drop doesn't follow the lip: it falls.
        let (a, b) = (spray(SPRAY_AT + 0.3), spray(SPRAY_AT + 0.5));
        assert!(b.iter().zip(&a).any(|(b, a)| b.0 .1 > a.0 .1));
    }

    #[test]
    fn no_two_breaks_alike_and_no_jumps_between_them() {
        let top = |t: f64| wave_at(t).0[6].1;
        let peaks: Vec<f32> = (0..6).map(|n| top(n as f64 * CYCLE + AT[2])).collect();
        assert!(peaks.windows(2).any(|w| (w[0] - w[1]).abs() > 0.3), "{peaks:?}");
        for n in 1..6 {
            let at = n as f64 * CYCLE;
            assert!((top(at - 1e-4) - top(at)).abs() < 0.2, "jump at break {n}");
        }
        for n in 0..50 {
            let v = rnd(n, 1);
            assert!((0.0..1.0).contains(&v));
        }
    }
}
