//! The native event loop handler.

use crate::pacing::{self, Pacing};
use crate::{App, AppOptions, InitContext, PresentPolicy, RunError, WinitInput};
use rugi_core::{InputEvent, PointerButton, Repaint, Vec2, WindowCommand};
use rugi_render::{Renderer, Viewport};
use rugi_ui::{DisplayInfo, FrameInput, Ui};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{Window, WindowId};

/// Frame step for the first frame after an idle period.
const NOMINAL_DT: f32 = 1.0 / 60.0;
/// Minimum age of the cached monitor refresh rate before it is queried again.
const REFRESH_QUERY_INTERVAL: Duration = Duration::from_secs(1);
/// Minimum time between two `RUGI_PACING_LOG` reports.
const PACING_LOG_INTERVAL: Duration = Duration::from_secs(2);

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    adapter_info: wgpu::AdapterInfo,
}

pub(crate) struct Runner<A: App> {
    options: AppOptions,
    app: A,
    ui: Ui,
    gpu: Option<Gpu>,
    error: Option<RunError>,
    last_frame: Instant,
    animating: bool,
    next_repaint: Option<Instant>,
    input: WinitInput,
    exit: bool,
    pacing: Pacing,
    /// Cached monitor refresh rate. Querying it hits the window system, so it
    /// is refreshed at most once per [`REFRESH_QUERY_INTERVAL`] or when the
    /// window moves (`refresh_queried` reset to `None`).
    refresh_hz: Option<f32>,
    refresh_queried: Option<Instant>,
}

impl<A: App> Runner<A> {
    pub fn new(options: AppOptions, app: A) -> Self {
        let mut ui = Ui::new(options.ui.clone());
        ui.set_transparent_canvas(options.transparent);
        Self {
            options,
            app,
            ui,
            gpu: None,
            error: None,
            last_frame: Instant::now(),
            animating: false,
            next_repaint: None,
            input: WinitInput::new(),
            exit: false,
            pacing: Pacing::new(),
            refresh_hz: None,
            refresh_queried: None,
        }
    }

    /// The error that ended the event loop, if any.
    pub fn take_error(&mut self) -> Option<RunError> {
        self.error.take()
    }

    fn init_gpu(&mut self, el: &ActiveEventLoop) -> Result<Gpu, RunError> {
        let attrs = Window::default_attributes()
            .with_title(self.options.title.clone())
            .with_inner_size(LogicalSize::new(self.options.size[0], self.options.size[1]))
            .with_min_inner_size(LogicalSize::new(self.options.min_size[0], self.options.min_size[1]))
            .with_decorations(self.options.decorations)
            .with_transparent(self.options.transparent)
            .with_resizable(self.options.resizable);
        let window = Arc::new(el.create_window(attrs).map_err(|e| RunError::Window(e.to_string()))?);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(
            el.owned_display_handle(),
        )));
        let surface = instance.create_surface(window.clone()).map_err(|e| RunError::Graphics(e.to_string()))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .map_err(|e| RunError::Graphics(format!("no compatible GPU adapter: {e}")))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("rugi device"),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .map_err(|e| RunError::Graphics(e.to_string()))?;
        let caps = surface.get_capabilities(&adapter);
        let format = pick_format(&caps).ok_or_else(|| RunError::Graphics("surface reports no formats".into()))?;
        let size = window.inner_size();
        let (present_mode, latency) = match self.options.present {
            PresentPolicy::Vsync => (wgpu::PresentMode::AutoVsync, 2),
            PresentPolicy::LowLatency => {
                let mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
                    wgpu::PresentMode::Mailbox
                } else {
                    wgpu::PresentMode::AutoVsync
                };
                (mode, 1)
            }
            PresentPolicy::Uncapped => (wgpu::PresentMode::AutoNoVsync, 2),
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            desired_maximum_frame_latency: latency,
            alpha_mode: pick_alpha(&caps, self.options.transparent),
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);
        if self.pacing.log {
            eprintln!(
                "[rugi surface] {:?} | format {:?} | alpha {:?} (supported {:?}) | present {:?}",
                adapter.get_info().backend,
                config.format,
                config.alpha_mode,
                caps.alpha_modes,
                config.present_mode
            );
        }
        let mut renderer = Renderer::new(&device, &queue, format);
        // A post-multiplying compositor needs straight alpha or translucent
        // edges come out too dark.
        renderer.set_straight_alpha_output(config.alpha_mode == wgpu::CompositeAlphaMode::PostMultiplied);
        Ok(Gpu { window, surface, device, queue, config, renderer, adapter_info: adapter.get_info() })
    }

    fn redraw(&mut self) {
        let Some(gpu) = self.gpu.as_mut() else { return };
        let now = Instant::now();
        let real_dt = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        // After an idle period, start animations from a nominal frame step.
        let dt = if self.animating { real_dt } else { real_dt.min(NOMINAL_DT) };
        let size = gpu.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        if size.width != gpu.config.width || size.height != gpu.config.height {
            gpu.config.width = size.width;
            gpu.config.height = size.height;
            gpu.surface.configure(&gpu.device, &gpu.config);
        }
        let scale = gpu.window.scale_factor() as f32;
        let input = FrameInput { size: Vec2::new(size.width as f32 / scale, size.height as f32 / scale), scale, dt };
        if self.refresh_queried.is_none_or(|t| now.duration_since(t) > REFRESH_QUERY_INTERVAL) {
            self.refresh_hz =
                gpu.window.current_monitor().and_then(|m| m.refresh_rate_millihertz()).map(|m| m as f32 / 1000.0);
            self.refresh_queried = Some(now);
        }
        let (mean, p99) = self.pacing.stats();
        self.ui.set_display_info(DisplayInfo {
            refresh_hz: self.refresh_hz,
            frame_interval_ms: mean,
            frame_interval_p99_ms: p99,
            fps: pacing::fps(mean),
        });
        if self.ui.begin_frame(input).is_err() {
            return;
        }
        self.app.frame(&mut self.ui);
        let out = self.ui.end_frame();

        let frame = match gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                gpu.surface.configure(&gpu.device, &gpu.config);
                // The atlas upload in `out` was consumed; resend all of it.
                self.ui.invalidate_gpu_resources();
                gpu.window.request_redraw();
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                self.ui.invalidate_gpu_resources();
                return;
            }
            other => {
                self.error = Some(RunError::Graphics(format!("surface error: {other:?}")));
                self.exit = true;
                return;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let viewport = Viewport { size_px: [size.width, size.height], scale };
        let clear = if self.options.transparent { [0.0; 4] } else { [0.0, 0.0, 0.0, 1.0] };
        gpu.renderer.render_to_view(&gpu.device, &gpu.queue, &view, out, viewport, clear);
        gpu.window.pre_present_notify();
        gpu.queue.present(frame);
        let presented = Instant::now();
        self.pacing.record(presented, self.animating);
        if self.pacing.log && self.animating && presented.duration_since(self.pacing.last_log) > PACING_LOG_INTERVAL {
            self.pacing.last_log = presented;
            let (mean, p99) = self.pacing.stats();
            eprintln!(
                "[rugi pacing] display {:?} Hz | {} samples | mean {:.2} ms ({:.1} fps) | p99 {:.2} ms | build {} us",
                self.refresh_hz,
                self.pacing.samples(),
                mean,
                pacing::fps(mean),
                p99,
                out.stats.build_us
            );
        }

        self.input.apply_output(&gpu.window, out);
        if out.exit_requested {
            self.exit = true;
        }
        let mut dragged = false;
        for cmd in &out.window_commands {
            match cmd {
                WindowCommand::StartDrag => {
                    let _ = gpu.window.drag_window();
                    dragged = true;
                }
                WindowCommand::Minimize => gpu.window.set_minimized(true),
                WindowCommand::ToggleMaximize => gpu.window.set_maximized(!gpu.window.is_maximized()),
                WindowCommand::Close => self.exit = true,
            }
        }
        self.animating = false;
        self.next_repaint = None;
        match out.repaint() {
            Repaint::Immediately => {
                self.animating = true;
                gpu.window.request_redraw();
            }
            Repaint::After(d) => self.next_repaint = Some(now + d.max(Duration::from_millis(1))),
            Repaint::Idle => {}
        }
        if dragged {
            // The OS consumes the release of a window drag; don't leave the UI pressed.
            self.ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: false });
            gpu.window.request_redraw();
        }
    }
}

impl<A: App> ApplicationHandler for Runner<A> {
    fn new_events(&mut self, _el: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::ResumeTimeReached { .. } = cause {
            self.next_repaint = None;
            if let Some(g) = &self.gpu {
                g.window.request_redraw();
            }
        }
    }

    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        match self.init_gpu(el) {
            Ok(mut gpu) => {
                let mut ctx = InitContext {
                    ui: &mut self.ui,
                    renderer: &mut gpu.renderer,
                    device: &gpu.device,
                    queue: &gpu.queue,
                    adapter_info: &gpu.adapter_info,
                };
                self.app.init(&mut ctx);
                gpu.window.request_redraw();
                self.gpu = Some(gpu);
            }
            Err(e) => {
                self.error = Some(e);
                el.exit();
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let scale = self.gpu.as_ref().map(|g| g.window.scale_factor()).unwrap_or(1.0);
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::Moved(_) => self.refresh_queried = None,
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.refresh_queried = None;
                if let Some(g) = &self.gpu {
                    g.window.request_redraw();
                }
            }
            event => {
                if self.input.on_window_event(&mut self.ui, &event, scale) {
                    if let Some(g) = &self.gpu {
                        g.window.request_redraw();
                    }
                }
            }
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.exit {
            el.exit();
            return;
        }
        match self.next_repaint {
            Some(t) => el.set_control_flow(ControlFlow::WaitUntil(t)),
            None => el.set_control_flow(ControlFlow::Wait),
        }
    }
}

/// Picks a non-sRGB surface format when available so blending happens in
/// sRGB space (matching design tools); the shader compensates otherwise.
fn pick_format(caps: &wgpu::SurfaceCapabilities) -> Option<wgpu::TextureFormat> {
    caps.formats.iter().copied().find(|f| !f.is_srgb()).or_else(|| caps.formats.first().copied())
}

/// Rugi renders premultiplied alpha; transparent windows need a compositor
/// mode that understands it (or at least one that is not opaque).
fn pick_alpha(caps: &wgpu::SurfaceCapabilities, transparent: bool) -> wgpu::CompositeAlphaMode {
    use wgpu::CompositeAlphaMode as M;
    let first = caps.alpha_modes.first().copied().unwrap_or(M::Auto);
    if !transparent {
        return first;
    }
    [M::PreMultiplied, M::PostMultiplied, M::Inherit]
        .into_iter()
        .find(|m| caps.alpha_modes.contains(m))
        .unwrap_or(first)
}
