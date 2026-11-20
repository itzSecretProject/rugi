//! Bind group 0: frame uniforms, glyph atlas, blur chain and noise.

use crate::noise::Noise;
use wgpu::util::DeviceExt as _;

/// Frame uniforms; mirrors `Globals` in `shader.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Globals {
    /// Logical target size.
    pub screen: [f32; 2],
    pub scale: f32,
    /// 1.0 when the target applies sRGB encoding on store.
    pub output_srgb: f32,
    /// 1.0 when the blur texture holds a snapshot of this frame.
    pub blur_ready: f32,
    pub blur_mips: f32,
    pub _pad: [f32; 2],
}

/// The long-lived resources of bind group 0. The atlas and blur views change
/// over time, so bind groups are built on demand with [`Self::bind_group`].
pub(crate) struct GlobalBindings {
    pub uniforms: wgpu::Buffer,
    sampler: wgpu::Sampler,
    /// Trilinear, clamped sampler for the blur chain and the post passes.
    pub blur_sampler: wgpu::Sampler,
    /// 1×1 black stand-in bound while no blur snapshot exists.
    placeholder_blur: wgpu::TextureView,
    noise: Noise,
}

impl GlobalBindings {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rugi globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("rugi sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let blur_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("rugi blur sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let placeholder_blur = device
            .create_texture_with_data(
                queue,
                &crate::texture_desc(
                    "rugi blur placeholder",
                    [1, 1],
                    wgpu::TextureFormat::Rgba8Unorm,
                    wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                ),
                wgpu::util::TextureDataOrder::LayerMajor,
                &[0, 0, 0, 255],
            )
            .create_view(&Default::default());
        Self { uniforms, sampler, blur_sampler, placeholder_blur, noise: Noise::new(device, queue) }
    }

    /// Builds bind group 0, binding the placeholder when `blur` is `None`.
    pub fn bind_group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        atlas: &wgpu::Texture,
        blur: Option<&wgpu::TextureView>,
    ) -> wgpu::BindGroup {
        let atlas = atlas.create_view(&Default::default());
        let blur = blur.unwrap_or(&self.placeholder_blur);
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rugi globals"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniforms.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&atlas) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(blur) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&self.blur_sampler) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&self.noise.view) },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::Sampler(&self.noise.sampler) },
            ],
        })
    }
}
