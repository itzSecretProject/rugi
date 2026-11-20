//! Rugi GPU renderer built on `wgpu`.
//!
//! Every primitive in a [`rugi_core::FrameOutput`] is one instance of a
//! 4-vertex triangle strip; the fragment shader evaluates analytic coverage
//! (rounded rects with borders and gradients, Gaussian box shadows,
//! round-capped lines) or samples the glyph atlas or an image. One draw call is
//! issued per [`rugi_core::DrawCommand`], split further only where effect
//! primitives need their own pipeline.
//!
//! GPU resources are persistent: the instance buffer grows geometrically, the
//! glyph atlas is updated with `write_texture` on its dirty region only, and
//! pipelines and samplers are created once.
//!
//! The renderer never owns the device or the surface, so it can record into a
//! render pass owned by a host application that already uses wgpu
//! ([`Renderer::prepare`] + [`Renderer::render`]).
//!
//! # Glass
//!
//! Frames containing glass are drawn by [`Renderer::render_to_view`] into an
//! offscreen scene texture. When the instance stream reaches a glass primitive
//! with new content behind it, the pass ends, the scene is downsampled into a
//! blur mip chain, and drawing resumes with the chain bound so glass can sample
//! a blurred, refracted copy of everything beneath it. The scene is then copied
//! to the target. With a host-owned pass ([`Renderer::render`]) there is no
//! snapshot and glass falls back to its tint.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod backdrop_layer;
mod glass;
mod globals;
mod headless;
mod noise;
mod pipelines;
mod renderer;

pub use headless::{render_to_rgba, HeadlessError};
pub use renderer::Renderer;

/// Target surface dimensions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// Physical size in pixels.
    pub size_px: [u32; 2],
    /// Physical pixels per logical pixel.
    pub scale: f32,
}

/// Renderer statistics for the last prepared frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStats {
    /// Draw calls issued.
    pub draw_calls: u32,
    /// Instances uploaded.
    pub instances: u32,
    /// Bytes uploaded to the GPU (instances, atlas and uniforms).
    pub upload_bytes: u64,
    /// Instance buffer capacity, in instances.
    pub instance_capacity: u32,
}

/// Descriptor for a single-mip 2D texture.
fn texture_desc(
    label: &str,
    [width, height]: [u32; 2],
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::TextureDescriptor<'_> {
    wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    }
}

/// Begins a render pass with a single color attachment.
fn begin_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    label: &str,
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}
