//! Bind group layouts and render pipelines, created once per renderer.

use rugi_core::Instance;

/// Vertex attributes of [`Instance`]: rect, radii, uv, params, color0,
/// color1, border_color, kind.
const INSTANCE_ATTRS: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4,
    4 => Unorm8x4, 5 => Unorm8x4, 6 => Unorm8x4, 7 => Uint32
];

/// Which instance pipeline draws a run of instances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InstancePipeline {
    /// Rects, shadows, glyphs, images and lines (`fs_main`).
    Common,
    Glass,
    /// A backdrop evaluated at full resolution.
    Backdrop,
    /// A backdrop composited from the half-resolution layer.
    LayeredBackdrop,
}

pub(crate) struct Pipelines {
    /// Group 0: see `GlobalBindings`.
    pub globals_layout: wgpu::BindGroupLayout,
    /// Group 1: the image (or backdrop layer) texture.
    pub image_layout: wgpu::BindGroupLayout,
    /// Source texture and sampler of the post passes.
    pub post_layout: wgpu::BindGroupLayout,
    common: wgpu::RenderPipeline,
    glass: wgpu::RenderPipeline,
    backdrop: wgpu::RenderPipeline,
    layered_backdrop: wgpu::RenderPipeline,
    /// Writes soft backdrops into the half-resolution layer.
    pub backdrop_fill: wgpu::RenderPipeline,
    /// One 2× step of the blur chain.
    pub downsample: wgpu::RenderPipeline,
    /// Scene-to-target copy: `[premultiplied, straight alpha]`.
    pub copy: [wgpu::RenderPipeline; 2],
}

impl Pipelines {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rugi globals"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                texture_entry(1),
                sampler_entry(2),
                texture_entry(3),
                sampler_entry(4),
                texture_entry(5),
                sampler_entry(6),
            ],
        });
        let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rugi image"),
            entries: &[texture_entry(0)],
        });
        let post_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rugi post"),
            entries: &[texture_entry(0), sampler_entry(1)],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rugi shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rugi layout"),
            bind_group_layouts: &[Some(&globals_layout), Some(&image_layout)],
            immediate_size: 0,
        });
        let instance_pipeline = |vs: &str, fs: &str, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(fs),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Instance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &INSTANCE_ATTRS,
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let blended = |fs: &str| instance_pipeline("vs_main", fs, Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING));

        let post = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rugi post"),
            source: wgpu::ShaderSource::Wgsl(include_str!("post.wgsl").into()),
        });
        let post_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rugi post"),
            bind_group_layouts: &[Some(&post_layout)],
            immediate_size: 0,
        });
        let post_pipeline = |fs: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("rugi post"),
                layout: Some(&post_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &post,
                    entry_point: Some("vs_full"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &post,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };

        Self {
            common: blended("fs_main"),
            glass: blended("fs_glass"),
            backdrop: blended("fs_backdrop"),
            layered_backdrop: blended("fs_backdrop_layer"),
            backdrop_fill: instance_pipeline("vs_backdrop_fill", "fs_backdrop_fill", None),
            downsample: post_pipeline("fs_down"),
            copy: [post_pipeline("fs_copy"), post_pipeline("fs_copy_straight")],
            globals_layout,
            image_layout,
            post_layout,
        }
    }

    pub fn instance(&self, which: InstancePipeline) -> &wgpu::RenderPipeline {
        match which {
            InstancePipeline::Common => &self.common,
            InstancePipeline::Glass => &self.glass,
            InstancePipeline::Backdrop => &self.backdrop,
            InstancePipeline::LayeredBackdrop => &self.layered_backdrop,
        }
    }
}

/// A filterable 2D float texture visible to the fragment stage.
fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// A filtering sampler visible to the fragment stage.
fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}
