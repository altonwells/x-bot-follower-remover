//! Original, display-only fly/brain artwork. No biological model or decision logic.
use std::sync::OnceLock;

use crate::theme::*;
use ratatui::{Frame, layout::Rect, style::Color};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Collect,
    Inspect,
    Remove,
    Rest,
    Waiting,
    Reconcile,
    Paused,
    Disconnected,
    Blocked,
    Still,
}
impl Motion {
    pub fn from_work(active: bool, waiting: bool, phase: &str, action: &str) -> Self {
        if !active {
            Self::Still
        } else if waiting {
            Self::Rest
        } else if action == "Removing" {
            Self::Remove
        } else if action == "Reconciling" {
            Self::Reconcile
        } else if matches!(action, "Checking" | "Checking activity") {
            Self::Inspect
        } else if matches!(phase, "following" | "followers") {
            Self::Collect
        } else if phase == "inspect" {
            Self::Inspect
        } else {
            Self::Waiting
        }
    }
    pub fn from_status(status: &crate::background::Status) -> Self {
        if matches!(
            status.state.as_str(),
            "Waiting for Chrome" | "Checking account"
        ) {
            Self::Disconnected
        } else if status.paused && status.has_job {
            Self::Paused
        } else if status
            .working
            .as_ref()
            .is_some_and(|(action, _)| action == "Reconciling")
            && status.wait_seconds == 0
        {
            Self::Reconcile
        } else if status.state == "Needs reconciliation" {
            Self::Blocked
        } else {
            Self::from_work(
                status.has_job,
                status.wait_seconds > 0,
                &status.phase,
                status.working.as_ref().map_or("", |(action, _)| action),
            )
        }
    }
    pub fn animating(self) -> bool {
        matches!(
            self,
            Self::Collect
                | Self::Inspect
                | Self::Remove
                | Self::Reconcile
                | Self::Rest
                | Self::Waiting
        )
    }
    fn label(self) -> &'static str {
        match self {
            Self::Collect => "COLLECTING",
            Self::Inspect => "CHECKING",
            Self::Remove => "REMOVING",
            Self::Rest => "COOLDOWN",
            Self::Waiting => "WAITING",
            Self::Reconcile => "RECONCILING",
            Self::Paused => "PAUSED",
            Self::Disconnected => "DISCONNECTED",
            Self::Blocked => "NEEDS REVIEW",
            Self::Still => "IDLE",
        }
    }
}
#[derive(Clone, Copy)]
struct Point {
    x: f32,
    y: f32,
    z: f32,
    part: u8,
    normal: [f32; 3],
}
// Shapes are sampled once. Animation only projects the same bounded point sets.
fn ellipsoid(
    points: &mut Vec<Point>,
    center: [f32; 3],
    radii: [f32; 3],
    count: usize,
    part: u8,
    tilt: f32,
) {
    for i in 0..count {
        let z = 1.0 - 2.0 * (i as f32 + 0.5) / count as f32;
        let radius = (1.0 - z * z).sqrt();
        let angle = i as f32 * 2.399_963_1;
        let x = radii[0] * radius * angle.cos();
        let y = radii[1] * radius * angle.sin();
        let (sin, cos) = tilt.sin_cos();
        let n = [x / radii[0].powi(2), y / radii[1].powi(2), z / radii[2]];
        let length = n.iter().map(|v| v * v).sum::<f32>().sqrt();
        let normal = [
            (n[0] * cos - n[1] * sin) / length,
            (n[0] * sin + n[1] * cos) / length,
            n[2] / length,
        ];
        points.push(Point {
            x: center[0] + x * cos - y * sin,
            y: center[1] + x * sin + y * cos,
            z: center[2] + radii[2] * z,
            part,
            normal,
        });
    }
}
struct BrainPoint {
    position: [f32; 3],
    normal: [f32; 3],
    clusters: [f32; 8],
}
// Original illustrative regions, not a measured connectome.
const CLUSTERS: [[f32; 3]; 8] = [
    [-0.94, 0.04, 0.20],
    [-0.53, -0.20, 0.28],
    [-0.23, -0.29, 0.31],
    [-0.17, 0.25, 0.17],
    [0.20, 0.23, 0.19],
    [0.28, -0.29, 0.30],
    [0.59, -0.13, 0.26],
    [0.97, 0.09, 0.17],
];
fn brain_points() -> &'static [BrainPoint] {
    static POINTS: OnceLock<Vec<BrainPoint>> = OnceLock::new();
    POINTS.get_or_init(|| {
        let mut lobes = Vec::new();
        for side in [-1.0, 1.0] {
            lobes.extend([
                ([side * 0.87, 0.05, 0.0], [0.37, 0.35, 0.34], 4000),
                ([side * 0.32, -0.14, 0.03], [0.36, 0.40, 0.42], 5000),
                ([side * 0.17, 0.25, 0.02], [0.19, 0.22, 0.25], 1800),
            ]);
        }
        let mut points = Vec::with_capacity(21600);
        for &(center, radii, count) in &lobes {
            let mut shell = Vec::with_capacity(count);
            ellipsoid(&mut shell, center, radii, count, 0, 0.0);
            for p in shell {
                let position = [p.x, p.y, p.z];
                // Hide surfaces inside another lobe, leaving a continuous outer shell.
                if lobes.iter().any(|&(other, r, _)| {
                    other != center
                        && (0..3)
                            .map(|i| ((position[i] - other[i]) / r[i]).powi(2))
                            .sum::<f32>()
                            < 0.98
                }) {
                    continue;
                }
                let normal = p.normal;
                // Fine surface relief catches the light as the brain turns.
                let relief =
                    (p.x * 35.0 + p.z * 17.0).sin() * (p.y * 29.0 - p.z * 13.0).sin() * 0.012;
                let position = std::array::from_fn(|i| position[i] + normal[i] * relief);
                let clusters = CLUSTERS.map(|center| {
                    let distance = (0..3)
                        .map(|i| (position[i] - center[i]).powi(2))
                        .sum::<f32>();
                    (1.0 - distance / 0.115).max(0.0).powi(2)
                });
                points.push(BrainPoint {
                    position,
                    normal,
                    clusters,
                });
            }
        }
        points
    })
}
fn fly_points() -> &'static [Point] {
    static POINTS: OnceLock<Vec<Point>> = OnceLock::new();
    POINTS.get_or_init(|| {
        let mut p = Vec::with_capacity(18000);
        // Overlapping abdominal segments taper toward the rear.
        for (x, radius) in [(-0.62, 0.09), (-0.50, 0.14), (-0.36, 0.18), (-0.20, 0.19)] {
            ellipsoid(
                &mut p,
                [x, 0.06, 0.02],
                [0.15, radius, radius * 0.85],
                1700,
                0,
                -0.08,
            );
        }
        ellipsoid(
            &mut p,
            [0.06, -0.06, 0.0],
            [0.25, 0.24, 0.21],
            3400,
            1,
            -0.12,
        );
        ellipsoid(
            &mut p,
            [0.40, -0.12, 0.04],
            [0.20, 0.20, 0.19],
            2200,
            2,
            -0.1,
        );
        for side in [-1.0, 1.0] {
            ellipsoid(
                &mut p,
                [0.46, -0.11, side * 0.20],
                [0.095, 0.155, 0.035],
                1800,
                3,
                0.15,
            );
            // Two thin, separate wing membranes extend out from the thorax.
            ellipsoid(
                &mut p,
                [-0.28, -0.26, side * 0.25],
                [0.51, 0.018, 0.22],
                1000,
                4,
                0.12,
            );
        }
        p
    })
}
struct Raster {
    area: Rect,
    dots: Vec<u8>,
    light: Vec<u8>,
    depth: Vec<f32>,
    scale: f32,
    ascii: bool,
}
impl Raster {
    fn new(area: Rect, aspect_width: f32, aspect_height: f32, ascii: bool) -> Self {
        let area = centered(area, 140, 40);
        Self {
            area,
            dots: vec![0; area.width as usize * area.height as usize],
            light: vec![0; area.width as usize * area.height as usize * 8],
            depth: vec![f32::NEG_INFINITY; area.width as usize * area.height as usize * 8],
            scale: (f32::from(area.width) * 2.0 / aspect_width)
                .min(f32::from(area.height) * 4.0 / aspect_height),
            ascii,
        }
    }
    fn depth_dot(&mut self, x: f32, y: f32, depth: f32, intensity: u8) {
        let x = (f32::from(self.area.width) + x * self.scale).round() as i32;
        let y = (f32::from(self.area.height) * 2.0 + y * self.scale).round() as i32;
        if x < 0
            || y < 0
            || x >= i32::from(self.area.width) * 2
            || y >= i32::from(self.area.height) * 4
        {
            return;
        }
        let index = (y as usize / 4) * self.area.width as usize + x as usize / 2;
        let bits = [[0, 3], [1, 4], [2, 5], [6, 7]];
        let bit = bits[y as usize % 4][x as usize % 2];
        let pixel = index * 8 + bit;
        if depth < self.depth[pixel] {
            return;
        }
        self.dots[index] |= 1 << bit;
        self.light[pixel] = if depth == self.depth[pixel] {
            self.light[pixel].max(intensity)
        } else {
            intensity
        };
        self.depth[pixel] = depth;
    }
    fn paint(&self, frame: &mut Frame, still: bool) {
        for (i, &bits) in self.dots.iter().enumerate() {
            if bits == 0 {
                continue;
            }
            let symbol = if self.ascii {
                ['.', ':', '*', 'o', '#'][(bits.count_ones() as usize / 2).min(4)]
            } else {
                char::from_u32(0x2800 + u32::from(bits)).unwrap()
            };
            let light = *self.light[i * 8..i * 8 + 8].iter().max().unwrap();
            let color = if still {
                match light {
                    0 => Color::Rgb(36, 55, 64),
                    1 => Color::Rgb(65, 84, 94),
                    _ => MUTED,
                }
            } else {
                match light {
                    0 => Color::Rgb(62, 94, 108),
                    1 => MUTED,
                    2 => ICE,
                    3 => MINT,
                    5 => AMBER,
                    _ => TEXT,
                }
            };
            frame.buffer_mut()[(
                self.area.x + i as u16 % self.area.width,
                self.area.y + (i / self.area.width as usize) as u16,
            )]
                .set_char(symbol)
                .set_fg(color);
        }
    }
}
struct Camera {
    yaw: (f32, f32),
    pitch: (f32, f32),
}
impl Camera {
    fn brain(time: f32) -> Self {
        Self {
            yaw: (0.38 + (time * 0.19).sin() * 0.46).sin_cos(),
            pitch: (-0.22 + (time * 0.13).sin() * 0.10).sin_cos(),
        }
    }
    fn fly(time: f32) -> Self {
        Self {
            yaw: (-0.42 + (time * 0.16).sin() * 0.24).sin_cos(),
            pitch: (0.40 + (time * 0.11).sin() * 0.06).sin_cos(),
        }
    }
    fn rotate(&self, [x, y, z]: [f32; 3]) -> [f32; 3] {
        let (sy, cy) = self.yaw;
        let (sx, cx) = self.pitch;
        let x1 = x * cy + z * sy;
        let z1 = z * cy - x * sy;
        [x1, y * cx - z1 * sx, y * sx + z1 * cx]
    }
    fn project(&self, point: [f32; 3]) -> [f32; 3] {
        let [x, y, z] = self.rotate(point);
        let perspective = 3.8 / (3.8 - z);
        [x * perspective, y * perspective, z]
    }
}
fn cluster_activation(time: f32, motion: Motion, burst: f32) -> [f32; 8] {
    if !motion.animating() {
        return [0.0; 8];
    }
    let (weights, speed): ([f32; 8], f32) = match motion {
        Motion::Collect => ([1.0, 0.65, 0.0, 0.0, 0.0, 0.0, 0.65, 1.0], 1.1),
        Motion::Inspect => ([0.0, 0.3, 1.0, 0.0, 0.0, 1.0, 0.3, 0.0], 1.5),
        Motion::Remove => ([0.0, 0.0, 0.25, 1.0, 1.0, 0.25, 0.0, 0.0], 2.4),
        Motion::Reconcile => ([0.0, 0.65, 1.0, 0.0, 0.0, 1.0, 0.65, 0.0], 0.7),
        _ => ([0.10; 8], 0.25),
    };
    std::array::from_fn(|i| {
        let phase = (time * speed - i as f32 * 0.48).rem_euclid(4.8);
        let pulse = (1.0 - ((phase - 0.65) / 0.65).abs()).max(0.0) * weights[i];
        // A broad response is reserved for an actual confirmed-removal receipt.
        let receipt = if burst > 0.0 {
            (1.0 - (((1.0 - burst) * 2.0 - i as f32 * 0.16) / 0.5).abs()).max(0.0)
        } else {
            0.0
        };
        pulse.max(receipt)
    })
}

fn brain(frame: &mut Frame, area: Rect, time: f32, motion: Motion, burst: f32, ascii: bool) {
    let mut raster = Raster::new(area, 2.7, 1.4, ascii);
    let camera = Camera::brain(time);
    let activation = cluster_activation(time, motion, burst);
    for (i, p) in brain_points().iter().enumerate() {
        let normal = camera.rotate(p.normal);
        if normal[2] < -0.1 {
            continue;
        }
        let [x, y, z] = camera.project(p.position);
        let energy = p
            .clusters
            .iter()
            .zip(activation)
            .map(|(weight, pulse)| weight * pulse)
            .fold(0.0_f32, f32::max);
        let illumination = (-normal[0] * 0.35 - normal[1] * 0.45 + normal[2] * 0.75).max(0.0);
        // Local, discrete glints inside each active cluster; no full-width scan band.
        let spark = (i as u64 * 17 + (time * 12.0) as u64).is_multiple_of(11);
        let intensity = if energy > 0.30 && spark {
            4
        } else if energy > 0.08 {
            3
        } else if illumination > 0.78 {
            2
        } else if illumination > 0.36 {
            1
        } else {
            0
        };
        raster.depth_dot(x, y, z, intensity);
    }
    // Short surface pathways connect the illustrative regions in firing order.
    for (i, pair) in CLUSTERS.windows(2).enumerate() {
        for step in 0..32 {
            let t = step as f32 / 31.0;
            let energy = activation[i] * (1.0 - t) + activation[i + 1] * t;
            if energy > 0.12 {
                let mut point =
                    std::array::from_fn(|axis| pair[0][axis] * (1.0 - t) + pair[1][axis] * t);
                point[2] += (t * std::f32::consts::PI).sin() * 0.10;
                let [x, y, z] = camera.project(point);
                raster.depth_dot(x, y, z, if energy > 0.5 { 3 } else { 1 });
            }
        }
    }
    raster.paint(frame, !motion.animating());
}
fn fly(frame: &mut Frame, area: Rect, time: f32, motion: Motion, burst: f32, ascii: bool) {
    let mut raster = Raster::new(area, 2.8, 1.6, ascii);
    let camera = Camera::fly(time);
    let walking = motion == Motion::Collect;
    let flying = motion == Motion::Remove || (burst > 0.0 && motion.animating());
    let swing = if walking { time * 5.0 } else { 0.0 };
    let bob = if flying {
        -0.055
    } else if walking {
        swing.sin() * 0.015
    } else {
        0.0
    };
    let flap = if flying {
        0.35 + (time * 18.0).sin() * 0.65
    } else {
        0.12
    };
    let (wing_sin, wing_cos) = flap.sin_cos();
    let wing_rotate = |[x, y, z]: [f32; 3], side: f32| {
        [
            x,
            y * wing_cos - z * side * wing_sin,
            y * side * wing_sin + z * wing_cos,
        ]
    };
    let wing_position = |[x, y, z]: [f32; 3]| {
        let side = z.signum();
        let [x, y, z] = wing_rotate([x - 0.1, y + 0.16, z - side * 0.08], side);
        [x + 0.1, y - 0.16, z + side * 0.08]
    };
    let project = |[x, y, z]: [f32; 3]| {
        let [x, y, z] = camera.project([x, y + bob, z]);
        [x * 1.4, y - 0.07, z]
    };
    for p in fly_points() {
        let mut position = [p.x, p.y, p.z];
        let mut normal = p.normal;
        if p.part == 4 {
            position = wing_position(position);
            normal = wing_rotate(normal, p.z.signum());
        }
        let normal = camera.rotate(normal);
        if p.part != 4 && normal[2] < 0.0 {
            continue;
        }
        let illumination = (-normal[0] * 0.30 - normal[1] * 0.45 + normal[2] * 0.80).max(0.0);
        let light = if p.part == 3 {
            if illumination > 0.7
                && ((p.x * 130.0) as i32 + (p.y * 130.0) as i32).rem_euclid(3) == 0
            {
                4
            } else {
                5
            }
        } else if p.part == 4 || (p.part == 0 && ((p.x + 0.7) * 25.0).rem_euclid(4.0) < 0.6) {
            0
        } else if illumination > 0.78 {
            2
        } else if illumination > 0.36 {
            1
        } else {
            0
        };
        let [x, y, z] = project(position);
        raster.depth_dot(x, y, z, light);
    }
    // Every appendage is projected in the same camera and depth buffer as the body.
    let line = |raster: &mut Raster, from: [f32; 3], to: [f32; 3], light| {
        for i in 0..48 {
            let t = i as f32 / 47.0;
            let [x, y, z] = project(std::array::from_fn(|axis| {
                from[axis] * (1.0 - t) + to[axis] * t
            }));
            raster.depth_dot(x, y, z, light);
        }
    };
    for side in [-1.0, 1.0] {
        let root = wing_position([0.10, -0.18, side * 0.08]);
        for tip in [
            [-0.77, -0.29, side * 0.24],
            [-0.61, -0.27, side * 0.43],
            [-0.35, -0.24, side * 0.45],
        ] {
            line(&mut raster, root, wing_position(tip), 1);
        }
        // Opposite tripod phases, with real near/far-side separation.
        for leg in 0..3 {
            let step = if walking {
                (swing + (leg as f32 + side.max(0.0)) * std::f32::consts::PI).sin()
            } else {
                0.0
            };
            let base_x = 0.19 - leg as f32 * 0.15;
            let reach = 0.58 - leg as f32 * 0.53;
            let lift = if walking {
                step.max(0.0) * 0.09
            } else if flying {
                0.08
            } else {
                0.0
            };
            let knee = [reach * 0.6, 0.30, side * 0.29];
            let foot = [reach + step * 0.09, 0.60 - lift, side * 0.40];
            line(&mut raster, [base_x, 0.10, side * 0.12], knee, 1);
            line(&mut raster, knee, foot, 2);
            line(&mut raster, foot, [foot[0] + 0.06, foot[1], foot[2]], 1);
            for bristle in 1..4 {
                let t = bristle as f32 / 4.0;
                let point: [f32; 3] =
                    std::array::from_fn(|axis| knee[axis] + (foot[axis] - knee[axis]) * t);
                line(
                    &mut raster,
                    point,
                    [point[0] - 0.025, point[1] - 0.018, point[2] + side * 0.018],
                    1,
                );
            }
        }
        let feel = if matches!(motion, Motion::Inspect | Motion::Reconcile) {
            (time * if motion == Motion::Inspect { 6.0 } else { 2.0 } + side).sin() * 0.08
        } else {
            0.0
        };
        line(
            &mut raster,
            [0.51, -0.17, side * 0.08],
            [0.70, -0.28 + feel, side * 0.22],
            2,
        );
        line(
            &mut raster,
            [0.55, -0.02, side * 0.05],
            [0.62, 0.11, side * 0.04],
            1,
        );
        for i in 0..14 {
            let x = -0.12 + i as f32 * 0.025;
            let y = -0.27 - (1.0 - ((x - 0.05) / 0.25).powi(2)).max(0.0).sqrt() * 0.045;
            line(
                &mut raster,
                [x, y, side * 0.08],
                [x - 0.025, y - 0.055, side * 0.10],
                1,
            );
        }
    }
    // A perspective ground reference makes the camera angle legible.
    for side in [-1.0, 1.0] {
        for i in 0..44 {
            let [x, y, z] = project([-0.94 + i as f32 * 0.044, 0.62 - bob, side * 0.45]);
            raster.depth_dot(x, y, z, 0);
        }
    }
    raster.paint(frame, !motion.animating());
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    motion: Motion,
    clock_ms: u64,
    departure_age: Option<u64>,
) {
    let [upper, lower] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Percentage(53),
        ratatui::layout::Constraint::Percentage(47),
    ])
    .areas(area);
    let brain_block = panel(format!(" BRAIN / VISUALIZATION · {} ", motion.label()));
    let brain_area = brain_block.inner(upper);
    frame.render_widget(brain_block, upper);
    let fly_block = panel(format!(" FLY 01 / {} ", motion.label()));
    let fly_area = fly_block.inner(lower);
    frame.render_widget(fly_block, lower);
    // Explicit opt-in fallback for terminals without Braille glyphs.
    static ASCII: OnceLock<bool> = OnceLock::new();
    let ascii = *ASCII.get_or_init(|| std::env::var_os("REMOVER_ASCII").is_some());
    let time = clock_ms as f32 / 1000.0;
    let burst = departure_age
        .filter(|age| *age < 1400)
        .map_or(0.0, |age| 1.0 - age as f32 / 1400.0);
    brain(frame, brain_area, time, motion, burst, ascii);
    fly(frame, fly_area, time, motion, burst, ascii);
    // A small event pulse travels between the two figures, inside their panels.
    if motion.animating() && !matches!(motion, Motion::Rest | Motion::Waiting) && area.height >= 22
    {
        let x = area.x + area.width / 2;
        for y in [
            upper.bottom() - 3,
            upper.bottom() - 2,
            lower.y + 1,
            lower.y + 2,
        ] {
            let lit = (u64::from(y) + clock_ms / 100).is_multiple_of(4);
            frame.buffer_mut()[(x, y)]
                .set_char(if lit { '•' } else { '·' })
                .set_fg(if lit { MINT } else { BORDER });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
    #[test]
    fn nearer_surface_hides_activation_on_the_far_surface() {
        for far_first in [true, false] {
            let mut raster = Raster::new(Rect::new(0, 0, 20, 10), 2.8, 1.65, false);
            let samples = if far_first {
                [(0.1, 4), (0.4, 1)]
            } else {
                [(0.4, 1), (0.1, 4)]
            };
            for (z, light) in samples {
                raster.depth_dot(0.0, 0.0, z, light);
            }
            assert_eq!(raster.light.iter().copied().max(), Some(1));
        }
    }
    #[test]
    fn activation_stays_local_and_projection_changes_depth() {
        for step in 0..100 {
            let activation = cluster_activation(step as f32 * 0.1, Motion::Inspect, 0.0);
            assert!(activation.iter().filter(|&&v| v > 0.0).count() <= 3);
        }
        let point = [0.9, 0.1, 0.2];
        let first = Camera::brain(0.0).project(point);
        let later = Camera::brain(6.0).project(point);
        assert!((first[0] - later[0]).abs() > 0.1);
        assert!((first[2] - later[2]).abs() > 0.1);
    }
    #[test]
    fn each_work_state_activates_its_own_regions_and_stopped_states_do_not_fire() {
        for (motion, region, silent) in [
            (Motion::Collect, 0, 3),
            (Motion::Inspect, 2, 0),
            (Motion::Remove, 3, 0),
            (Motion::Reconcile, 2, 3),
        ] {
            let mut peak = 0.0_f32;
            for step in 0..200 {
                let activation = cluster_activation(step as f32 * 0.05, motion, 0.0);
                peak = peak.max(activation[region]);
                assert_eq!(activation[silent], 0.0);
            }
            assert!(peak > 0.9, "{motion:?}");
        }
        for motion in [
            Motion::Still,
            Motion::Paused,
            Motion::Disconnected,
            Motion::Blocked,
        ] {
            assert!(!motion.animating());
            assert_eq!(cluster_activation(1.0, motion, 1.0), [0.0; 8]);
        }
        let camera = Camera::fly(1.0);
        let front = camera.project([0.0, 0.0, 0.3]);
        let back = camera.project([0.0, 0.0, -0.3]);
        assert!((front[0] - back[0]).abs() > 0.1);
        assert!((front[1] - back[1]).abs() > 0.1);
        assert!(front[2] > back[2]);
    }
    fn capture(motion: Motion, clock: u64, age: Option<u64>) -> ratatui::buffer::Buffer {
        let mut terminal = Terminal::new(TestBackend::new(90, 32)).unwrap();
        terminal
            .draw(|f| render(f, Rect::new(0, 0, 90, 32), motion, clock, age))
            .unwrap();
        terminal.backend().buffer().clone()
    }
    #[test]
    fn work_states_and_confirmed_receipts_drive_distinct_visuals() {
        let collect = capture(Motion::Collect, 700, None);
        let inspect = capture(Motion::Inspect, 700, None);
        let removal = capture(Motion::Remove, 700, None);
        let resting = capture(Motion::Rest, 700, None);
        assert_ne!(collect, inspect);
        assert_ne!(inspect, removal);
        assert_ne!(resting, capture(Motion::Rest, 1400, None));
        assert_ne!(inspect, capture(Motion::Inspect, 700, Some(400)));
        assert_eq!(inspect, capture(Motion::Inspect, 700, Some(2000)));
        assert_eq!(
            Motion::from_work(false, true, "inspect", "Removing"),
            Motion::Still
        );
        assert_eq!(
            Motion::from_work(true, true, "inspect", "Removing"),
            Motion::Rest
        );
        assert_eq!(
            Motion::from_work(true, false, "inspect", "Removing"),
            Motion::Remove
        );
        assert_eq!(
            Motion::from_work(true, false, "followers", ""),
            Motion::Collect
        );
    }
    #[test]
    fn rendering_is_clipped_and_ascii_fallback_contains_only_ascii() {
        for (width, height) in [(1, 1), (30, 8), (70, 18), (200, 80)] {
            let mut terminal = Terminal::new(TestBackend::new(width + 4, height + 4)).unwrap();
            terminal
                .draw(|f| {
                    render(
                        f,
                        Rect::new(2, 2, width, height),
                        Motion::Collect,
                        1000,
                        None,
                    )
                })
                .unwrap();
            for x in 0..width + 4 {
                assert_eq!(terminal.backend().buffer()[(x, 0)].symbol(), " ");
            }
            for y in 0..height + 4 {
                assert_eq!(terminal.backend().buffer()[(0, y)].symbol(), " ");
            }
        }
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        terminal
            .draw(|f| {
                brain(f, Rect::new(0, 0, 60, 10), 1.0, Motion::Inspect, 0.0, true);
                fly(f, Rect::new(0, 10, 60, 10), 1.0, Motion::Inspect, 0.0, true);
            })
            .unwrap();
        assert!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .all(|c| c.symbol().is_ascii())
        );
        assert!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .any(|c| c.symbol() != " ")
        );
    }
    #[test]
    #[ignore = "manual render-cost measurement"]
    fn render_cost() {
        let mut terminal = Terminal::new(TestBackend::new(160, 50)).unwrap();
        let start = std::time::Instant::now();
        for i in 0..200 {
            terminal
                .draw(|f| render(f, Rect::new(0, 0, 160, 50), Motion::Inspect, i * 50, None))
                .unwrap();
        }
        eprintln!(
            "fly panels: {:.2} ms/frame",
            start.elapsed().as_secs_f64() * 1000.0 / 200.0
        );
    }
}
