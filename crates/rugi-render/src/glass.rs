//! Offscreen scene and blur chain for frames that contain glass.

use crate::globals::GlobalBindings;
use crate::pipelines::Pipelines;
use rugi_core::{Instance, PrimitiveKind};

/// Maximum blur snapshots per frame; later glass reuses the last one.
const MAX_SNAPSHOTS: usize = 8;
/// Mip levels of the blur chain (level 0 is half resolution).
const BLUR_MIPS: u32 = 6;

/// Render targets for one viewport size.
pub(crate) struct GlassTargets {
    pub size: [u32; 2],
    pub scene_view: wgpu::TextureView,
    mip_views: Vec<wgpu::TextureView>,
    /// Sources of the downsample passes: `[scene, mip 0, mip 1, ...]`. The
    /// scene entry also feeds the final copy to the target.
    down_sources: Vec<wgpu::BindGroup>,
    /// Bind group 0 with the blur chain bound.
    pub globals_bg: wgpu::BindGroup,
    pub mips: u32,
}

impl GlassTargets {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: [u32; 2],
        pipelines: &Pipelines,
        globals: &GlobalBindings,
        atlas: &wgpu::Texture,
    ) -> Self {
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let [w, h] = [size[0].max(1), size[1].max(1)];
        let scene = device.create_texture(&crate::texture_desc("rugi scene", [w, h], format, usage));
        let (bw, bh) = ((w / 2).max(1), (h / 2).max(1));
        let mips = BLUR_MIPS.min(32 - bw.min(bh).leading_zeros()).max(1);
        let blur = device.create_texture(&wgpu::TextureDescriptor {
            mip_level_count: mips,
            ..crate::texture_desc("rugi blur chain", [bw, bh], format, usage)
        });
        let scene_view = scene.create_view(&Default::default());
        let mip_views: Vec<wgpu::TextureView> = (0..mips)
            .map(|m| {
                blur.create_view(&wgpu::TextureViewDescriptor {
                    base_mip_level: m,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let source = |view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rugi post"),
                layout: &pipelines.post_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&globals.blur_sampler),
                    },
                ],
            })
        };
        let down_sources = std::iter::once(&scene_view).chain(&mip_views[..mips as usize - 1]).map(source).collect();
        let full_chain = blur.create_view(&Default::default());
        let globals_bg = globals.bind_group(device, &pipelines.globals_layout, atlas, Some(&full_chain));
        Self { size, scene_view, mip_views, down_sources, globals_bg, mips }
    }

    /// Downsamples the scene into the blur chain. Returns the draw calls issued.
    pub fn encode_blur(&self, encoder: &mut wgpu::CommandEncoder, pipelines: &Pipelines) -> u32 {
        for (target, source) in self.mip_views.iter().zip(&self.down_sources) {
            let mut pass =
                crate::begin_pass(encoder, "rugi blur pass", target, wgpu::LoadOp::Clear(wgpu::Color::BLACK));
            pass.set_pipeline(&pipelines.downsample);
            pass.set_bind_group(0, source, &[]);
            pass.draw(0..3, 0..1);
        }
        self.mip_views.len() as u32
    }

    /// Copies the scene to `view`, un-premultiplying if `straight_alpha`.
    pub fn encode_copy(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &Pipelines,
        view: &wgpu::TextureView,
        clear: wgpu::Color,
        straight_alpha: bool,
    ) {
        let mut pass = crate::begin_pass(encoder, "rugi present copy", view, wgpu::LoadOp::Clear(clear));
        pass.set_pipeline(&pipelines.copy[straight_alpha as usize]);
        pass.set_bind_group(0, &self.down_sources[0], &[]);
        pass.draw(0..3, 0..1);
    }
}

/// Fills `barriers` with the instance indices at which the frame must be
/// snapshotted and blurred: each glass primitive with new content drawn since
/// the previous snapshot.
pub(crate) fn snapshot_barriers(instances: &[Instance], barriers: &mut Vec<u32>) {
    let glass = PrimitiveKind::Glass as u32;
    let none = PrimitiveKind::None as u32;
    let mut dirty = true;
    barriers.clear();
    for (i, inst) in instances.iter().enumerate() {
        if inst.kind == glass {
            if dirty && barriers.len() < MAX_SNAPSHOTS {
                barriers.push(i as u32);
                dirty = false;
            }
        } else if inst.kind != none {
            dirty = true;
        }
    }
}
