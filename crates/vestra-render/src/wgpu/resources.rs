//! Persistent source textures, reusable working textures, and readback state.

use crate::{
    Category, Diagnostic, geometry::IntrinsicSize, plan::RenderPlan, render::DecodedAssets,
};

use super::texture_pool::TexturePool;

pub(super) struct FrameResources {
    pub(super) working: TexturePool,
    pub(super) padded_row_bytes: u32,
}

impl FrameResources {
    pub(super) fn create(device: &wgpu::Device, plan: &RenderPlan, padded_row_bytes: u32) -> Self {
        Self {
            working: TexturePool::create(device, plan),
            padded_row_bytes,
        }
    }
}

pub(super) struct PreparedRasterTexture {
    pub(super) _texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    pub(super) intrinsic_size: IntrinsicSize,
}

pub(super) struct SourceResources {
    pub(super) textures: Vec<PreparedRasterTexture>,
    pub(super) solid_texture: PreparedRasterTexture,
    pub(super) uploaded_texture_bytes: u64,
}

impl SourceResources {
    pub(super) fn create(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        plan: &RenderPlan,
        decoded: &DecodedAssets,
        max_texture_dimension_2d: u32,
    ) -> Result<Self, Diagnostic> {
        let mut textures = Vec::with_capacity(plan.images.len());
        let mut uploaded_texture_bytes = 0_u64;
        for asset in 0..plan.images.len() {
            let image = decoded.image(asset);
            if image.width() > max_texture_dimension_2d || image.height() > max_texture_dimension_2d
            {
                return Err(Diagnostic::error(
                    "WGPU-SOURCE-DIMENSIONS",
                    Category::Backend,
                    format!("source image {asset} exceeds adapter texture dimensions"),
                    "",
                ));
            }
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("vestra source"),
                size: wgpu::Extent3d {
                    width: image.width(),
                    height: image.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                image.as_raw(),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(image.width() * 4),
                    rows_per_image: Some(image.height()),
                },
                wgpu::Extent3d {
                    width: image.width(),
                    height: image.height(),
                    depth_or_array_layers: 1,
                },
            );
            uploaded_texture_bytes += u64::from(image.width()) * u64::from(image.height()) * 4;
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            textures.push(PreparedRasterTexture {
                _texture: texture,
                view,
                intrinsic_size: IntrinsicSize::new(image.width(), image.height()),
            });
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("vestra solid source"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Ok(Self {
            textures,
            solid_texture: PreparedRasterTexture {
                _texture: texture,
                view,
                intrinsic_size: IntrinsicSize::new(1, 1),
            },
            uploaded_texture_bytes,
        })
    }
}
