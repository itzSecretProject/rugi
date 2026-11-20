//! Offscreen rendering to CPU memory (screenshots, visual tests).

use crate::{Renderer, Viewport};
use rugi_core::FrameOutput;

/// Errors from headless rendering.
#[derive(Debug)]
pub enum HeadlessError {
    /// Mapping the readback buffer failed.
    Readback(String),
}

impl std::fmt::Display for HeadlessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeadlessError::Readback(e) => write!(f, "GPU readback failed: {e}"),
        }
    }
}

impl std::error::Error for HeadlessError {}

/// Renders `output` into a new `Rgba8Unorm` texture and reads it back as
/// tightly packed RGBA8 rows (sRGB-encoded). `renderer` must have been created
/// with `wgpu::TextureFormat::Rgba8Unorm`.
pub fn render_to_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &mut Renderer,
    output: &FrameOutput,
    viewport: Viewport,
    clear: [f64; 4],
) -> Result<Vec<u8>, HeadlessError> {
    let [w, h] = viewport.size_px;
    let texture = device.create_texture(&crate::texture_desc(
        "rugi offscreen",
        [w, h],
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    ));
    let view = texture.create_view(&Default::default());
    renderer.render_to_view(device, queue, &view, output, viewport, clear);

    let padded = (w * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rugi readback"),
        size: (padded * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(h) },
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit(Some(enc.finish()));
    let slice = buf.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| HeadlessError::Readback(e.to_string()))?;
    rx.recv()
        .map_err(|e| HeadlessError::Readback(e.to_string()))?
        .map_err(|e| HeadlessError::Readback(e.to_string()))?;
    let data = slice.get_mapped_range().map_err(|e| HeadlessError::Readback(format!("{e:?}")))?;
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let s = (row * padded) as usize;
        out.extend_from_slice(&data[s..s + (w * 4) as usize]);
    }
    drop(data);
    buf.unmap();
    Ok(out)
}
