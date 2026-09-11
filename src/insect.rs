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
fn brain_points() -> &'static [Point] {
    static POINTS: OnceLock<Vec<Point>> = OnceLock::new();
    POINTS.get_or_init(|| {
        let mut p = Vec::with_capacity(6800);
        for side in [-1.0, 1.0] {
            ellipsoid(
                &mut p,
                [side * 0.83, 0.08, 0.0],
                [0.39, 0.39, 0.32],
                1400,
                0,
                side * 0.15,
            );
            ellipsoid(
                &mut p,
                [side * 0.32, -0.12, 0.05],
                [0.40, 0.43, 0.35],
                1500,
                1,
                side * 0.23,
            );
            ellipsoid(
                &mut p,
                [side * 0.16, 0.27, 0.0],
                [0.20, 0.23, 0.22],
                400,
                2,
                0.0,
            );
        }
        p
    })
}
fn fly_points() -> &'static [Point] {
    static POINTS: OnceLock<Vec<Point>> = OnceLock::new();
    POINTS.get_or_init(|| {
        let mut p = Vec::with_capacity(2800);
        ellipsoid(&mut p, [-0.35, 0.07, 0.0], [0.40, 0.18, 0.17], 800, 0, -0.1); // abdomen
        ellipsoid(&mut p, [0.05, -0.04, 0.0], [0.25, 0.24, 0.20], 600, 1, 0.0); // thorax
        ellipsoid(&mut p, [0.38, -0.11, 0.03], [0.19, 0.19, 0.19], 440, 2, 0.0); // head
        ellipsoid(
            &mut p,
            [0.44, -0.10, 0.20],
            [0.09, 0.14, 0.035],
            240,
            3,
            0.15,
        ); // compound eye
        for (y, z) in [(-0.27, 0.06), (-0.37, -0.08)] {
            ellipsoid(&mut p, [-0.28, y, z], [0.48, 0.12, 0.016], 360, 4, 0.35);
        }
        p
    })
}
struct Raster {
    area: Rect,
    dots: Vec<u8>,
    light: Vec<u8>,
    scale: f32,
    ascii: bool,
}
impl Raster {
    fn new(area: Rect, aspect_width: f32, aspect_height: f32, ascii: bool) -> Self {
        let area = centered(area, 140, 40);
        Self {
            area,
            dots: vec![0; area.width as usize * area.height as usize],
            light: vec![0; area.width as usize * area.height as usize],
            scale: (f32::from(area.width) * 2.0 / aspect_width)
                .min(f32::from(area.height) * 4.0 / aspect_height),
            ascii,
        }
    }
    fn dot(&mut self, x: f32, y: f32, intensity: u8) {
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
        self.dots[index] |= 1 << bits[y as usize % 4][x as usize % 2];
        self.light[index] = self.light[index].max(intensity);
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
                match self.light[i] {
                    0 => Color::Rgb(62, 94, 108),
                    1 => MUTED,
                    2 => ICE,
                    3 => MINT,
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
fn brain(frame: &mut Frame, area: Rect, time: f32, motion: Motion, burst: f32, ascii: bool) {
    let mut raster = Raster::new(area, 2.65, 1.4, ascii);
    let speed = match motion {
        Motion::Rest => 0.4,
        Motion::Collect => 1.2,
        _ => 2.0,
    };
    let wave = (time * speed).rem_euclid(2.8) - 1.4;
    for p in brain_points() {
        let x = p.x + p.z * (time * 0.2).sin() * 0.08;
        let pulse = if motion == Motion::Inspect {
            (p.x.abs() - wave.abs()).abs()
        } else {
            (p.x - wave).abs()
        };
        let intensity = if burst > 0.0 && (p.y - (0.8 - burst * 1.5)).abs() < 0.12 {
            4
        } else if pulse < 0.08 {
            3
        } else if p.z > 0.15 {
            2
        } else if p.z > 0.0 {
            1
        } else {
            0
        };
        raster.dot(x, p.y, intensity);
    }
    raster.paint(frame, motion == Motion::Still);
}
fn fly(frame: &mut Frame, area: Rect, time: f32, motion: Motion, burst: f32, ascii: bool) {
    let mut raster = Raster::new(area, 2.7, 1.65, ascii);
    let walking = matches!(motion, Motion::Collect | Motion::Remove);
    let swing = if walking { time * 5.0 } else { 0.0 };
    let bob = if walking { swing.sin() * 0.015 } else { 0.0 };
    for p in fly_points() {
        let mut y = p.y + bob;
        if p.part == 4 && (motion == Motion::Remove || burst > 0.0) {
            y -= ((time * 30.0).sin() * 0.12).abs();
        }
        let light = if p.part == 3 {
            3
        } else if p.part == 4 {
            1
        } else if p.z > 0.1 {
            2
        } else {
            0
        };
        raster.dot(p.x, y, light);
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
            raster.line([base_x, 0.1], knee, if side == 0 { 1 } else { 0 });
            raster.line(knee, foot, if side == 0 { 2 } else { 1 });
        }
    }
    let feel = if motion == Motion::Inspect {
        (time * 6.0).sin() * 0.07
    } else {
        0.0
    };
    raster.line([0.51, -0.17], [0.70, -0.28 + feel], 2);
    raster.line([0.49, -0.20], [0.59, -0.37 - feel], 1);
    for i in 0..60 {
        raster.dot(-1.1 + i as f32 * 0.038, 0.73, 0);
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
