//! Robustness test for `Ui::show_demo`: every page renders and every widget
//! on it can be clicked without panics or API-misuse reports.

use rugi_core::{InputEvent, PointerButton};
use rugi_ui::{DemoPage, FrameInput, Ui, UiOptions, Vec2};

const INPUT: FrameInput = FrameInput { size: Vec2 { x: 1280.0, y: 800.0 }, scale: 2.0, dt: 1.0 / 60.0 };

fn frame(ui: &mut Ui) {
    ui.begin_frame(INPUT).unwrap();
    ui.show_demo();
    ui.end_frame();
    assert_eq!(ui.take_error(), None, "demo reported API misuse");
}

#[test]
fn every_page_renders_and_every_widget_is_clickable() {
    let pages = [
        DemoPage::Welcome,
        DemoPage::Basics,
        DemoPage::Inputs,
        DemoPage::Layout,
        DemoPage::Popups,
        DemoPage::Data,
        DemoPage::Style,
        DemoPage::Fonts,
        DemoPage::Metrics,
    ];
    let mut ui = Ui::new(UiOptions { inspection: true, ..Default::default() });
    let mut clicked = 0;
    for page in pages {
        ui.set_demo_page(page);
        frame(&mut ui);
        frame(&mut ui);
        assert!(ui.output().stats.instances > 100, "{page:?} drew almost nothing");
        let targets: Vec<(Vec2, &'static str)> = ui
            .output()
            .widgets
            .iter()
            .filter(|w| w.kind != "label" && w.kind != "nav_item")
            .filter(|w| !w.label.contains("Quit"))
            .map(|w| (w.rect.intersect(&w.clip).center(), w.kind))
            .filter(|(p, _)| p.x.is_finite() && p.y.is_finite())
            .collect();
        for (p, _) in targets {
            ui.set_demo_page(page);
            ui.push_event(InputEvent::PointerMoved { x: p.x, y: p.y });
            ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: true });
            ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: false });
            frame(&mut ui);
            frame(&mut ui);
            clicked += 1;
            // Close anything a click opened (popups, dialogs, the palette).
            ui.push_event(InputEvent::Key {
                key: rugi_core::Key::Escape,
                pressed: true,
                modifiers: Default::default(),
            });
            frame(&mut ui);
        }
    }
    assert!(clicked > 100, "only {clicked} widgets exercised");
}
