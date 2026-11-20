//! Offscreen runner: the same UI and renderer without a window.

use crate::{InitContext, RunError};
use rugi_core::{FrameStats, InputEvent, TextureId, Vec2};
use rugi_render::{render_to_rgba, Renderer, Viewport};
use rugi_ui::{FrameInput, Ui, UiOptions};

const OPAQUE_BLACK: [f64; 4] = [0.0, 0.0, 0.0, 1.0];
/// Size of the resolved begin/end pass timestamps (two `u64`s).
const TIMESTAMPS_SIZE: u64 = 16;

/// Renders UI frames offscreen, for screenshots, visual tests and GPU
/// benchmarks.
pub struct HeadlessRunner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
    /// The UI context (push input events here between frames).
    pub ui: Ui,
    size: Vec2,
    scale: f32,
    adapter_info: wgpu::AdapterInfo,
    /// Render target of the no-readback benchmark paths.
    target: Option<(wgpu::TextureView, [u32; 2])>,
}

impl HeadlessRunner {
    /// Creates a GPU device (no surface) and a UI context. `size` is logical.
    pub fn new(options: UiOptions, size: Vec2, scale: f32) -> Result<Self, RunError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .map_err(|e| RunError::Graphics(format!("no GPU adapter: {e}")))?;
        // Timestamp queries (when supported) let benchmarks report true GPU time.
        let features = adapter.features() & wgpu::Features::TIMESTAMP_QUERY;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("rugi headless"),
            required_features: features,
            required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .map_err(|e| RunError::Graphics(e.to_string()))?;
        let renderer = Renderer::new(&device, &queue, wgpu::TextureFormat::Rgba8Unorm);
        Ok(Self {
            device,
            queue,
            renderer,
            ui: Ui::new(options),
            size,
            scale,
            adapter_info: adapter.get_info(),
            target: None,
        })
    }

    /// Adapter description (name, backend, driver).
    pub fn adapter_info(&self) -> &wgpu::AdapterInfo {
        &self.adapter_info
    }

    /// The context [`App::init`](crate::App::init) receives, so an app can be
    /// initialized offscreen the same way the windowed runner does it.
    pub fn init_context(&mut self) -> InitContext<'_> {
        InitContext {
            ui: &mut self.ui,
            renderer: &mut self.renderer,
            device: &self.device,
            queue: &self.queue,
            adapter_info: &self.adapter_info,
        }
    }

    /// Uploads an RGBA8 image.
    pub fn register_texture(&mut self, width: u32, height: u32, rgba: &[u8]) -> TextureId {
        self.renderer.register_texture(&self.device, &self.queue, width, height, rgba)
    }

    /// Queues an input event for the next frame.
    pub fn push_event(&mut self, e: InputEvent) {
        self.ui.push_event(e);
    }

    /// Builds one frame with `f` and uploads its GPU data (no pixels read back).
    pub fn frame(&mut self, dt: f32, f: impl FnOnce(&mut Ui)) -> FrameStats {
        let input = FrameInput { size: self.size, scale: self.scale, dt };
        if self.ui.begin_frame(input).is_err() {
            return FrameStats::default();
        }
        f(&mut self.ui);
        let vp = self.viewport();
        let out = self.ui.end_frame();
        self.renderer.prepare(&self.device, &self.queue, out, vp);
        out.stats
    }

    /// Renders the last built frame and waits for the GPU; returns wall time
    /// of submit + GPU completion in microseconds.
    pub fn render_and_wait(&mut self) -> Result<u64, RunError> {
        let vp = self.viewport();
        let t = std::time::Instant::now();
        render_to_rgba(&self.device, &self.queue, &mut self.renderer, self.ui.output(), vp, OPAQUE_BLACK)
            .map_err(|e| RunError::Graphics(e.to_string()))?;
        Ok(t.elapsed().as_micros() as u64)
    }

    /// Renders the last built frame into an offscreen texture (no readback)
    /// and blocks until the GPU finishes. Returns `(wall_us, gpu_us)`: wall
    /// time for prepare + encode + submit + wait, and the render pass's GPU
    /// execution time from timestamp queries (`None` if unsupported).
    pub fn render_offscreen_timed(&mut self) -> Result<(u64, Option<f64>), RunError> {
        let vp = self.viewport();
        self.ensure_target(vp);
        let timestamps = self.device.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let query = timestamps.then(|| {
            let set = self.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("rugi timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            });
            let resolve = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rugi ts resolve"),
                size: TIMESTAMPS_SIZE,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let read = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rugi ts read"),
                size: TIMESTAMPS_SIZE,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            (set, resolve, read)
        });
        let t = std::time::Instant::now();
        let out = self.ui.output();
        self.renderer.prepare(&self.device, &self.queue, out, vp);
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let (view, _) = self.target.as_ref().expect("ensured above");
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rugi bench pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: query.as_ref().map(|(set, ..)| wgpu::RenderPassTimestampWrites {
                    query_set: set,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.renderer.render(&mut pass, out);
        }
        if let Some((set, resolve, read)) = &query {
            enc.resolve_query_set(set, 0..2, resolve, 0);
            enc.copy_buffer_to_buffer(resolve, 0, read, 0, TIMESTAMPS_SIZE);
        }
        self.queue.submit(Some(enc.finish()));
        let mut gpu_us = None;
        if let Some((_, _, read)) = &query {
            read.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        }
        self.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| RunError::Graphics(e.to_string()))?;
        let wall = t.elapsed().as_micros() as u64;
        if let Some((_, _, read)) = &query {
            if let Ok(data) = read.slice(..).get_mapped_range() {
                let t0 = u64::from_le_bytes(data[0..8].try_into().expect("8 bytes"));
                let t1 = u64::from_le_bytes(data[8..16].try_into().expect("8 bytes"));
                let period = self.queue.get_timestamp_period() as f64;
                if t1 > t0 {
                    gpu_us = Some((t1 - t0) as f64 * period / 1000.0);
                }
            }
            read.unmap();
        }
        Ok((wall, gpu_us))
    }

    /// Submits `count` renders of the last built frame back to back and waits
    /// once; returns the mean wall time per frame in microseconds. With the GPU
    /// kept busy this approximates sustained GPU throughput.
    pub fn render_offscreen_throughput(&mut self, count: u32) -> Result<f64, RunError> {
        let vp = self.viewport();
        self.ensure_target(vp);
        let t = std::time::Instant::now();
        for _ in 0..count {
            let (view, _) = self.target.as_ref().expect("ensured above");
            self.renderer.render_to_view(&self.device, &self.queue, view, self.ui.output(), vp, OPAQUE_BLACK);
        }
        self.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| RunError::Graphics(e.to_string()))?;
        Ok(t.elapsed().as_secs_f64() * 1e6 / count as f64)
    }

    /// Renders the last built frame and returns `(width, height, rgba)`.
    pub fn capture(&mut self) -> Result<(u32, u32, Vec<u8>), RunError> {
        self.capture_with_clear(OPAQUE_BLACK)
    }

    /// [`HeadlessRunner::capture`] with an explicit clear color (e.g.
    /// transparent, to inspect what a transparent window would show).
    pub fn capture_with_clear(&mut self, clear: [f64; 4]) -> Result<(u32, u32, Vec<u8>), RunError> {
        let vp = self.viewport();
        let px = render_to_rgba(&self.device, &self.queue, &mut self.renderer, self.ui.output(), vp, clear)
            .map_err(|e| RunError::Graphics(e.to_string()))?;
        Ok((vp.size_px[0], vp.size_px[1], px))
    }

    /// See [`Renderer::set_straight_alpha_output`].
    pub fn set_straight_alpha_output(&mut self, on: bool) {
        self.renderer.set_straight_alpha_output(on);
    }

    /// Renderer statistics for the last frame.
    pub fn render_stats(&self) -> rugi_render::RenderStats {
        self.renderer.stats()
    }

    fn viewport(&self) -> Viewport {
        Viewport {
            size_px: [(self.size.x * self.scale).round() as u32, (self.size.y * self.scale).round() as u32],
            scale: self.scale,
        }
    }

    fn ensure_target(&mut self, vp: Viewport) {
        if self.target.as_ref().is_some_and(|(_, size)| *size == vp.size_px) {
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rugi bench target"),
            size: wgpu::Extent3d { width: vp.size_px[0], height: vp.size_px[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.target = Some((texture.create_view(&Default::default()), vp.size_px));
    }
}
