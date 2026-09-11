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
    Still,
}
impl Motion {
    pub fn from_work(active: bool, waiting: bool, phase: &str, removing: bool) -> Self {
        if !active {
            Self::Still
        } else if waiting {
            Self::Rest
        } else if removing {
            Self::Remove
        } else if matches!(phase, "following" | "followers") {
            Self::Collect
        } else if phase == "inspect" {
            Self::Inspect
        } else {
            Self::Rest
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Collect => "WALKING",
            Self::Inspect => "SENSING",
            Self::Remove => "WORKING",
            Self::Rest => "RESTING",
            Self::Still => "STILL",
        }
    }
}
#[derive(Clone, Copy)]
struct Point {
    x: f32,
    y: f32,
    z: f32,
    part: u8,
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
        points.push(Point {
            x: center[0] + x * cos - y * sin,
            y: center[1] + x * sin + y * cos,
            z: center[2] + radii[2] * z,
            part,
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
                let normal: [f32; 3] =
                    std::array::from_fn(|i| (position[i] - center[i]) / radii[i].powi(2));
                let length = normal.iter().map(|v| v * v).sum::<f32>().sqrt();
                let normal = normal.map(|v| v / length);
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
        ellipsoid(
            &mut p,
            [0.46, -0.11, 0.20],
            [0.095, 0.155, 0.035],
            1800,
            3,
            0.15,
        );
        // Sparse membranes preserve transparent wings; bright veins carry their shape.
        for (y, z) in [(-0.23, 0.08), (-0.32, -0.08)] {
            ellipsoid(&mut p, [-0.30, y, z], [0.51, 0.12, 0.015], 650, 4, 0.20);
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
    fn dot(&mut self, x: f32, y: f32, intensity: u8) {
        self.depth_dot(x, y, f32::INFINITY, intensity);
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
    fn line(&mut self, from: [f32; 2], to: [f32; 2], intensity: u8) {
        for i in 0..48 {
            let t = i as f32 / 47.0;
            self.dot(
                from[0] + (to[0] - from[0]) * t,
                from[1] + (to[1] - from[1]) * t,
                intensity,
            );
        }
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
            let color = if still {
                MUTED
            } else {
                match self.light[i * 8..i * 8 + 8].iter().max().unwrap() {
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
struct BrainCamera {
    yaw: (f32, f32),
    pitch: (f32, f32),
}
impl BrainCamera {
    fn new(time: f32) -> Self {
        Self {
            yaw: (0.38 + (time * 0.19).sin() * 0.46).sin_cos(),
            pitch: (-0.22 + (time * 0.13).sin() * 0.10).sin_cos(),
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
    let speed = match motion {
        Motion::Rest => 0.45,
        Motion::Collect => 0.8,
        Motion::Remove => 1.3,
        _ => 1.0,
    };
    std::array::from_fn(|i| {
        let phase = (time * speed - i as f32 * 0.48).rem_euclid(4.8);
        let pulse = (1.0 - ((phase - 0.65) / 0.65).abs()).max(0.0);
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
    let camera = BrainCamera::new(time);
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
    raster.paint(frame, motion == Motion::Still);
}
fn fly(frame: &mut Frame, area: Rect, time: f32, motion: Motion, burst: f32, ascii: bool) {
    let mut raster = Raster::new(area, 2.65, 1.40, ascii);
    let walking = matches!(motion, Motion::Collect | Motion::Remove);
    let swing = if walking { time * 5.0 } else { 0.0 };
    let bob = if walking { swing.sin() * 0.015 } else { 0.0 };
    for p in fly_points() {
        let mut y = p.y + bob;
        if p.part == 4 && (motion == Motion::Remove || burst > 0.0) {
            y -= ((time * 30.0).sin() * 0.12).abs();
        }
        // Shade the visible shell; suppress the far side instead of flattening both surfaces.
        if p.part != 4 && p.z < 0.025 {
            continue;
        }
        let light = if p.part == 3 {
            if ((p.x * 130.0) as i32 + (p.y * 130.0) as i32).rem_euclid(3) == 0 {
                4
            } else {
                5
            }
        } else if p.part == 4 || (p.part == 0 && ((p.x + 0.7) * 25.0).rem_euclid(4.0) < 0.6) {
            0
        } else if p.z > 0.15 {
            2
        } else if p.z > 0.09 {
            1
        } else {
            0
        };
        raster.dot(p.x * 1.45, y * 0.85 - 0.05, light);
    }
    let project = |p: [f32; 2]| [p[0] * 1.45, p[1] * 0.85 - 0.05];
    for offset in [0.0, -0.10] {
        let flutter = if motion == Motion::Remove || burst > 0.0 {
            ((time * 30.0).sin() * 0.12).abs()
        } else {
            0.0
        };
        let root = project([0.10, -0.13 + offset - flutter]);
        for tip in [[-0.73, -0.42], [-0.78, -0.29], [-0.64, -0.20]] {
            raster.line(root, project([tip[0], tip[1] + offset - flutter]), 1);
        }
    }
    // Alternating tripod gait: three legs per side with opposite phase.
    for side in 0..2 {
        for leg in 0..3 {
            let step = if walking {
                (swing + (leg + side) as f32 * std::f32::consts::PI).sin()
            } else {
                0.0
            };
            let base_x = 0.19 - leg as f32 * 0.15;
            let reach = 0.6 - leg as f32 * 0.54;
            let lift = if walking { step.max(0.0) * 0.08 } else { 0.0 };
            let grooming = if motion == Motion::Rest && leg == 0 {
                (time * 2.0).sin() * 0.06
            } else {
                0.0
            };
            let knee = [reach * 0.6 + side as f32 * 0.05, 0.32 + bob];
            let foot = [
                reach + step * 0.09 + side as f32 * 0.10,
                0.66 - lift - grooming,
            ];
            raster.line(
                project([base_x, 0.1]),
                project(knee),
                if side == 0 { 1 } else { 0 },
            );
            raster.line(project(knee), project(foot), if side == 0 { 2 } else { 1 });
            raster.line(project(foot), project([foot[0] + 0.06, foot[1] + 0.01]), 1);
            for bristle in 1..4 {
                let t = bristle as f32 / 4.0;
                let point = [
                    knee[0] + (foot[0] - knee[0]) * t,
                    knee[1] + (foot[1] - knee[1]) * t,
                ];
                raster.line(
                    project(point),
                    project([point[0] - 0.025, point[1] - 0.018]),
                    1,
                );
            }
        }
    }
    let feel = if motion == Motion::Inspect {
        (time * 6.0).sin() * 0.07
    } else {
        0.0
    };
    raster.line(project([0.51, -0.17]), project([0.70, -0.28 + feel]), 2);
    raster.line(project([0.49, -0.20]), project([0.59, -0.37 - feel]), 1);
    raster.line(project([0.55, -0.02]), project([0.62, 0.11]), 1);
    for i in 0..14 {
        let x = -0.12 + i as f32 * 0.025;
        let y = -0.27 - (1.0 - ((x - 0.05) / 0.25).powi(2)).max(0.0).sqrt() * 0.045;
        raster.line(project([x, y]), project([x - 0.025, y - 0.055]), 1);
    }
    for i in 0..60 {
        raster.dot(-1.3 + i as f32 * 0.044, 0.60, 0);
    }
    raster.paint(frame, motion == Motion::Still);
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
    let brain_block = panel(" BRAIN / VISUALIZATION ");
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
    if motion != Motion::Still && area.height >= 22 {
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
        let first = BrainCamera::new(0.0).project(point);
        let later = BrainCamera::new(6.0).project(point);
        assert!((first[0] - later[0]).abs() > 0.1);
        assert!((first[2] - later[2]).abs() > 0.1);
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
            Motion::from_work(false, true, "inspect", true),
            Motion::Still
        );
        assert_eq!(Motion::from_work(true, true, "inspect", true), Motion::Rest);
        assert_eq!(
            Motion::from_work(true, false, "inspect", true),
            Motion::Remove
        );
        assert_eq!(
            Motion::from_work(true, false, "followers", false),
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
