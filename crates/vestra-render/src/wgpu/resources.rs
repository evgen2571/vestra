//! Persistent source textures, reusable working textures, and readback state.

use std::sync::Arc;
use vestra_core::plan::RenderPlan;

use crate::{Category, Diagnostic, geometry::IntrinsicSize, render::DecodedAssets};
use image::RgbaImage;

use super::{texture_pool::TexturePool, topology::PlanTopology};

pub(super) struct FrameResources {
    pub(super) working: TexturePool,
    pub(super) padded_row_bytes: u32,
}

impl FrameResources {
    pub(super) fn create_with_topology(
        device: &wgpu::Device,
        plan: &RenderPlan,
        topology: &PlanTopology,
        padded_row_bytes: u32,
    ) -> Self {
        Self {
            working: TexturePool::create_with_topology(device, plan, topology),
            padded_row_bytes,
        }
    }
}

pub(super) struct PreparedRasterTexture {
    pub(super) view: wgpu::TextureView,
    pub(super) _texture: wgpu::Texture,
    pub(super) intrinsic_size: IntrinsicSize,
}

pub(super) struct SourceResources {
    pub(super) raster_textures: Vec<PreparedRasterTexture>,
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
        dynamic_frames: &[Arc<RgbaImage>],
    ) -> Result<Self, Diagnostic> {
        let mut textures =
            Vec::with_capacity(plan.images.len() + plan.shapes.len() + plan.texts.len());
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
        for (shape_index, _) in plan.shapes.iter().enumerate() {
            let prepared = decoded.shape(shape_index);
            let image = prepared.pixels.as_ref();
            if image.width() > max_texture_dimension_2d || image.height() > max_texture_dimension_2d
            {
                return Err(Diagnostic::error(
                    "WGPU-SOURCE-DIMENSIONS",
                    Category::Backend,
                    "prepared shape exceeds adapter texture dimensions",
                    "",
                ));
            }
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("vestra prepared shape"),
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
                intrinsic_size: prepared.intrinsic_size,
            });
        }
        for text_index in 0..plan.texts.len() {
            let prepared = decoded.text(text_index);
            let image = prepared.pixels.as_ref();
            if image.width() > max_texture_dimension_2d || image.height() > max_texture_dimension_2d
            {
                return Err(Diagnostic::error(
                    "WGPU-SOURCE-DIMENSIONS",
                    Category::Backend,
                    "prepared text exceeds adapter texture dimensions",
                    "",
                ));
            }
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("vestra prepared text"),
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
                intrinsic_size: prepared.intrinsic_size,
            });
        }
        for image in dynamic_frames {
            if image.width() > max_texture_dimension_2d || image.height() > max_texture_dimension_2d
            {
                return Err(Diagnostic::error(
                    "WGPU-SOURCE-DIMENSIONS",
                    Category::Backend,
                    "decoded video frame exceeds adapter texture dimensions",
                    "",
                ));
            }
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("vestra dynamic video source"),
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
            raster_textures: textures,
            solid_texture: PreparedRasterTexture {
                _texture: texture,
                view,
                intrinsic_size: IntrinsicSize::new(1, 1),
            },
            uploaded_texture_bytes,
        })
    }

    pub(super) fn upload_video(
        &self,
        queue: &wgpu::Queue,
        source_index: usize,
        pixels: &RgbaImage,
    ) -> Result<(), Diagnostic> {
        let source = self.raster_textures.get(source_index).ok_or_else(|| {
            Diagnostic::error(
                "WGPU-VIDEO-SOURCE",
                Category::Backend,
                "dynamic video source index is out of range",
                "",
            )
        })?;
        if source.intrinsic_size.width != pixels.width()
            || source.intrinsic_size.height != pixels.height()
        {
            return Err(Diagnostic::error(
                "WGPU-VIDEO-DIMENSIONS",
                Category::Backend,
                "video frame dimensions changed during rendering",
                "",
            ));
        }
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &source._texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels.as_raw(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(pixels.width() * 4),
                rows_per_image: Some(pixels.height()),
            },
            wgpu::Extent3d {
                width: pixels.width(),
                height: pixels.height(),
                depth_or_array_layers: 1,
            },
        );
        Ok(())
    }
}
