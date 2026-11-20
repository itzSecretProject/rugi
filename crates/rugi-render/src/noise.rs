//! Tiling random texture sampled by the procedural backdrops.

use wgpu::util::DeviceExt as _;

/// Edge length of the noise texture in texels.
const SIZE: u32 = 256;

/// A `SIZE`×`SIZE` single-channel random texture with a repeating sampler.
pub(crate) struct Noise {
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Noise {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        // A fixed xorshift sequence, so backdrops look identical on every machine.
        let mut state = 0x9E37_79B9u32;
        let data: Vec<u8> = (0..SIZE * SIZE)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state >> 24) as u8
            })
            .collect();
        let texture = device.create_texture_with_data(
            queue,
            &crate::texture_desc(
                "rugi noise",
                [SIZE, SIZE],
                wgpu::TextureFormat::R8Unorm,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            ),
            wgpu::util::TextureDataOrder::LayerMajor,
            &data,
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("rugi noise sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self { view: texture.create_view(&Default::default()), sampler }
    }
}
