//! The smallest Rugi program: a window with a few widgets.
//!
//! ```text
//! cargo run -p rugi --example hello
//! ```

use rugi::{AppOptions, Ui};

fn main() -> Result<(), rugi::RunError> {
    let options = AppOptions { title: "Hello, Rugi".into(), size: [480, 320], ..Default::default() };
    let mut name = String::new();
    let mut volume = 0.5;
    let mut clicks = 0;

    rugi::run(options, move |ui: &mut Ui| {
        ui.central_panel(|ui| {
            ui.title("Hello, Rugi");
            ui.text_input("name", &mut name, "Your name");
            ui.slider("Volume", &mut volume, 0.0, 1.0);
            if ui.button("Click me").clicked {
                clicks += 1;
            }
            ui.label(&format!("Clicked {clicks} times"));
        });
    })
}
