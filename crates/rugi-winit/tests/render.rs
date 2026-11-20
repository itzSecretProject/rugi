//! GPU rendering tests (offscreen). Skipped with a message when no adapter exists.

use rugi_core::{Color, Rect, Vec2};
use rugi_ui::UiOptions;
use rugi_winit::HeadlessRunner;

fn runner(scale: f32) -> Option<HeadlessRunner> {
    match HeadlessRunner::new(UiOptions::default(), Vec2::new(200.0, 100.0), scale) {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("skipping GPU test: {e}");
            None
        }
    }
}

fn px(img: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

#[test]
fn primitives_land_where_expected() {
    let Some(mut r) = runner(2.0) else { return };
    r.frame(1.0 / 60.0, |ui| {
        let p = ui.painter();
        p.rect(Rect::from_xywh(10.0, 10.0, 40.0, 40.0), 0.0, Color::rgb(255, 0, 0), None);
        p.rect(Rect::from_xywh(60.0, 10.0, 40.0, 40.0), 20.0, Color::rgb(0, 255, 0), None);
        p.rect(Rect::from_xywh(110.0, 10.0, 40.0, 40.0), 4.0, Color::TRANSPARENT, Some((4.0, Color::rgb(0, 0, 255))));
        p.line(Vec2::new(160.0, 20.0), Vec2::new(190.0, 20.0), 4.0, Color::WHITE);
    });
    let (w, h, img) = r.capture().expect("capture");
    assert_eq!((w, h), (400, 200));
    // Solid red square center.
    assert_eq!(px(&img, w, 60, 60), [255, 0, 0, 255]);
    // Circle (radius = half size): center green, corner shows background.
    assert_eq!(px(&img, w, 160, 60)[1], 255);
    assert_ne!(px(&img, w, 122, 22)[1], 255);
    // Border-only rect: border blue, interior is background.
    assert_eq!(px(&img, w, 223, 60), [0, 0, 255, 255]);
    assert_ne!(px(&img, w, 260, 60)[2], 255);
    // Line.
    assert_eq!(px(&img, w, 350, 40), [255, 255, 255, 255]);
    // Anti-aliasing: the circle edge has intermediate coverage somewhere.
    let partial = (20..100u32)
        .flat_map(|y| (120..200u32).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let g = px(&img, w, x, y)[1];
            g > 30 && g < 225
        })
        .count();
    // A 40 px radius circle has a perimeter of ~250 px; most edge pixels are partial.
    assert!(partial > 100, "expected anti-aliased circle edge, got {partial} partial pixels");
}

#[test]
fn text_renders_glyph_pixels() {
    let Some(mut r) = runner(1.0) else { return };
    r.frame(1.0 / 60.0, |ui| {
        ui.central_panel(|ui| {
            ui.title("Rugi");
        });
    });
    let (w, h, img) = r.capture().expect("capture");
    let bright = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| px(&img, w, x, y)[0] > 200).count();
    assert!(bright > 50, "expected light glyph pixels on the dark theme, got {bright}");
}

#[test]
fn glass_blurs_what_is_behind_it() {
    let Some(mut r) = runner(1.0) else { return };
    // Left half red, right half blue; a clear glass panel straddles the seam.
    let glass = rugi_core::GlassStyle {
        blur: 1.0,
        tint: Color::TRANSPARENT,
        highlight: 0.0,
        grain: 0.0,
        border_width: 0.0,
        saturation: 1.0,
        ..rugi_core::GlassStyle::frosted()
    };
    let build = |ui: &mut rugi_ui::Ui| {
        let p = ui.painter();
        p.rect(Rect::from_xywh(0.0, 0.0, 100.0, 100.0), 0.0, Color::rgb(255, 0, 0), None);
        p.rect(Rect::from_xywh(100.0, 0.0, 100.0, 100.0), 0.0, Color::rgb(0, 0, 255), None);
        ui.glass(Rect::from_xywh(60.0, 20.0, 80.0, 60.0), &glass);
    };
    r.frame(1.0 / 60.0, build);
    let (w, _, img) = r.capture().expect("capture");
    // Outside the glass the halves are untouched.
    assert_eq!(px(&img, w, 20, 50), [255, 0, 0, 255]);
    assert_eq!(px(&img, w, 180, 50), [0, 0, 255, 255]);
    // Inside the glass at the seam, red and blue are mixed by the blur.
    let seam = px(&img, w, 100, 50);
    assert!(seam[0] > 40 && seam[2] > 40, "expected blurred red/blue mix at the seam, got {seam:?}");
    // Glass draws through the snapshot path; the frame still has its primitives.
    assert!(r.render_stats().draw_calls >= 3, "scene pass + blur passes + copy");
}

#[test]
fn backdrop_fills_with_animated_pattern() {
    let Some(mut r) = runner(1.0) else { return };
    let b = rugi_core::Backdrop::new(rugi_core::BackdropKind::Plasma);
    r.frame(1.0 / 60.0, |ui| ui.backdrop(&b));
    let (w, h, a) = r.capture().expect("capture");
    let distinct = (0..h)
        .step_by(10)
        .flat_map(|y| (0..w).step_by(10).map(move |x| (x, y)))
        .map(|(x, y)| px(&a, w, x, y))
        .collect::<std::collections::HashSet<_>>();
    assert!(distinct.len() > 20, "backdrop should vary across the window, got {} colors", distinct.len());
    for _ in 0..30 {
        r.frame(1.0 / 60.0, |ui| ui.backdrop(&b));
    }
    let (_, _, later) = r.capture().expect("capture");
    assert_ne!(a, later, "backdrop animates over time");
}

#[test]
fn overlapping_backdrops_do_not_bleed_through_rounded_corners() {
    let Some(mut r) = runner(1.0) else { return };
    let grey = Color::rgb(60, 60, 60);
    let red = Color::rgb(220, 0, 0);
    let mut bg = rugi_core::Backdrop::new(rugi_core::BackdropKind::Mesh).with_colors(grey, grey, grey);
    bg.speed = 0.0;
    let mut card = rugi_core::Backdrop::new(rugi_core::BackdropKind::Mesh).with_colors(red, red, red);
    card.speed = 0.0;
    r.frame(1.0 / 60.0, |ui| {
        ui.backdrop(&bg);
        ui.backdrop_rect(Rect::from_xywh(50.0, 20.0, 100.0, 60.0), 30.0, &card);
    });
    let (w, _, img) = r.capture().expect("capture");
    // Inside the card: red. Just outside its rounded corner (inside its
    // bounding box): the grey background, not the card's color.
    let inside = px(&img, w, 100, 50);
    assert!(inside[0] > 120 && inside[1] < 40, "card center should be red, got {inside:?}");
    let corner = px(&img, w, 52, 22);
    assert!((corner[0] as i32 - corner[1] as i32).abs() < 25, "corner should show grey background, got {corner:?}");
}

#[test]
fn straight_alpha_output_unpremultiplies_for_transparent_windows() {
    let Some(mut r) = runner(1.0) else { return };
    r.ui.set_transparent_canvas(true);
    let half_white = Color::rgba(255, 255, 255, 128);
    let build = |ui: &mut rugi_ui::Ui| ui.painter().rect(Rect::from_xywh(0.0, 0.0, 50.0, 50.0), 0.0, half_white, None);
    r.frame(1.0 / 60.0, build);
    let (w, _, pre) = r.capture_with_clear([0.0; 4]).expect("capture");
    let p = px(&pre, w, 10, 10);
    assert!(p[0] < 140 && p[3] > 120 && p[3] < 136, "premultiplied by default, got {p:?}");
    r.set_straight_alpha_output(true);
    r.frame(1.0 / 60.0, build);
    let (_, _, straight) = r.capture_with_clear([0.0; 4]).expect("capture");
    let s = px(&straight, w, 10, 10);
    assert!(s[0] > 245 && s[3] > 120 && s[3] < 136, "straight alpha keeps full color, got {s:?}");
    assert_eq!(px(&straight, w, 150, 50), [0, 0, 0, 0], "transparent stays transparent");
}
