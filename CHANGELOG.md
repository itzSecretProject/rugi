# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The C ABI is
versioned separately through `RUGI_ABI_VERSION`.

## [Unreleased]

## [0.1.0]

Initial release.

### Rust API

- Immediate-mode context (`Ui`) with retained per-id state, rendered by wgpu
  as instanced SDF primitives.
- Widgets: buttons, labels, checkboxes, toggles, radio buttons, float and
  integer sliders and spin boxes, text and password input, combo and list
  boxes, multi-select, segmented controls, tabs, collapsing sections, trees,
  color picker, line and bar plots, progress bars, badges, key bindings, a
  virtualized table, a carousel, and a search button.
- Layout: panels, rows (optionally centered on the cross axis), labeled rows,
  columns, cards, scroll areas; clipped single-line text ends in an ellipsis.
- Overlays: menu bar, dropdown and context menus, tooltips, anchored popups,
  modal dialogs, a command palette and a live inspector.
- Keyboard: every widget is reachable with Tab and usable from the keyboard;
  arrow keys move through menus and tables; Shift+F10 or the Menu key opens
  context menus.
- Themes: dark, light and four tinted presets, all meeting WCAG 2.2 AA
  contrast; `Theme::set_accent` derives accessible accent colors;
  `rugi_theme::contrast_ratio`.
- Text: Inter and JetBrains Mono bundled, any TTF/OTF at runtime, 2,379
  Lucide icons usable inside labels.
- Effects: glass panels with backdrop blur, animated procedural backdrops,
  screen transitions, borderless and transparent windows with a title bar.
- `rugi::run` opens a window; `WinitInput`, `Ui` and `render::Renderer` embed
  Rugi in an application with its own winit event loop and wgpu device.
  `HeadlessRunner` renders offscreen for screenshots and tests.
- Rugi makes no heap allocations per frame after warm-up.

### C and C++

- C ABI with 107 functions, a generated header (`include/rugi/rugi.h`),
  opaque handles checked on every call, typed errors, panic safety and
  thread checks. Hostile callers are tested under AddressSanitizer and UBSan.
- Header-only C++17 wrapper (`include/rugi/rugi.hpp`) with RAII scopes and
  exception-safe callbacks.
- CMake target `rugi::rugi` for `add_subdirectory` and `FetchContent`.

### Examples

- `rugi-demo`: the built-in demo, `ui.show_demo()`, in which every section
  shows the code it runs.
- `disk-usage`: a disk-usage explorer for real folders.
- `effects`: glass panels and animated backdrops.
- `hello` and `integration` (`cargo run -p rugi --example …`), and C++
  versions of the demo and of `effects`.

Requires Rust 1.90 or later.
