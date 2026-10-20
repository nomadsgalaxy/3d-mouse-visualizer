//! The demo: a cap or a trackball driven by sliders, by a calibration-style walk through each
//! motion, or (on Linux, with the `device` feature) live by the first HID multi-axis controller,
//! its raw axes divided by 350 with no calibration.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mouse3d_visualizer as space_mouse_model;
use space_mouse_model::Pose;
use space_mouse_model::ball::{self, Ball};

/// The motions a calibration asks for, in order: each axis, positive way first.
const STEPS: [(usize, bool); 12] = {
    let mut steps = [(0, true); 12];
    let mut i = 0;
    while i < 12 {
        steps[i] = (i / 2, i % 2 == 0);
        i += 1;
    }
    steps
};

const NAMES: [&str; 6] = [
    "X slide right",
    "Y pull toward",
    "Z press down",
    "Rx tilt back",
    "Ry tilt left",
    "Rz twist clockwise",
];

#[derive(PartialEq, Clone, Copy)]
enum Source {
    Sliders,
    Steps,
    Device,
}

struct Bench {
    source: Source,
    travel: [f32; 6],
    invert: [bool; 6],
    size: f32,
    ball_mode: bool,
    ball: Ball,
    started: Instant,
    /// The latest raw axes from the device thread, and what it has to say about the device.
    raw: Arc<Mutex<([i16; 6], String)>>,
}

/// Reads the first multi-axis controller's reports on a thread of its own.
#[cfg(not(all(feature = "device", target_os = "linux")))]
fn read_device(shared: Arc<Mutex<([i16; 6], String)>>) {
    if let Ok(mut s) = shared.lock() {
        s.1 = "built without the device feature (Linux only)".into();
    }
}

/// Reads the first multi-axis controller's reports on a thread of its own.
#[cfg(all(feature = "device", target_os = "linux"))]
fn read_device(shared: Arc<Mutex<([i16; 6], String)>>) {
    let say = |text: String| {
        if let Ok(mut s) = shared.lock() {
            s.1 = text;
        }
    };
    let Ok(dir) = std::fs::read_dir("/sys/class/hidraw") else {
        return say("no /sys/class/hidraw".into());
    };
    use solidcraft_space_mouse::Descriptor;
    use std::path::{Path, PathBuf};
    let mut names: Vec<PathBuf> = dir.filter_map(Result::ok).map(|e| e.path()).collect();
    names.sort();
    let found = names.iter().find_map(|sys| {
        let descriptor =
            Descriptor::parse(&std::fs::read(sys.join("device/report_descriptor")).ok()?);
        let layout = descriptor
            .is_multi_axis_controller()
            .then(|| descriptor.layout())
            .flatten()?;
        Some((Path::new("/dev").join(sys.file_name()?), layout))
    });
    let Some((node, layout)) = found else {
        return say("no multi-axis controller found".into());
    };
    let mut file = match std::fs::File::open(&node) {
        Ok(file) => file,
        Err(error) => return say(format!("can't open {}: {error}", node.display())),
    };
    say(format!("reading {}", node.display()));
    let mut buf = [0_u8; 4097];
    loop {
        use std::io::Read;
        let size = match file.read(&mut buf) {
            Ok(0) => return say("device gone".into()),
            Ok(size) => size,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return say(format!("device gone: {error}")),
        };
        let Some(report) = layout.decode(&buf[..size]) else {
            continue;
        };
        if let Ok(mut s) = shared.lock() {
            for (value, new) in s.0.iter_mut().zip(report.axes) {
                if let Some(new) = new {
                    *value = new;
                }
            }
        }
    }
}

impl eframe::App for Bench {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::Panel::right("controls")
            .min_size(260.0)
            .show(ui, |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.ball_mode, false, "Cap");
                    ui.selectable_value(&mut self.ball_mode, true, "Ball");
                    if self.ball_mode && ui.button("Square the ball").clicked() {
                        self.ball = Ball::new();
                    }
                });
                ui.separator();
                ui.radio_value(&mut self.source, Source::Sliders, "Sliders");
                if ui
                    .radio_value(
                        &mut self.source,
                        Source::Steps,
                        "Play the calibration steps",
                    )
                    .clicked()
                {
                    self.started = Instant::now();
                }
                ui.radio_value(&mut self.source, Source::Device, "Device (raw ÷ 350)");
                ui.separator();
                if self.source == Source::Device {
                    let (raw, status) = self.raw.lock().map(|s| s.clone()).unwrap_or_default();
                    ui.label(status);
                    for (i, travel) in self.travel.iter_mut().enumerate() {
                        let sign = if self.invert[i] { -1.0 } else { 1.0 };
                        *travel = sign * f32::from(raw[i]) / 350.0;
                    }
                    ctx.request_repaint_after(Duration::from_millis(16));
                }
                for (i, name) in NAMES.iter().enumerate() {
                    ui.label(*name);
                    ui.horizontal(|ui| {
                        let slider =
                            egui::Slider::new(&mut self.travel[i], -1.0..=1.0).fixed_decimals(2);
                        ui.add_enabled(self.source == Source::Sliders, slider);
                        if self.source == Source::Device {
                            ui.checkbox(&mut self.invert[i], "invert");
                        }
                    });
                }
                if ui.button("Reset").clicked() {
                    self.travel = [0.0; 6];
                }
                ui.separator();
                ui.add(egui::Slider::new(&mut self.size, 100.0..=600.0).text("width"));
            });
        egui::CentralPanel::default().show(ui, |ui| {
            let size = egui::vec2(self.size, self.size * 0.75);
            ui.vertical_centered(|ui| {
                let mut moving = self.travel.map(|v| v.abs() > 0.1);
                if self.source == Source::Steps {
                    // Each step for one showing of its motion, from rest, as the calibration window plays it.
                    let length: f32 = space_mouse_model::DEMO_SECONDS.iter().sum();
                    let elapsed = self.started.elapsed().as_secs_f32();
                    let index = (elapsed / length) as usize % STEPS.len();
                    if let Some(&(axis, positive)) = STEPS.get(index) {
                        ui.label(
                            format!("step {} of {}: {}", index + 1, STEPS.len(), NAMES[axis])
                                + if positive { "" } else { " (back)" },
                        );
                        // The same motion as `space_mouse_model::demo`, drawn at the bench's size.
                        let swing = space_mouse_model::demo_swing(elapsed % length);
                        self.travel = [0.0; 6];
                        self.travel[axis] = if positive { swing } else { -swing };
                        moving = [false; 6];
                        moving[axis] = swing > 0.15;
                        ctx.request_repaint_after(Duration::from_millis(16));
                    }
                }
                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                if self.ball_mode {
                    // Sliding and tilting roll the ball (pulling it toward you rolls it toward you), twisting
                    // spins it, and Z turns the scroll ring.
                    // Tilting the cap rolls it too, so a cap can stand in for a trackball here.
                    let t = self.travel;
                    let rate = [t[3] - t[1], t[0] - t[4], t[5], t[2]];
                    self.ball.roll(rate, ui.input(|i| i.stable_dt));
                    let rolling = rate[..3].iter().any(|v| v.abs() > 0.1);
                    ball::paint(ui, rect, &self.ball, rolling, rate[3].abs() > 0.1);
                    ctx.request_repaint_after(Duration::from_millis(16));
                } else {
                    space_mouse_model::paint(ui, rect, &Pose::new(self.travel), moving);
                }
            });
        });
    }
}

/// `--pose=x,y,z,rx,ry,rz` starts the sliders there (each -1 to 1), for a screenshot of a given
/// pose; `--ball` starts on the trackball.
fn start_pose() -> [f32; 6] {
    let mut travel = [0.0; 6];
    if let Some(list) = std::env::args().find_map(|a| a.strip_prefix("--pose=").map(str::to_owned))
    {
        for (value, text) in travel.iter_mut().zip(list.split(',')) {
            *value = text
                .trim()
                .parse::<f32>()
                .ok()
                .filter(|v| v.is_finite())
                .unwrap_or(0.0)
                .clamp(-1.0, 1.0);
        }
    }
    travel
}

fn main() -> eframe::Result {
    let raw = Arc::new(Mutex::new(([0_i16; 6], "not started".to_string())));
    let thread_raw = raw.clone();
    std::thread::spawn(move || read_device(thread_raw));
    let bench = Bench {
        source: Source::Sliders,
        travel: start_pose(),
        invert: [false; 6],
        size: 360.0,
        ball_mode: std::env::args().any(|a| a == "--ball"),
        ball: Ball::new(),
        started: Instant::now(),
        raw,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([760.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "3D mouse visualizer",
        options,
        Box::new(|_| Ok(Box::new(bench))),
    )
}
