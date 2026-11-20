# Architecture

## Design choices

Rugi implements its own UI layer (identity, retained per-id state, layout,
widgets, themes, renderer, C ABI) and relies on established crates for the
solved low-level problems: wgpu for the GPU, winit for windows and input
(optional; embedding hosts skip it), cosmic-text for shaping, BiDi and font
fallback, and cbindgen for the C header. Forking an existing Rust UI library
was considered and rejected: none had a stable C ABI, an explicit begin/end
container model that maps one to one onto C, and instanced SDF rendering, so
a fork would have meant rewriting most of it. Dear ImGui is the model for the
API and for embedding: the host owns the window and the device, the library
produces draw data.

## Workspace

```
crates/
  rugi-core     geometry, Color, Id, input events, Instance/DrawList, FrameOutput   (deps: bytemuck)
  rugi-icons    Lucide icon font (ISC) + generated constants (Rust and C)          (no deps)
  rugi-text     cosmic-text shaping, font registry, layout cache, glyph atlas       (deps: core, cosmic-text, icons)
  rugi-theme    design tokens: Palette, Metrics, Typography, Motion; dark/light     (deps: core)
  rugi-ui       Ui context, identity + retained state, input/focus, layout, widgets (deps: core, text, theme)
  rugi-render   wgpu renderer: instanced SDF quads, effect pipelines                (deps: core, wgpu)
  rugi-winit    native window runner + HeadlessRunner (offscreen)                   (deps: ui, render, winit)
  rugi-ffi      stable C ABI (cdylib + staticlib), header via cbindgen              (deps: ui, winit)
  rugi          facade re-exporting the above
include/rugi/   rugi.h (generated C header), rugi.hpp (C++17 wrapper)
examples/        rugi-demo, disk-usage, effects (Rust); cpp-demo, cpp-glass (C++)
benchmarks/      Rust benchmark binary (+ optional egui), Dear ImGui comparison
tests/cpp/       C++ end-to-end interop test, C11 header smoke test
```

`rugi-ui` has no GPU or windowing dependency: everything up to the draw list
is testable headlessly and reusable by any renderer.

## Frame lifecycle

```
platform events ──push_event──▶ queue
begin_frame(size, scale, dt) ── drains queue into InputState (press+release in one frame both register)
  widgets: allocate rect ▸ interact(id, rect, sense) ▸ animate ▸ paint into DrawList
end_frame() ──▶ FrameOutput { draw_list, atlas_update, repaint, cursor, stats, widgets }
renderer.prepare(output) ▸ renderer.render(pass)
```

`Repaint` drives the event loop: `Idle` (sleep until input; measured ~0% CPU
while idle), `Immediately` (animations running), `After(d)` (caret blink).

## Identity and state ownership

* `Id` = FNV-1a hash of the parent scope id and the label (`"Text##id"` and
  `"###id"` conventions as in Dear ImGui; `push_id`/`scope` for lists).
* The application owns its data (`&mut bool`, `&mut f32`, `&mut String`,
  or C buffers). The context owns per-widget UI state: animation channels
  (hover/press/focus/value), scroll offsets, text cursor/selection, keyed by
  `Id`. Typed state for custom widgets: `ui.state_mut::<T>(id)`.
* Interaction state: one `active` id (pointer captured), one `focused` id,
  Tab/Shift+Tab focus order rebuilt each frame, Enter/Space activation,
  Escape/click-away to blur. Stale ids are dropped when a widget disappears.

## Layout

Single pass with explicit containers (`begin_*` / `end`): vertical, horizontal
(start or end aligned), equal-width column grids, cards, side/top/central
panels, scroll areas. A container measures its content and is placed in its
parent at `end`. Cards draw their background *under* their content without a
second pass by reserving draw-list slots at `begin` and filling them at `end`.
`set_next_item_width` overrides one item. The Rust closure API (`ui.card(|ui|
…)`) is sugar over begin/end, which is exactly what the C ABI exposes.

The mouse wheel goes to the innermost scrollable area under the pointer
(resolved from the previous frame's geometry), so nested scroll areas work.

Not yet: flexbox/grid solving. Taffy can be added as a container type that
lays out children in a measure pass; the container abstraction already
separates "measure content" from "place in parent".

## Overlays, popups and dialogs

Popups, menus, context menus, tooltips, the command palette and modal
dialogs draw into a separate overlay draw list that is appended after the
main list, so a popup opened from the sidebar covers the central panel built
later in the same frame. Pointer input under an open popup is blocked using
the previous frame's popup rectangles. Popups are usually built after the
widgets they cover, so this frame's rectangles aren't available yet. A modal
blocks everything outside the overlay. One popup is open at a time
(`open_popup`/`close_popup`). It closes on an outside click, Escape, or when
it is no longer built. Popup content fades in by scaling the alpha of the
instances it emitted (`DrawList::fade_from`), so no second pass is needed.

## Styles, fonts and icons

`Theme` is shared behind an `Rc`: widgets clone a pointer, not the palette.
`push_style(|t| …)` / `pop_style()` save and restore the theme around a block,
the equivalent of `PushStyleColor`/`PushStyleVar`/`PushFont` in Dear ImGui
but covering every token. Unbalanced pushes are restored at `end_frame` and
reported. Fonts are registered at runtime (`add_font`) and addressed by
`FontFamily::Custom(n)`. All registered fonts take part in fallback, which is
how the bundled Lucide icon font renders inside any label.

## Motion

Two primitives, both time-based so the same motion plays at any refresh rate:

* `anim::approach`: exponential smoothing for hover/press/focus/value channels
  (per widget id, 4 channels; settles to exact values so the UI returns to idle).
* `anim::Spring`: an exact critically damped spring (no overshoot, keeps
  velocity when the target changes). It drives the shared nav highlight (one
  highlight glides between sidebar items) and the tab indicator.

## Tools

* Command palette (`command_palette(&[(name, hint)])`): Cmd/Ctrl+K, fuzzy
  subsequence ranking with word-start and contiguous-match bonuses, keyboard
  navigation, modal.
* Live inspector (F12, Cmd/Ctrl+Shift+I): outlines every widget's visible
  bounds, shows kind/label/id/rect/state for the hovered one, and a HUD with
  build time, instances, batches, culled count, display refresh and measured
  pacing. It is the visual side of `UiOptions::inspection`, the structured
  per-widget data used by tests, the demo's scripted screenshots, and future
  AI tooling.

## Rendering

Every primitive is one 80-byte `Instance` (`#[repr(C)]`, `Pod`) drawn as a
4-vertex triangle strip. The fragment shader computes coverage analytically:

| Kind | Technique |
|---|---|
| Rect | rounded-box SDF, per-corner radii, linear gradient, inner border |
| Shadow | Gaussian-blurred rounded box (closed-form erf approximation) |
| Glyph | R8 atlas sample, pixel-snapped quads |
| Image | RGBA texture with rounded-corner SDF mask |
| Line | capsule SDF |

The CPU never tessellates curves. Batches split only on scissor or image
texture changes (glyphs share one atlas), so the dashboard demo is ~450
instances in ~8 draw calls. Frames with effects also split on pipeline
changes (see below). Primitives outside the current clip are culled on
the CPU. Persistent GPU resources: the instance buffer grows geometrically and
is reused; the atlas is updated with only its dirty rectangle; pipeline,
layouts and samplers are created once. Blending is premultiplied alpha in sRGB
space (design-tool parity); sRGB swapchains are compensated in the shader.

The renderer never owns the device: `Renderer::prepare` + `Renderer::render`
record into a caller's `wgpu::RenderPass`, which is the integration point for
hosts that already use wgpu.

### Effects (glass and backdrops)

Glass (`PrimitiveKind::Glass`) and procedural backdrops
(`PrimitiveKind::Backdrop`) are regular 80-byte instances. The draw list
counts them, so frames without effects take the single-pass path with no
extra cost. When a frame has effects, `render_to_view` does the
following:

1. Backdrop layer. Soft backdrops that don't overlap each other are drawn
   into a half-resolution layer (`fs_backdrop_fill`). The main pass then
   composites them with a full-resolution rounded mask (`fs_backdrop_layer`).
2. Pipeline split. Command ranges are split into runs by pipeline:
   `fs_main`, `fs_glass`, `fs_backdrop` and `fs_backdrop_layer`.
3. Glass snapshots. If there is glass, the frame renders into an
   offscreen scene texture. A "barrier" is any glass instance with new
   content drawn behind it. At each one the pass ends, the scene is
   downsampled into a blur mip chain (`post.wgsl`, 13-tap filter, up to 6
   levels) and drawing resumes. At most 8 barriers per frame.
4. Copy. The scene is copied to the target.

Glass samples the chain with `textureSampleLevel` (4-tap tent). Its
refraction offset comes from the rounded-rect SDF gradient.

## Text

cosmic-text shapes with HarfRust (ligatures, kerning, BiDi, fallback). Layouts
are cached by (text, style, wrap width) and evicted after 120 unused frames,
so steady-state frames do not re-shape. Glyphs are rasterized by swash at the
physical size (DPI-aware) into a 1024² R8 atlas that doubles (to 4096²) on
overflow, applied between frames so no frame references stale UVs. Inter is
bundled; `TextOptions::load_system_fonts` enables OS fallback fonts.

## C++ interoperability

See [c-abi.md](c-abi.md). Two modes share the same functions:

* Window mode: `rugi_app_run(desc, callback, user)`. Rugi owns the window,
  GPU and loop; the C++ callback builds each frame.
* Host-driven mode: `rugi_ui_create`, the input functions and begin/end
  frame. The C++ application owns the loop. This mode produces UI state and
  statistics but has no C-level renderer: rendering inside a C++-owned
  D3D11/D3D12/Vulkan/GL device is not implemented yet.

## Extension points

Implemented:

| Feature | Where it lives |
|---|---|
| Custom widgets | `allocate`, `interact`, `state_mut`, `painter` (as the demo's accent swatches do); tables, plots and the command palette are built this way |
| GPU effects | A `PrimitiveKind` plus its own fragment entry point (glass and backdrops) |
| Headless screenshots, visual tests | `HeadlessRunner::capture`, used by the demos and GPU tests |
| Inspection for tests and tooling | `UiOptions::inspection` records kind/label/rect/clip/hover/focus per widget each frame; the F12 inspector visualizes it |

Planned, not implemented:

| Feature | Where it would plug in |
|---|---|
| Docking, node editors | New container/widget modules on the same primitives |
| Theme files, hot reload | `Theme` is plain data; add serde and a file watcher |
| Inspection over the C ABI | Expose `UiOptions::inspection` data through the C ABI and a JSON socket |
| ECS integration (e.g. `bevy_ecs`) | An optional crate drawing entity/component inspectors with the existing widgets |
| Native C++ backends (D3D11/12, Vulkan, GL) | Expose the `Instance` array, batches and atlas through the C ABI and ship per-API reference renderers of `shader.wgsl` |
| WebAssembly/WebGPU | wgpu and winit support it; needs a web runner and testing |
