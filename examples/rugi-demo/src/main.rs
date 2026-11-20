//! The Rugi demo: every widget and feature in one call, `ui.show_demo()`.
//!
//! ```text
//! cargo run -p rugi-demo [-- --page <name>] [--light]
//! cargo run -p rugi-demo -- --screenshot out.png --page data [--click <label>]...
//! ```
//!
//! Pages: welcome, basics, inputs, layout, popups, data, style, fonts, metrics.
//! The screenshot flags are listed in `examples/common/src/lib.rs`.

use rugi_example_shot as shot;

use rugi::{AppOptions, DemoPage, Theme, Ui, UiOptions};

fn page(name: &str) -> DemoPage {
    match name {
        "basics" => DemoPage::Basics,
        "inputs" => DemoPage::Inputs,
        "layout" => DemoPage::Layout,
        "popups" => DemoPage::Popups,
        "data" => DemoPage::Data,
        "style" => DemoPage::Style,
        "fonts" => DemoPage::Fonts,
        "metrics" => DemoPage::Metrics,
        _ => DemoPage::Welcome,
    }
}

fn main() -> shot::Result {
    let args: Vec<String> = std::env::args().collect();
    let theme = if shot::flag(&args, "--light") { Theme::light() } else { Theme::dark() };
    let options = AppOptions {
        title: "Rugi Demo".into(),
        size: [1440, 900],
        ui: UiOptions { theme, ..Default::default() },
        ..Default::default()
    };

    let start = page(&shot::arg(&args, "--page").unwrap_or_default());
    let mut first_frame = true;
    let frame = move |ui: &mut Ui| {
        if std::mem::take(&mut first_frame) {
            ui.set_demo_page(start);
        }
        ui.show_demo();
    };

    match shot::arg(&args, "--screenshot") {
        Some(path) => shot::screenshot(&args, &path, &options, frame),
        None => Ok(rugi::run(options, frame)?),
    }
}
