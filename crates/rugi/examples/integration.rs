//! Rugi inside an application that owns its window, event loop and wgpu
//! device: the app clears the frame with its own color (stand-in for a game
//! or 3D scene), and Rugi draws a settings panel on top in the same pass.
//!
//! ```text
//! cargo run -p rugi --example integration
//! ```
//!
//! The pieces an integration needs:
//! - [`rugi::WinitInput`] turns winit events into Rugi input and applies the
//!   cursor and clipboard output.
//! - [`rugi::Ui`] builds a frame between `begin_frame` and `end_frame`.
//! - [`rugi::render::Renderer`] uploads that frame with `prepare` and records it
//!   into any render pass with `render`.
//!
//! `Renderer::render` draws into a pass you own. Glass panels need the blurred
//! scene behind them, so they render only through `Renderer::render_to_view`
//! (or `rugi::run`).

use rugi::render::{Renderer, Viewport};
use rugi::winit::application::ApplicationHandler;
use rugi::winit::event::WindowEvent;
use rugi::winit::event_loop::{ActiveEventLoop, EventLoop};
use rugi::winit::window::{Window, WindowId};
use rugi::{wgpu, FrameInput, Ui, UiOptions, Vec2, WinitInput};
use std::sync::Arc;
use std::time::Instant;

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}

/// State owned by the application; the UI edits it.
struct Scene {
    color: [f32; 3],
    paused: bool,
    time: f32,
}

struct Integration {
    gpu: Option<Gpu>,
    renderer: Option<Renderer>,
    ui: Ui,
    input: WinitInput,
    scene: Scene,
    last_frame: Instant,
}

impl Integration {
    fn new() -> Self {
        Self {
            gpu: None,
            renderer: None,
            ui: Ui::new(UiOptions::default()),
            input: WinitInput::new(),
            scene: Scene { color: [0.10, 0.12, 0.20], paused: false, time: 0.0 },
            last_frame: Instant::now(),
        }
    }

    fn settings_panel(ui: &mut Ui, scene: &mut Scene) {
        ui.side_panel(280.0, |ui| {
            ui.heading("Scene");
            ui.muted("Rugi drawn over the application's own frame.");
            ui.space(8.0);
            let [r, g, b] = &mut scene.color;
            ui.slider("Red", r, 0.0, 1.0);
            ui.slider("Green", g, 0.0, 1.0);
            ui.slider("Blue", b, 0.0, 1.0);
            ui.checkbox("Pause animation", &mut scene.paused);
            ui.label(&format!("Time: {:.1} s", scene.time));
        });
    }

    fn redraw(&mut self) {
        let (Some(gpu), Some(renderer)) = (self.gpu.as_mut(), self.renderer.as_mut()) else { return };
        let size = gpu.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        if (size.width, size.height) != (gpu.config.width, gpu.config.height) {
            gpu.config.width = size.width;
            gpu.config.height = size.height;
            gpu.surface.configure(&gpu.device, &gpu.config);
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        if !self.scene.paused {
            self.scene.time += dt;
        }

        // 1. Build the UI.
        let scale = gpu.window.scale_factor() as f32;
        let logical = Vec2::new(size.width as f32 / scale, size.height as f32 / scale);
        if self.ui.begin_frame(FrameInput { size: logical, scale, dt }).is_err() {
            return;
        }
        Self::settings_panel(&mut self.ui, &mut self.scene);
        let output = self.ui.end_frame();

        let frame = match gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => {
                // The atlas upload in `output` was not used; send it again.
                gpu.surface.configure(&gpu.device, &gpu.config);
                self.ui.invalidate_gpu_resources();
                gpu.window.request_redraw();
                return;
            }
        };
        let view = frame.texture.create_view(&Default::default());

        // 2. Upload the UI frame, then draw the app and the UI in one pass.
        renderer.prepare(&gpu.device, &gpu.queue, output, Viewport { size_px: [size.width, size.height], scale });
        let pulse = 0.5 + 0.5 * self.scene.time.sin();
        let [r, g, b] = self.scene.color.map(|c| (c * (0.7 + 0.3 * pulse)) as f64);
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("app pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            // Your own draw calls go here, before or after the UI.
            renderer.render(&mut pass, output);
        }
        gpu.queue.submit(Some(encoder.finish()));
        gpu.window.pre_present_notify();
        gpu.queue.present(frame);

        // 3. Apply cursor and clipboard changes; keep animating.
        self.input.apply_output(&gpu.window, output);
        gpu.window.request_redraw();
    }
}

impl ApplicationHandler for Integration {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        match init_gpu(el) {
            Ok(gpu) => {
                self.renderer = Some(Renderer::new(&gpu.device, &gpu.queue, gpu.config.format));
                gpu.window.request_redraw();
                self.gpu = Some(gpu);
            }
            Err(e) => {
                eprintln!("{e}");
                el.exit();
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(gpu) = &self.gpu else { return };
        let scale = gpu.window.scale_factor();
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::RedrawRequested => self.redraw(),
            event => {
                // Only events Rugi used need a new frame; an application that
                // also reacts to input itself would handle `event` here too.
                if self.input.on_window_event(&mut self.ui, &event, scale) {
                    gpu.window.request_redraw();
                }
            }
        }
    }
}

fn init_gpu(el: &ActiveEventLoop) -> Result<Gpu, String> {
    let attrs = Window::default_attributes()
        .with_title("Rugi integration")
        .with_inner_size(rugi::winit::dpi::LogicalSize::new(960.0, 600.0));
    let window = Arc::new(el.create_window(attrs).map_err(|e| e.to_string())?);
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(
        el.owned_display_handle(),
    )));
    let surface = instance.create_surface(window.clone()).map_err(|e| e.to_string())?;
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| format!("no compatible GPU adapter: {e}"))?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).map_err(|e| e.to_string())?;
    let caps = surface.get_capabilities(&adapter);
    let format = caps.formats.first().copied().ok_or("surface reports no formats")?;
    let size = window.inner_size();
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto),
        view_formats: vec![],
        color_space: wgpu::SurfaceColorSpace::Auto,
    };
    surface.configure(&device, &config);
    Ok(Gpu { window, surface, device, queue, config })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut Integration::new())?;
    Ok(())
}
