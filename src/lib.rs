//! Pictures of a 3D mouse for egui, drawn in code: a cap on a round base (the kind that springs
//! back to the middle) or a trackball in its scroll ring, seen from the user's seat, moving as the
//! device does. Each way of moving it has a marker that lights while it reads as motion.
//!
//! The cap: the groove on its front and the pointer on its top turn against the ticks on the base
//! (twist), the mast tilts away from the dashed upright (tilt), the cap leaves the ring it rests
//! in (slide), and the gauge beside it rises and falls (pull up, press down). Pass [`Pose::new`]
//! a reading on the six axes, each ±1 at full deflection, and draw it with [`paint`].
//!
//! The trackball: a ball with dots spread over it and a scroll ring with notches. A trackball
//! reports speed, not position, so [`ball::Ball`] keeps its turn between frames: roll it each
//! frame with [`ball::Ball::roll`] (see [`ball_rate`]) and draw it with [`ball::paint`].
//!
//! Generic shapes only, not any maker's device. Colours come from the egui theme.
#![forbid(unsafe_code)]

use egui::{Pos2, Shape, Stroke};

/// A point in the device's space: right, away from the user, up. The base's radius is 1.
type P3 = [f32; 3];

/// How far down the view looks, in radians.
const PITCH: f32 = 0.52;
/// Full deflection, in base radii and radians.
const SLIDE: f32 = 0.18;
const LIFT: f32 = 0.09;
const TILT: f32 = 0.35;
const TWIST: f32 = 0.6;
const BASE_HEIGHT: f32 = 0.22;
/// The base under the cap, and under the trackball's scroll ring.
const BASE_RADIUS: f32 = 0.8;
/// The cap's bottom at rest; the gap under it shows the light, closing when pressed.
const CAP_BOTTOM: f32 = BASE_HEIGHT + 0.1;
const CAP_RADIUS: f32 = 0.5;
const CAP_HEIGHT: f32 = 0.62;
/// Where the gauge stands, and how far its pointer goes at full deflection.
const GAUGE: [f32; 2] = [1.05, 0.6];
const GAUGE_TRAVEL: f32 = 0.3;

/// The cap's position and turn for a reading on the canonical axes (±1 at full deflection):
/// slid right +X, pulled toward the user +Y, pressed down +Z, tilted back +Rx, tilted left +Ry,
/// twisted clockwise +Rz.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    offset: P3,
    /// Columns of the cap's rotation.
    turn: [P3; 3],
}

impl Pose {
    pub fn new(travel: [f32; 6]) -> Self {
        let t = travel.map(|v| {
            if v.is_finite() {
                v.clamp(-1.5, 1.5)
            } else {
                0.0
            }
        });
        // Twisting clockwise as seen from above is a negative turn about up; tilting back takes
        // the top away from the user, tilting left takes it to the left.
        let twist = rotate_z(-t[5] * TWIST);
        let back = rotate_x(t[3] * TILT);
        let left = rotate_y(-t[4] * TILT);
        let turn = [0, 1, 2].map(|i| apply(&left, apply(&back, apply(&twist, unit(i)))));
        Self {
            offset: [t[0] * SLIDE, -t[1] * SLIDE, CAP_BOTTOM - t[2] * LIFT],
            turn,
        }
    }

    /// A point on the cap (from the centre of its bottom) in the device's space.
    fn place(&self, p: P3) -> P3 {
        add(apply(&self.turn, p), self.offset)
    }
}

/// Which way the user looks at the device.
fn eye() -> P3 {
    [0.0, -PITCH.cos(), PITCH.sin()]
}

/// The device's space on screen, as a share of `scale`, y down.
fn project(p: P3) -> [f32; 2] {
    [p[0], -(p[1] * PITCH.sin() + p[2] * PITCH.cos())]
}

/// Draws the device in `rect` for `pose`; `moving` lights each canonical axis's marker.
pub fn paint(ui: &egui::Ui, rect: egui::Rect, pose: &Pose, moving: [bool; 6]) {
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    let scale = (rect.width() / 2.9).min(rect.height() / 2.1);
    let origin = rect.center() + egui::vec2(-0.15, 0.4) * scale;
    let at = |p: P3| {
        let [x, y] = project(p);
        origin + egui::vec2(x, y) * scale
    };
    let (faint, accent) = (visuals.weak_text_color(), visuals.selection.stroke.color);
    let marker = |live: bool| {
        Stroke::new(
            if live { 2.5 } else { 1.5 },
            if live { accent } else { faint },
        )
    };
    let outline = Stroke::new(1.0, faint);
    let (base_side, base_top) = (
        visuals.widgets.noninteractive.bg_stroke.color,
        visuals.widgets.inactive.bg_fill,
    );
    let (cap_side, cap_top) = (
        visuals.widgets.active.bg_fill,
        visuals.widgets.hovered.bg_fill,
    );

    // The base, the ticks round it to read a twist against, and the ring the cap rests in.
    let base: Vec<Pos2> = ring(BASE_RADIUS, 0.0)
        .into_iter()
        .chain(ring(BASE_RADIUS, BASE_HEIGHT))
        .map(at)
        .collect();
    painter.add(Shape::convex_polygon(hull(base), base_side, outline));
    painter.add(Shape::convex_polygon(
        ring(BASE_RADIUS, BASE_HEIGHT).into_iter().map(at).collect(),
        base_top,
        outline,
    ));
    for i in 0..24 {
        let (sin, cos) = (i as f32 / 24.0 * std::f32::consts::TAU).sin_cos();
        let (inner, outer) = if i % 6 == 0 {
            (0.58, 0.76)
        } else {
            (0.62, 0.71)
        };
        painter.line_segment(
            [
                at([inner * cos, inner * sin, BASE_HEIGHT]),
                at([outer * cos, outer * sin, BASE_HEIGHT]),
            ],
            outline,
        );
    }
    let sliding = moving[0] || moving[1];
    painter.add(Shape::closed_line(
        ring(CAP_RADIUS + 0.03, BASE_HEIGHT)
            .into_iter()
            .map(at)
            .collect(),
        marker(sliding),
    ));
    // The light under the cap, showing in the gap that pulling up widens and pressing closes.
    let glow = ring(CAP_RADIUS - 0.04, BASE_HEIGHT)
        .into_iter()
        .map(|p| at(add(p, [pose.offset[0], pose.offset[1], 0.0])));
    painter.add(Shape::convex_polygon(
        glow.collect(),
        visuals.selection.bg_fill,
        Stroke::NONE,
    ));

    // The gauge: its notch is rest, its pointer where pulling up or pressing has the cap.
    let [gx, gh] = GAUGE;
    let gauge = |h: f32| at([gx, 0.0, h]);
    painter.line_segment(
        [gauge(gh - GAUGE_TRAVEL), gauge(gh + GAUGE_TRAVEL)],
        outline,
    );
    for h in [gh - GAUGE_TRAVEL, gh, gh + GAUGE_TRAVEL] {
        painter.line_segment(
            [
                gauge(h) - egui::vec2(3.0, 0.0),
                gauge(h) + egui::vec2(3.0, 0.0),
            ],
            outline,
        );
    }
    let lifting = moving[2];
    let pointer = gauge(gh + (pose.offset[2] - CAP_BOTTOM) / LIFT * GAUGE_TRAVEL);
    let tip = egui::vec2(-1.0, 0.0) * 4.0;
    painter.add(Shape::convex_polygon(
        vec![
            pointer + tip,
            pointer + egui::vec2(5.0, -5.0),
            pointer + egui::vec2(5.0, 5.0),
        ],
        if lifting { accent } else { faint },
        Stroke::NONE,
    ));

    // The cap: its side, the groove down its front where the side faces the user, then its top.
    let bottom = ring(CAP_RADIUS, 0.0);
    let top = ring(CAP_RADIUS, CAP_HEIGHT);
    let side: Vec<Pos2> = bottom
        .iter()
        .chain(&top)
        .map(|&p| at(pose.place(p)))
        .collect();
    painter.add(Shape::convex_polygon(hull(side), cap_side, outline));
    let twisting = moving[5];
    let front = apply(&pose.turn, [0.0, -1.0, 0.0]);
    if dot(front, eye()) > 0.0 {
        let groove = [0.1, CAP_HEIGHT - 0.1].map(|h| at(pose.place([0.0, -CAP_RADIUS, h])));
        painter.line_segment(groove, marker(twisting));
    }
    painter.add(Shape::convex_polygon(
        top.iter().map(|&p| at(pose.place(p))).collect(),
        cap_top,
        Stroke::new(1.5, faint),
    ));
    let centre = pose.place([0.0, 0.0, CAP_HEIGHT]);
    let pointer = pose.place([0.0, -0.36, CAP_HEIGHT]);
    painter.line_segment([at(centre), at(pointer)], marker(twisting));
    painter.circle_filled(at(pointer), 2.5, if twisting { accent } else { faint });

    // The mast stands on the cap's axis; the dashed upright is where it stands at rest.
    let tilting = moving[3] || moving[4];
    let rest = [0.0, 0.0, CAP_BOTTOM + CAP_HEIGHT];
    painter.extend(Shape::dashed_line(
        &[at(rest), at(add(rest, [0.0, 0.0, 0.5]))],
        Stroke::new(1.0, faint.gamma_multiply(0.6)),
        3.0,
        3.0,
    ));
    let mast = add(centre, scaled(apply(&pose.turn, [0.0, 0.0, 1.0]), 0.5));
    painter.line_segment([at(centre), at(mast)], marker(tilting));
    painter.circle_filled(at(mast), 3.0, if tilting { accent } else { faint });
}

/// One showing of a step's motion, in seconds: easing out to full deflection, holding there,
/// easing back, then resting before it starts again.
pub const DEMO_SECONDS: [f32; 4] = [1.5, 0.8, 0.7, 0.5];

/// How far through its travel a step's motion is, `seconds` after the step started (0 to 1).
/// It always starts at rest, so every step begins with the cap or ball still.
pub fn demo_swing(seconds: f32) -> f32 {
    let [out, hold, back, rest] = DEMO_SECONDS;
    let t = if seconds.is_finite() {
        seconds.max(0.0) % (out + hold + back + rest)
    } else {
        0.0
    };
    // Smoothstep: starts and stops gently.
    let ease = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    if t < out {
        ease(t / out)
    } else if t < out + hold {
        1.0
    } else {
        1.0 - ease((t - out - hold) / back)
    }
}

/// How a trackball moves for a reading on the canonical axes: rolled away from the user (-Y),
/// rolled right (+X), spun clockwise (+Rz), its scroll ring turned clockwise (+Z). The reading is a
/// speed, so this is too; pass it to [`ball::Ball::roll`].
pub fn ball_rate(travel: [f32; 6]) -> [f32; 4] {
    [-travel[1], travel[0], travel[5], travel[2]]
}

/// A level circle round the device's centre line.
fn ring(radius: f32, height: f32) -> Vec<P3> {
    (0..48)
        .map(|i| {
            let (sin, cos) = (i as f32 / 48.0 * std::f32::consts::TAU).sin_cos();
            [radius * cos, radius * sin, height]
        })
        .collect()
}

pub mod ball {
    use egui::{Pos2, Shape, Stroke};

    use super::{
        BASE_HEIGHT, BASE_RADIUS, P3, add, apply, dot, eye, hull, project, ring, rotate_x,
        rotate_y, rotate_z, scaled,
    };

    const RADIUS: f32 = 0.6;
    /// The ball's centre; its lower part sits down in the scroll ring.
    const CENTRE: P3 = [0.0, 0.0, RING_HEIGHT + 0.2];
    /// The dots on the ball, spread evenly over it.
    pub const DOTS: usize = 90;
    /// The scroll ring round the ball, inside and outside, and its notches.
    const RING: [f32; 2] = [0.58, 0.72];
    const RING_HEIGHT: f32 = BASE_HEIGHT + 0.05;
    const NOTCHES: usize = 24;

    /// A trackball: unlike a cap it doesn't spring back, so the ball's and the scroll ring's
    /// turns add up as they move.
    #[derive(Clone, Copy, Debug)]
    pub struct Ball {
        /// Columns of the ball's rotation.
        pub(crate) turn: [P3; 3],
        /// How far the scroll ring has turned, in radians, clockwise seen from above.
        pub(crate) wheel: f32,
    }

    impl Default for Ball {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Ball {
        pub fn new() -> Self {
            Self {
                turn: rotate_z(0.0),
                wheel: 0.0,
            }
        }

        /// Moves the ball for `seconds` at `rate` (turns per second at full deflection): rolled
        /// away from the user, rolled to the right, spun clockwise (seen from above), and the
        /// scroll ring turned clockwise.
        pub fn roll(&mut self, rate: [f32; 4], seconds: f32) {
            let [away, right, clockwise, wheel] = rate.map(|v| {
                if v.is_finite() {
                    v.clamp(-3.0, 3.0)
                } else {
                    0.0
                }
            });
            let seconds = if seconds.is_finite() {
                seconds.clamp(0.0, 0.1)
            } else {
                0.0
            };
            let turn = std::f32::consts::TAU * seconds;
            // Rolling away turns the top away (about right); rolling right turns it right (about away).
            let step = apply_all(
                &rotate_z(-clockwise * turn),
                apply_all(&rotate_y(right * turn), rotate_x(away * turn)),
            );
            self.turn = orthonormal(apply_all(&step, self.turn));
            self.wheel = (self.wheel + wheel * turn) % std::f32::consts::TAU;
        }

        /// Where dot `i` of the ball is now, as a direction from its centre.
        pub fn dot(&self, i: usize) -> P3 {
            // A Fibonacci sphere: even spacing without poles that crowd.
            let k = i as f32 + 0.5;
            let up = 1.0 - 2.0 * k / DOTS as f32;
            let round = (1.0 - up * up).max(0.0).sqrt();
            let (sin, cos) = (k * std::f32::consts::PI * (3.0 - 5.0_f32.sqrt())).sin_cos();
            apply(&self.turn, [round * cos, round * sin, up])
        }
    }

    /// Draws a trackball in `rect`: the ball in its scroll ring on the same base as the cap. The
    /// ball's dots slide as it rolls and spins and light while `rolling`; the ring's notches turn
    /// with it and light while `scrolling`.
    pub fn paint(ui: &egui::Ui, rect: egui::Rect, ball: &Ball, rolling: bool, scrolling: bool) {
        let painter = ui.painter_at(rect);
        let visuals = ui.visuals();
        let scale = (rect.width() / 2.9).min(rect.height() / 2.1);
        let origin = rect.center() + egui::vec2(-0.15, 0.4) * scale;
        let at = |p: P3| {
            let [x, y] = project(p);
            origin + egui::vec2(x, y) * scale
        };
        let (faint, accent) = (visuals.weak_text_color(), visuals.selection.stroke.color);
        let outline = Stroke::new(1.0, faint);
        let flat = |radius: f32, height: f32| {
            ring(radius, height)
                .into_iter()
                .map(at)
                .collect::<Vec<Pos2>>()
        };
        let base: Vec<Pos2> = ring(BASE_RADIUS, 0.0)
            .into_iter()
            .chain(ring(BASE_RADIUS, BASE_HEIGHT))
            .map(at)
            .collect();
        painter.add(Shape::convex_polygon(
            hull(base),
            visuals.widgets.noninteractive.bg_stroke.color,
            outline,
        ));
        painter.add(Shape::convex_polygon(
            flat(BASE_RADIUS, BASE_HEIGHT),
            visuals.widgets.inactive.bg_fill,
            outline,
        ));

        // The scroll ring: raised a little off the base, its notches turned by the wheel.
        let [inner, outer] = RING;
        let raised: Vec<Pos2> = ring(outer, BASE_HEIGHT)
            .into_iter()
            .chain(ring(outer, RING_HEIGHT))
            .map(at)
            .collect();
        painter.add(Shape::convex_polygon(
            hull(raised),
            visuals.widgets.active.bg_fill,
            outline,
        ));
        painter.add(Shape::convex_polygon(
            flat(outer, RING_HEIGHT),
            visuals.widgets.hovered.bg_fill,
            outline,
        ));
        let notch = Stroke::new(
            if scrolling { 2.0 } else { 1.0 },
            if scrolling { accent } else { faint },
        );
        let (ring_top, ring_side) = (
            visuals.widgets.hovered.bg_fill,
            visuals.widgets.active.bg_fill,
        );
        // A band between two circles (radius, height), from ring point `from` to `to` (of 48).
        let band = |from: usize, to: usize, [ra, ha]: [f32; 2], [rb, hb]: [f32; 2], colour| {
            let mut mesh = egui::Mesh::default();
            for i in from..=to {
                let (sin, cos) = (i as f32 / 48.0 * std::f32::consts::TAU).sin_cos();
                mesh.colored_vertex(at([ra * cos, ra * sin, ha]), colour);
                mesh.colored_vertex(at([rb * cos, rb * sin, hb]), colour);
            }
            for k in 0..(to - from) as u32 {
                mesh.add_triangle(2 * k, 2 * k + 1, 2 * k + 2);
                mesh.add_triangle(2 * k + 1, 2 * k + 3, 2 * k + 2);
            }
            painter.add(Shape::mesh(mesh));
        };
        // Half the ring's top, from ring point `from` to `to`, with its notches.
        let half = |from: usize, to: usize, front: bool| {
            band(
                from,
                to,
                [inner, RING_HEIGHT],
                [outer, RING_HEIGHT],
                ring_top,
            );
            for i in 0..NOTCHES {
                let (sin, cos) =
                    (i as f32 / NOTCHES as f32 * std::f32::consts::TAU - ball.wheel).sin_cos();
                if (sin < 0.0) == front {
                    let (a, b) = (inner + 0.03, outer - 0.03);
                    painter.line_segment(
                        [
                            at([a * cos, a * sin, RING_HEIGHT]),
                            at([b * cos, b * sin, RING_HEIGHT]),
                        ],
                        notch,
                    );
                }
            }
        };
        // The far half and the hole go behind the ball, the near half in front of it, so the
        // ball sits down in the ring.
        half(0, 24, false);
        painter.add(Shape::closed_line(flat(outer, RING_HEIGHT), outline));
        painter.add(Shape::convex_polygon(
            flat(inner, RING_HEIGHT),
            visuals.extreme_bg_color,
            outline,
        ));

        // Seen straight along any line, a sphere is a circle.
        painter.circle(
            at(CENTRE),
            RADIUS * scale,
            visuals.widgets.active.bg_fill,
            outline,
        );
        let colour = if rolling { accent } else { faint };
        for i in 0..DOTS {
            let d = ball.dot(i);
            // Only the near half shows; dots shrink and fade toward the edge, which reads as round.
            let facing = dot(d, eye());
            if facing <= 0.05 {
                continue;
            }
            let p = at(add(CENTRE, scaled(d, RADIUS)));
            painter.circle_filled(
                p,
                scale * 0.045 * (0.35 + 0.65 * facing),
                colour.gamma_multiply(0.35 + 0.65 * facing),
            );
        }
        // Everything nearer than the ball, base and ring, over the ball's lower part.
        let (base_side, base_top) = (
            visuals.widgets.noninteractive.bg_stroke.color,
            visuals.widgets.inactive.bg_fill,
        );
        band(
            24,
            48,
            [BASE_RADIUS, 0.0],
            [BASE_RADIUS, BASE_HEIGHT],
            base_side,
        );
        band(
            24,
            48,
            [outer, BASE_HEIGHT],
            [BASE_RADIUS, BASE_HEIGHT],
            base_top,
        );
        band(
            24,
            48,
            [outer, BASE_HEIGHT],
            [outer, RING_HEIGHT],
            ring_side,
        );
        half(24, 48, true);
        let edges = [
            [inner, RING_HEIGHT],
            [outer, RING_HEIGHT],
            [outer, BASE_HEIGHT],
            [BASE_RADIUS, BASE_HEIGHT],
            [BASE_RADIUS, 0.0],
        ];
        for [radius, height] in edges {
            let edge = ring(radius, height);
            painter.add(Shape::line(
                edge.iter()
                    .skip(24)
                    .chain(edge.first())
                    .map(|&p| at(p))
                    .collect(),
                outline,
            ));
        }
    }

    /// `a` after `b`, as rotations.
    fn apply_all(a: &[P3; 3], b: [P3; 3]) -> [P3; 3] {
        b.map(|column| apply(a, column))
    }

    /// `m` put back to a pure rotation, so many small turns don't skew or shrink it.
    fn orthonormal(m: [P3; 3]) -> [P3; 3] {
        let unit = |v: P3| {
            let length = dot(v, v).sqrt();
            if length > 1e-6 && length.is_finite() {
                scaled(v, 1.0 / length)
            } else {
                [0.0; 3]
            }
        };
        let x = unit(m[0]);
        let y = unit(add(m[1], scaled(x, -dot(m[1], x))));
        let z = [
            x[1] * y[2] - x[2] * y[1],
            x[2] * y[0] - x[0] * y[2],
            x[0] * y[1] - x[1] * y[0],
        ];
        if dot(z, z) < 0.5 {
            rotate_z(0.0)
        } else {
            [x, y, z]
        }
    }
}

/// The convex hull of `points`, in order round it (Andrew's monotone chain).
fn hull(mut points: Vec<Pos2>) -> Vec<Pos2> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    let cross = |o: Pos2, a: Pos2, b: Pos2| (a - o).x * (b - o).y - (a - o).y * (b - o).x;
    let mut out: Vec<Pos2> = Vec::with_capacity(points.len() + 1);
    for pass in [false, true] {
        let start = out.len();
        let ordered: Box<dyn Iterator<Item = &Pos2>> = if pass {
            Box::new(points.iter().rev())
        } else {
            Box::new(points.iter())
        };
        for &p in ordered {
            while out.len() >= start + 2 && cross(out[out.len() - 2], out[out.len() - 1], p) <= 0.0
            {
                out.pop();
            }
            out.push(p);
        }
        out.pop();
    }
    out
}

fn unit(i: usize) -> P3 {
    let mut v = [0.0; 3];
    v[i] = 1.0;
    v
}

fn apply(columns: &[P3; 3], p: P3) -> P3 {
    [0, 1, 2].map(|row| columns[0][row] * p[0] + columns[1][row] * p[1] + columns[2][row] * p[2])
}

fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scaled(a: P3, k: f32) -> P3 {
    a.map(|v| v * k)
}

fn dot(a: P3, b: P3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Turns about right: positive takes up toward away.
fn rotate_x(angle: f32) -> [P3; 3] {
    let (sin, cos) = angle.sin_cos();
    [[1.0, 0.0, 0.0], [0.0, cos, -sin], [0.0, sin, cos]]
}

/// Turns about away: positive takes up toward right.
fn rotate_y(angle: f32) -> [P3; 3] {
    let (sin, cos) = angle.sin_cos();
    [[cos, 0.0, -sin], [0.0, 1.0, 0.0], [sin, 0.0, cos]]
}

/// Turns about up: positive is anticlockwise seen from above.
fn rotate_z(angle: f32) -> [P3; 3] {
    let (sin, cos) = angle.sin_cos();
    [[cos, sin, 0.0], [-sin, cos, 0.0], [0.0, 0.0, 1.0]]
}

#[cfg(test)]
mod tests {
    use super::ball::{Ball, DOTS};
    use super::*;

    fn reading(axis: usize, value: f32) -> [f32; 6] {
        let mut travel = [0.0; 6];
        travel[axis] = value;
        travel
    }

    /// Where a point on the cap lands on screen for a reading.
    fn on_screen(travel: [f32; 6], p: P3) -> [f32; 2] {
        project(Pose::new(travel).place(p))
    }

    const TOP_CENTRE: P3 = [0.0, 0.0, CAP_HEIGHT];
    const FRONT: P3 = [0.0, -CAP_RADIUS, CAP_HEIGHT / 2.0];

    #[test]
    fn the_cap_moves_the_way_the_reading_says() {
        let rest = on_screen([0.0; 6], TOP_CENTRE);
        // Slid right: right on screen.
        assert!(on_screen(reading(0, 1.0), TOP_CENTRE)[0] > rest[0]);
        // Pressed down: lower on screen; pulled up: higher.
        assert!(on_screen(reading(2, 1.0), TOP_CENTRE)[1] > rest[1]);
        assert!(on_screen(reading(2, -1.0), TOP_CENTRE)[1] < rest[1]);
        // Pulled toward the user: nearer, so lower on screen.
        assert!(Pose::new(reading(1, 1.0)).place(TOP_CENTRE)[1] < 0.0);
    }

    #[test]
    fn tilts_and_twists_turn_the_cap_the_right_way() {
        let axis = |travel| apply(&Pose::new(travel).turn, [0.0, 0.0, 1.0]);
        assert!((axis([0.0; 6])[2] - 1.0).abs() < 1e-6);
        // Tilted back: the top leans away from the user; tilted left: to the left.
        assert!(axis(reading(3, 1.0))[1] > 0.1);
        assert!(axis(reading(4, 1.0))[0] < -0.1);
        // Twisted clockwise (from above): the groove on the front swings to the user's left.
        let rest = Pose::new([0.0; 6]).place(FRONT);
        let twisted = Pose::new(reading(5, 1.0)).place(FRONT);
        assert!(twisted[0] < rest[0] - 0.1);
        assert!(
            (twisted[2] - rest[2]).abs() < 1e-6,
            "a twist alone keeps the cap's height"
        );
    }

    #[test]
    fn hostile_readings_stay_finite() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1e30] {
            let pose = Pose::new([value; 6]);
            assert!(pose.place(TOP_CENTRE).iter().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn the_hull_keeps_only_the_outline() {
        let square = [
            (0.0, 0.0),
            (2.0, 0.0),
            (2.0, 2.0),
            (0.0, 2.0),
            (1.0, 1.0),
            (1.0, 0.0),
        ];
        let hull = hull(square.iter().map(|&(x, y)| Pos2::new(x, y)).collect());
        assert_eq!(hull.len(), 4);
        assert!(!hull.contains(&Pos2::new(1.0, 1.0)));
        assert!(super::hull(Vec::new()).is_empty());
    }

    #[test]
    fn the_ball_rolls_the_way_it_is_pushed() {
        let top_after = |rate: [f32; 3]| {
            let mut ball = Ball::new();
            // A quarter turn, in small steps as frames give it.
            for _ in 0..25 {
                ball.roll([rate[0], rate[1], rate[2], 0.0], 0.01);
            }
            apply(&ball.turn, [0.0, 0.0, 1.0])
        };
        assert!(
            top_after([1.0, 0.0, 0.0])[1] > 0.9,
            "rolled away: the top goes away"
        );
        assert!(
            top_after([0.0, 1.0, 0.0])[0] > 0.9,
            "rolled right: the top goes right"
        );
        let mut ball = Ball::new();
        for _ in 0..25 {
            ball.roll([0.0, 0.0, 1.0, 0.0], 0.01);
        }
        // Spun clockwise from above: what faced the user turns to their left.
        assert!(apply(&ball.turn, [0.0, -1.0, 0.0])[0] < -0.9);
    }

    #[test]
    fn the_ball_stays_a_rotation() {
        let mut ball = Ball::new();
        for i in 0..20_000 {
            ball.roll([0.7, -0.3, (i % 7) as f32 * 0.2, 0.5], 0.016);
        }
        for (i, a) in ball.turn.iter().enumerate() {
            for (j, b) in ball.turn.iter().enumerate() {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((dot(*a, *b) - expected).abs() < 1e-4);
            }
        }
        for value in [f32::NAN, f32::INFINITY, -1e30] {
            ball.roll([value; 4], value);
            assert!(ball.turn.iter().flatten().all(|v| v.is_finite()) && ball.wheel.is_finite());
            assert!((0..DOTS).all(|i| ball.dot(i).iter().all(|v| v.is_finite())));
        }
    }

    #[test]
    fn the_scroll_ring_turns_and_keeps_its_turn() {
        let mut ball = Ball::new();
        for _ in 0..10 {
            ball.roll([0.0, 0.0, 0.0, 1.0], 0.01);
        }
        let turned = ball.wheel;
        assert!((turned - std::f32::consts::TAU * 0.1).abs() < 1e-4);
        // Let go: a scroll ring stays where it was turned to, and the ball doesn't move with it.
        ball.roll([0.0; 4], 0.01);
        assert_eq!(ball.wheel, turned);
        assert!((apply(&ball.turn, [0.0, 0.0, 1.0])[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn each_step_starts_at_rest_and_eases_out_to_full() {
        let [out, hold, back, rest] = DEMO_SECONDS;
        assert_eq!(demo_swing(0.0), 0.0);
        assert!((demo_swing(out) - 1.0).abs() < 1e-6);
        assert_eq!(demo_swing(out + hold / 2.0), 1.0);
        assert_eq!(demo_swing(out + hold + back + rest / 2.0), 0.0);
        // Gently: it creeps away from rest rather than jumping, and never goes back on itself on the way out.
        assert!(demo_swing(out * 0.05) < 0.01);
        let samples: Vec<f32> = (0..=100)
            .map(|i| demo_swing(out * i as f32 / 100.0))
            .collect();
        assert!(samples.windows(2).all(|w| w[1] >= w[0]));
        // Again from rest after a full showing; nothing odd for odd times.
        assert_eq!(demo_swing(out + hold + back + rest), 0.0);
        for seconds in [f32::NAN, f32::INFINITY, -5.0] {
            assert_eq!(demo_swing(seconds), 0.0);
        }
    }
}
