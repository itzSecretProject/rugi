//! The run-or-screenshot entry point shared by the example apps.
//!
//! [`run`] opens a window, or with `--screenshot` renders the same app
//! offscreen, replays scripted input and writes a PNG:
//!
//! ```text
//! --screenshot out.png [--size WxH] [--scale 2] [--frames 90]
//!     [--click <label>] [--rclick <label>] [--hover <label>]
//!     [--type <text>] [--key <name>] [--palette] [--wait <frames>]
//!     [--sleep <ms>] [--inspector] [--inspect]
//! ```
//!
//! Input steps run in command-line order. A label matches a widget's
//! inspection label exactly, otherwise as a substring; the topmost widget
//! wins. `--wait` renders frames without input; `--sleep` keeps rendering in
//! real time, for work the app does on other threads. `--inspector` shows
//! Rugi's inspector in the image; `--inspect` prints every widget of the last
//! frame.

use rugi::core::{InputEvent, Key, Modifiers, PointerButton};
use rugi::{App, AppOptions, HeadlessRunner, Ui, Vec2};
use std::time::{Duration, Instant};

pub type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

const DT: f32 = 1.0 / 60.0;
/// Frames rendered before the first input step, so layout and ids exist.
const WARM_UP_FRAMES: u32 = 4;
/// Frames rendered after a click or key press so its effect is visible.
const SETTLE_FRAMES: u32 = 3;
/// Frames rendered after the last step, unless `--frames` says otherwise.
const DEFAULT_FINAL_FRAMES: u32 = 90;

/// The command-line arguments, without the program name.
pub fn args() -> Vec<String> {
    std::env::args().skip(1).collect()
}

/// The value following `name` on the command line.
pub fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

/// Whether `name` appears on the command line.
pub fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

/// Runs `app` in a window, or offscreen when the command line has
/// `--screenshot <file.png>`. In both cases [`App::init`] runs first.
pub fn run<A: App + 'static>(options: AppOptions, mut app: A) -> Result {
    let args = args();
    let Some(path) = arg(&args, "--screenshot") else {
        return Ok(rugi::run(options, app)?);
    };
    let mut shot = Screenshot::new(&args, &options)?;
    app.init(&mut shot.runner.init_context());
    shot.save(&path, |ui| app.frame(ui))
}

/// Renders `frame` offscreen as the window described by `options` would show
/// it, for apps without an [`App::init`].
pub fn screenshot(args: &[String], path: &str, options: &AppOptions, frame: impl FnMut(&mut Ui)) -> Result {
    Screenshot::new(args, options)?.save(path, frame)
}

/// An offscreen stand-in for the window described by an [`AppOptions`].
struct Screenshot<'a> {
    runner: HeadlessRunner,
    args: &'a [String],
    transparent: bool,
}

impl<'a> Screenshot<'a> {
    fn new(args: &'a [String], options: &AppOptions) -> Result<Self> {
        let [default_w, default_h] = options.size;
        let size = match arg(args, "--size") {
            Some(s) => {
                let (w, h) = s.split_once('x').ok_or("--size must be WxH")?;
                Vec2::new(w.parse()?, h.parse()?)
            }
            None => Vec2::new(default_w as f32, default_h as f32),
        };
        let scale = arg(args, "--scale").map(|s| s.parse()).transpose()?.unwrap_or(2.0);
        let mut ui_options = options.ui.clone();
        ui_options.inspection = true;
        let mut runner = HeadlessRunner::new(ui_options, size, scale)?;
        runner.ui.set_transparent_canvas(options.transparent);
        runner.ui.set_inspector(flag(args, "--inspector"));
        Ok(Self { runner, args, transparent: options.transparent })
    }

    /// Replays the scripted input, lets animations settle and writes the PNG.
    fn save(&mut self, path: &str, mut frame: impl FnMut(&mut Ui)) -> Result {
        let final_frames = arg(self.args, "--frames").map(|s| s.parse()).transpose()?.unwrap_or(DEFAULT_FINAL_FRAMES);
        let r = &mut self.runner;
        let mut run = |r: &mut HeadlessRunner, frames: u32| {
            for _ in 0..frames {
                r.frame(DT, &mut frame);
            }
        };
        run(r, WARM_UP_FRAMES);
        let mut steps = self.args.iter();
        while let Some(step) = steps.next() {
            let mut value = || steps.next().cloned().ok_or_else(|| format!("{step} needs a value"));
            match step.as_str() {
                "--click" | "--rclick" => {
                    let button = if step == "--click" { PointerButton::Primary } else { PointerButton::Secondary };
                    move_to(r, &value()?)?;
                    run(r, 1);
                    r.push_event(InputEvent::PointerButton { button, pressed: true });
                    run(r, 1);
                    r.push_event(InputEvent::PointerButton { button, pressed: false });
                    run(r, SETTLE_FRAMES);
                }
                "--hover" => {
                    move_to(r, &value()?)?;
                    run(r, 1);
                }
                "--type" | "--palette-query" => {
                    r.push_event(InputEvent::Text(value()?));
                    run(r, 1);
                }
                "--key" => {
                    let name = value()?;
                    let key = Key::ALL
                        .iter()
                        .copied()
                        .find(|k| k.name().eq_ignore_ascii_case(&name))
                        .ok_or_else(|| format!("unknown key {name:?}"))?;
                    let modifiers = Modifiers::default();
                    r.push_event(InputEvent::Key { key, pressed: true, modifiers });
                    run(r, 1);
                    r.push_event(InputEvent::Key { key, pressed: false, modifiers });
                    run(r, SETTLE_FRAMES);
                }
                "--palette" => {
                    r.ui.set_command_palette_open(true);
                    run(r, 1);
                }
                "--wait" => run(r, value()?.parse()?),
                "--sleep" => {
                    let end = Instant::now() + Duration::from_millis(value()?.parse()?);
                    while Instant::now() < end {
                        run(r, 1);
                        std::thread::sleep(Duration::from_secs_f32(DT));
                    }
                }
                _ => {}
            }
        }
        run(r, final_frames);
        if let Some(e) = r.ui.take_error() {
            return Err(format!("API misuse reported: {e}").into());
        }

        let clear = if self.transparent { [0.0; 4] } else { [0.0, 0.0, 0.0, 1.0] };
        let (w, h, rgba) = r.capture_with_clear(clear)?;
        let file = std::io::BufWriter::new(std::fs::File::create(path)?);
        let mut encoder = png::Encoder::new(file, w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&rgba)?;
        println!("wrote {path} ({w}x{h}, {})", r.adapter_info().name);
        if flag(self.args, "--inspect") {
            print_widgets(&r.ui);
        }
        Ok(())
    }
}

fn move_to(r: &mut HeadlessRunner, label: &str) -> Result {
    let widgets = &r.ui.output().widgets;
    let target = widgets
        .iter()
        .rev()
        .find(|w| w.label == label)
        .or_else(|| widgets.iter().rev().find(|w| w.label.contains(label)))
        .ok_or_else(|| format!("no widget labelled {label:?}"))?;
    let p = target.rect.intersect(&target.clip).center();
    r.push_event(InputEvent::PointerMoved { x: p.x, y: p.y });
    Ok(())
}

fn print_widgets(ui: &Ui) {
    for w in &ui.output().widgets {
        println!(
            "{:<12} {:<32} x={:<7.1} y={:<7.1} w={:<7.1} h={:<6.1} hovered={} focused={}",
            w.kind,
            format!("{:?}", w.label),
            w.rect.min.x,
            w.rect.min.y,
            w.rect.width(),
            w.rect.height(),
            w.hovered,
            w.focused
        );
    }
}
