//! Half-resolution layer for soft procedural backdrops.
//!
//! Smooth, low-frequency backdrop kinds are rendered once per frame at half
//! resolution; the main pass then only composites them with a full-resolution
//! rounded mask and grain (`fs_backdrop_layer`).

use rugi_core::{BackdropKind, DrawList, PrimitiveKind, Rect};
use std::ops::Range;

/// How far a layered backdrop reaches beyond its rect: the fill pass grows
/// quads (`vs_backdrop_fill`) and bilinear sampling reads neighbors.
const FILL_MARGIN: f32 = 6.0;

#[derive(Default)]
pub(crate) struct BackdropLayer {
    target: Option<Target>,
    /// Sorted indices of the instances composited from the layer this frame.
    instances: Vec<u32>,
    ready: bool,
}

struct Target {
    size: [u32; 2],
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

impl BackdropLayer {
    /// Picks this frame's layered backdrops. Returns `false` if there are none.
    ///
    /// Each layer pixel must belong to exactly one backdrop, so a backdrop
    /// overlapping one already selected is left to the full-resolution path;
    /// otherwise a covered backdrop would sample another's pixels through its
    /// rounded corners and margins.
    pub fn select(&mut self, draw_list: &DrawList) -> bool {
        self.ready = false;
        self.instances.clear();
        if draw_list.backdrop_count() == 0 {
            return false;
        }
        let backdrop = PrimitiveKind::Backdrop as u32;
        let mut taken: Vec<Rect> = Vec::new();
        for (i, inst) in draw_list.instances().iter().enumerate() {
            if inst.kind == backdrop && is_layered_kind(inst.uv[3]) {
                let r = Rect::from_xywh(inst.rect[0], inst.rect[1], inst.rect[2], inst.rect[3]).expand(FILL_MARGIN);
                if taken.iter().all(|t| !t.intersects(&r)) {
                    taken.push(r);
                    self.instances.push(i as u32);
                }
            }
        }
        !self.instances.is_empty()
    }

    /// (Re)creates the layer texture if `size` changed and returns its view.
    pub fn target(
        &mut self,
        device: &wgpu::Device,
        image_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        size: [u32; 2],
    ) -> &wgpu::TextureView {
        if self.target.as_ref().is_none_or(|t| t.size != size) {
            let texture = device.create_texture(&crate::texture_desc(
                "rugi backdrop layer",
                size,
                format,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            ));
            let view = texture.create_view(&Default::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rugi backdrop layer"),
                layout: image_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
            });
            self.target = Some(Target { size, view, bind_group });
        }
        &self.target.as_ref().expect("created above").view
    }

    /// Contiguous runs of selected instances, one fill draw call each.
    pub fn runs(&self) -> impl Iterator<Item = Range<u32>> + '_ {
        let mut rest = self.instances.as_slice();
        std::iter::from_fn(move || {
            let (&start, _) = rest.split_first()?;
            let len = rest.iter().zip(start..).take_while(|(&idx, expected)| idx == *expected).count();
            rest = &rest[len..];
            Some(start..start + len as u32)
        })
    }

    pub fn set_ready(&mut self, ready: bool) {
        self.ready = ready;
    }

    /// The layer's bind group, once it holds this frame's backdrops.
    pub fn bind_group(&self) -> Option<&wgpu::BindGroup> {
        self.target.as_ref().filter(|_| self.ready).map(|t| &t.bind_group)
    }

    pub fn contains(&self, instance: u32) -> bool {
        self.instances.binary_search(&instance).is_ok()
    }
}

/// Backdrop kinds smooth enough to evaluate at half resolution.
fn is_layered_kind(kind: f32) -> bool {
    use BackdropKind::*;
    matches!(BackdropKind::from_u32(kind.round() as u32), Some(Aurora | Smoke | Plasma | Mesh))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_group_consecutive_indices() {
        let layer = BackdropLayer { instances: vec![1, 2, 3, 7, 9, 10], ..Default::default() };
        assert_eq!(layer.runs().collect::<Vec<_>>(), [1..4, 7..8, 9..11]);
    }
}
