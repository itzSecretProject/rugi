//! [`Renderer`]: per-frame uploads and pass recording.

use crate::backdrop_layer::BackdropLayer;
use crate::glass::{snapshot_barriers, GlassTargets};
use crate::globals::{GlobalBindings, Globals};
use crate::pipelines::{InstancePipeline, Pipelines};
use crate::{begin_pass, RenderStats, Viewport};
use rugi_core::{FrameOutput, Instance, PrimitiveKind, Rect, TextureId};
use std::collections::HashMap;
use wgpu::util::DeviceExt as _;

/// Initial glyph atlas edge length; replaced by the first atlas update.
const INITIAL_ATLAS_SIZE: u32 = 1024;
/// Initial instance buffer capacity.
const INITIAL_INSTANCE_CAPACITY: u64 = 1024;

/// The wgpu renderer.
pub struct Renderer {
    pipelines: Pipelines,
    globals: GlobalBindings,
    /// Bind group 0 with the placeholder blur texture.
    globals_bg: wgpu::BindGroup,
    atlas: wgpu::Texture,
    atlas_size: u32,
    instance_buf: wgpu::Buffer,
    instance_cap: u64,
    textures: HashMap<TextureId, UserTexture>,
    next_texture: u64,
    format: wgpu::TextureFormat,
    output_srgb: bool,
    straight_alpha: bool,
    viewport: Viewport,
    stats: RenderStats,
    backdrop_layer: BackdropLayer,
    glass: Option<GlassTargets>,
    /// Whether the current frame is drawn through the offscreen scene.
    glass_frame: bool,
    /// Snapshot points of the current frame, see [`snapshot_barriers`].
    barriers: Vec<u32>,
}

struct UserTexture {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

impl std::fmt::Debug for Renderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Renderer").field("stats", &self.stats).finish_non_exhaustive()
    }
}

impl Renderer {
    /// Creates a renderer drawing into targets of `format`.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let pipelines = Pipelines::new(device, format);
        let globals = GlobalBindings::new(device, queue);
        let atlas = create_atlas(device, INITIAL_ATLAS_SIZE);
        let globals_bg = globals.bind_group(device, &pipelines.globals_layout, &atlas, None);
        let mut renderer = Self {
            pipelines,
            globals,
            globals_bg,
            atlas,
            atlas_size: INITIAL_ATLAS_SIZE,
            instance_buf: create_instance_buf(device, INITIAL_INSTANCE_CAPACITY),
            instance_cap: INITIAL_INSTANCE_CAPACITY,
            textures: HashMap::new(),
            next_texture: 1,
            format,
            output_srgb: format.is_srgb(),
            straight_alpha: false,
            viewport: Viewport { size_px: [1, 1], scale: 1.0 },
            stats: RenderStats::default(),
            backdrop_layer: BackdropLayer::default(),
            glass: None,
            glass_frame: false,
            barriers: Vec::new(),
        };
        // `TextureId::NONE` maps to 1×1 white so group 1 is always bindable.
        let white = renderer.create_user_texture(device, queue, 1, 1, &[255, 255, 255, 255]);
        renderer.textures.insert(TextureId::NONE, white);
        renderer
    }

    /// Uploads an RGBA8 image (sRGB-encoded, straight alpha) for use with
    /// image widgets.
    pub fn register_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> TextureId {
        assert_eq!(rgba.len(), (width * height * 4) as usize, "rgba length must be width*height*4");
        let texture = self.create_user_texture(device, queue, width, height, rgba);
        let id = TextureId(self.next_texture);
        self.next_texture += 1;
        self.textures.insert(id, texture);
        id
    }

    /// Releases a texture registered with [`Renderer::register_texture`].
    pub fn unregister_texture(&mut self, id: TextureId) {
        if id != TextureId::NONE {
            self.textures.remove(&id);
        }
    }

    /// Makes [`Renderer::render_to_view`] output straight (non-premultiplied)
    /// alpha, for compositors in `PostMultiplied` mode (e.g. transparent
    /// windows on macOS). Costs one full-screen copy per frame.
    pub fn set_straight_alpha_output(&mut self, on: bool) {
        self.straight_alpha = on;
    }

    /// Statistics for the last prepared frame.
    pub fn stats(&self) -> RenderStats {
        self.stats
    }

    /// Uploads per-frame data (uniforms, atlas changes, instances). Must be
    /// called before [`Renderer::render`] each frame.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, output: &FrameOutput, viewport: Viewport) {
        self.prepare_frame(device, queue, output, viewport, false);
    }

    /// Records draw calls into an existing render pass. The pass must target
    /// the format given to [`Renderer::new`] with the viewport passed to
    /// [`Renderer::prepare`].
    pub fn render(&mut self, pass: &mut wgpu::RenderPass<'_>, output: &FrameOutput) {
        let n = output.draw_list.instances().len() as u32;
        self.stats.draw_calls = self.render_range(pass, output, 0..n, false);
    }

    /// Prepares and renders `output` into `view`, clearing it first with
    /// `clear` (sRGB-encoded RGBA in 0..=1), and submits.
    pub fn render_to_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        output: &FrameOutput,
        viewport: Viewport,
        clear: [f64; 4],
    ) {
        self.prepare_frame(device, queue, output, viewport, true);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("rugi frame") });
        self.encode_backdrop_layer(device, &mut encoder, output, viewport);
        let clear = clear_color(clear, self.output_srgb);
        if !self.glass_frame {
            let mut pass = begin_pass(&mut encoder, "rugi pass", view, wgpu::LoadOp::Clear(clear));
            self.render(&mut pass, output);
        } else {
            let glass = self.glass.as_ref().expect("glass targets are created in prepare");
            let n = output.draw_list.instances().len() as u32;
            let mut calls = 0;
            let mut start = 0;
            for (i, stop) in self.barriers.iter().copied().chain([n]).enumerate() {
                let load = if i == 0 { wgpu::LoadOp::Clear(clear) } else { wgpu::LoadOp::Load };
                {
                    let mut pass = begin_pass(&mut encoder, "rugi scene pass", &glass.scene_view, load);
                    calls += self.render_range(&mut pass, output, start..stop, true);
                }
                if stop < n {
                    calls += glass.encode_blur(&mut encoder, &self.pipelines);
                }
                start = stop;
            }
            glass.encode_copy(&mut encoder, &self.pipelines, view, clear, self.straight_alpha);
            self.stats.draw_calls = calls + 1;
        }
        self.backdrop_layer.set_ready(false);
        queue.submit(Some(encoder.finish()));
    }

    fn create_user_texture(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> UserTexture {
        let texture = device.create_texture_with_data(
            queue,
            &crate::texture_desc(
                "rugi image",
                [width, height],
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            ),
            wgpu::util::TextureDataOrder::LayerMajor,
            rgba,
        );
        let view = texture.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rugi image"),
            layout: &self.pipelines.image_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
        });
        UserTexture { _texture: texture, bind_group }
    }

    /// `offscreen` allows the glass path; only [`Renderer::render_to_view`]
    /// owns the passes needed for it.
    fn prepare_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output: &FrameOutput,
        viewport: Viewport,
        offscreen: bool,
    ) {
        let mut upload = 0u64;
        self.viewport = viewport;
        if let Some(update) = &output.atlas_update {
            if update.size != self.atlas_size {
                self.atlas = create_atlas(device, update.size);
                self.atlas_size = update.size;
                self.globals_bg = self.globals.bind_group(device, &self.pipelines.globals_layout, &self.atlas, None);
                // The glass bind group references the old atlas.
                self.glass = None;
            }
            let [x, y, w, h] = update.region;
            if w > 0 && h > 0 {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.atlas,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x, y, z: 0 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &update.data,
                    wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w), rows_per_image: Some(h) },
                    wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                );
                upload += update.data.len() as u64;
            }
        }

        // Straight-alpha output also goes through the scene texture so the
        // final copy can un-premultiply.
        let draw_list = &output.draw_list;
        self.glass_frame = offscreen && (draw_list.glass_count() > 0 || self.straight_alpha);
        if self.glass_frame {
            if self.glass.as_ref().is_none_or(|g| g.size != viewport.size_px) {
                self.glass = Some(GlassTargets::new(
                    device,
                    self.format,
                    viewport.size_px,
                    &self.pipelines,
                    &self.globals,
                    &self.atlas,
                ));
            }
            if draw_list.glass_count() > 0 {
                snapshot_barriers(draw_list.instances(), &mut self.barriers);
            } else {
                self.barriers.clear();
            }
        }
        let globals = Globals {
            screen: [viewport.size_px[0] as f32 / viewport.scale, viewport.size_px[1] as f32 / viewport.scale],
            scale: viewport.scale,
            output_srgb: if self.output_srgb { 1.0 } else { 0.0 },
            blur_ready: if self.glass_frame { 1.0 } else { 0.0 },
            blur_mips: self.glass.as_ref().map_or(1.0, |g| g.mips as f32),
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.globals.uniforms, 0, bytemuck::bytes_of(&globals));
        upload += std::mem::size_of::<Globals>() as u64;

        let instances = draw_list.instances();
        let needed = instances.len() as u64;
        if needed > self.instance_cap {
            self.instance_cap = needed.next_power_of_two();
            self.instance_buf = create_instance_buf(device, self.instance_cap);
        }
        if !instances.is_empty() {
            queue.write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(instances));
            upload += std::mem::size_of_val(instances) as u64;
        }
        self.stats = RenderStats {
            draw_calls: 0,
            instances: instances.len() as u32,
            upload_bytes: upload,
            instance_capacity: self.instance_cap as u32,
        };
    }

    /// Renders the frame's layered backdrops into the half-resolution layer.
    fn encode_backdrop_layer(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        output: &FrameOutput,
        viewport: Viewport,
    ) {
        if !self.backdrop_layer.select(&output.draw_list) {
            return;
        }
        let size = [viewport.size_px[0].div_ceil(2).max(1), viewport.size_px[1].div_ceil(2).max(1)];
        let target = self.backdrop_layer.target(device, &self.pipelines.image_layout, self.format, size);
        let mut pass =
            begin_pass(encoder, "rugi backdrop layer pass", target, wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT));
        pass.set_pipeline(&self.pipelines.backdrop_fill);
        pass.set_bind_group(0, &self.globals_bg, &[]);
        pass.set_bind_group(1, &self.textures[&TextureId::NONE].bind_group, &[]);
        pass.set_vertex_buffer(
            0,
            self.instance_buf.slice(..std::mem::size_of_val(output.draw_list.instances()) as u64),
        );
        for run in self.backdrop_layer.runs() {
            pass.draw(0..4, run);
        }
        drop(pass);
        self.backdrop_layer.set_ready(true);
    }

    /// Records the instances in `range`, one draw call per command and
    /// pipeline run. Returns the number of draw calls.
    fn render_range(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        output: &FrameOutput,
        range: std::ops::Range<u32>,
        blur_bound: bool,
    ) -> u32 {
        let instances = output.draw_list.instances();
        if instances.is_empty() || range.is_empty() {
            return 0;
        }
        let globals = match (&self.glass, blur_bound) {
            (Some(g), true) => &g.globals_bg,
            _ => &self.globals_bg,
        };
        pass.set_pipeline(self.pipelines.instance(InstancePipeline::Common));
        pass.set_bind_group(0, globals, &[]);
        pass.set_vertex_buffer(0, self.instance_buf.slice(..std::mem::size_of_val(instances) as u64));

        let effects = output.draw_list.glass_count() > 0 || output.draw_list.backdrop_count() > 0;
        let layer = self.backdrop_layer.bind_group();
        let pipeline_for = |idx: u32| -> InstancePipeline {
            match instances[idx as usize].kind {
                k if k == PrimitiveKind::Glass as u32 => InstancePipeline::Glass,
                k if k == PrimitiveKind::Backdrop as u32 => {
                    if layer.is_some() && self.backdrop_layer.contains(idx) {
                        InstancePipeline::LayeredBackdrop
                    } else {
                        InstancePipeline::Backdrop
                    }
                }
                _ => InstancePipeline::Common,
            }
        };
        let mut bound: Option<TextureId> = None;
        let mut current = InstancePipeline::Common;
        let mut calls = 0;
        for cmd in output.draw_list.commands() {
            let (start, end) = (cmd.start.max(range.start), cmd.end.min(range.end));
            if start >= end {
                continue;
            }
            let Some([x, y, w, h]) = scissor(cmd.clip, self.viewport) else { continue };
            pass.set_scissor_rect(x, y, w, h);
            let tex = if self.textures.contains_key(&cmd.texture) { cmd.texture } else { TextureId::NONE };
            if bound != Some(tex) {
                pass.set_bind_group(1, &self.textures[&tex].bind_group, &[]);
                bound = Some(tex);
            }
            if !effects {
                pass.draw(0..4, start..end);
                calls += 1;
                continue;
            }
            // Split the command into runs that share a pipeline.
            let mut run_start = start;
            while run_start < end {
                let pipeline = pipeline_for(run_start);
                let mut run_end = run_start + 1;
                while run_end < end && pipeline_for(run_end) == pipeline {
                    run_end += 1;
                }
                if pipeline != current {
                    pass.set_pipeline(self.pipelines.instance(pipeline));
                    current = pipeline;
                }
                match (pipeline, layer) {
                    (InstancePipeline::LayeredBackdrop, Some(layer)) => {
                        pass.set_bind_group(1, layer, &[]);
                        bound = None;
                    }
                    _ if bound != Some(tex) => {
                        pass.set_bind_group(1, &self.textures[&tex].bind_group, &[]);
                        bound = Some(tex);
                    }
                    _ => {}
                }
                pass.draw(0..4, run_start..run_end);
                calls += 1;
                run_start = run_end;
            }
        }
        calls
    }
}

fn create_atlas(device: &wgpu::Device, size: u32) -> wgpu::Texture {
    device.create_texture(&crate::texture_desc(
        "rugi glyph atlas",
        [size, size],
        wgpu::TextureFormat::R8Unorm,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    ))
}

fn create_instance_buf(device: &wgpu::Device, capacity: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rugi instances"),
        size: capacity * std::mem::size_of::<Instance>() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// `clip` in physical pixels, clamped to the target, as `[x, y, w, h]`.
/// `None` if nothing is visible.
fn scissor(clip: Rect, viewport: Viewport) -> Option<[u32; 4]> {
    let [w, h] = viewport.size_px.map(|v| v as f32);
    let s = viewport.scale;
    let x0 = (clip.min.x * s).floor().clamp(0.0, w) as u32;
    let y0 = (clip.min.y * s).floor().clamp(0.0, h) as u32;
    let x1 = (clip.max.x * s).ceil().clamp(0.0, w) as u32;
    let y1 = (clip.max.y * s).ceil().clamp(0.0, h) as u32;
    (x1 > x0 && y1 > y0).then(|| [x0, y0, x1 - x0, y1 - y0])
}

/// Converts an sRGB-encoded clear color to what the target expects.
fn clear_color([r, g, b, a]: [f64; 4], output_srgb: bool) -> wgpu::Color {
    if !output_srgb {
        return wgpu::Color { r, g, b, a };
    }
    let linear = |c: f64| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    wgpu::Color { r: linear(r), g: linear(g), b: linear(b), a }
}
