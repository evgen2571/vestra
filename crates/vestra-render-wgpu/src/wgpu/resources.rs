//! Persistent source textures, reusable working textures, and readback state.

use std::sync::Arc;
use vestra_core::plan::{RenderPlan, TextAnimationFrame};
use vestra_render_common::{ByteLruCache, CacheStats, PreparedText, TextRasterState};

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

pub(super) enum RasterSource {
    Texture(PreparedRasterTexture),
    Text(PreparedText),
}

impl RasterSource {
    pub(super) fn intrinsic_size(&self) -> IntrinsicSize {
        match self {
            Self::Texture(texture) => texture.intrinsic_size,
            Self::Text(text) => text.intrinsic_size(),
        }
    }

    pub(super) fn prepared_texture(&self) -> Option<&PreparedRasterTexture> {
        match self {
            Self::Texture(texture) => Some(texture),
            Self::Text(_) => None,
        }
    }
}

const TEXT_TEXTURE_CACHE_BYTES: u64 = 8 * 1024 * 1024;

pub(super) struct SourceResources {
    pub(super) rasters: Vec<RasterSource>,
    pub(super) prepared_raster_count: usize,
    text_textures: ByteLruCache<usize, (TextRasterState, Arc<PreparedRasterTexture>)>,
    pub(super) glyph_atlases: Vec<PreparedRasterTexture>,
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
            uploaded_texture_bytes += image.as_raw().len() as u64;
            textures.push(RasterSource::Texture(
                PreparedRasterTexture::create_and_upload(
                    device,
                    queue,
                    "vestra source",
                    image,
                    IntrinsicSize::new(image.width(), image.height()),
                ),
            ));
        }
        for (shape_index, _) in plan.shapes.iter().enumerate() {
            let prepared = decoded.shape(shape_index);
            let image = prepared.pixels().as_ref();
            if image.width() > max_texture_dimension_2d || image.height() > max_texture_dimension_2d
            {
                return Err(Diagnostic::error(
                    "WGPU-SOURCE-DIMENSIONS",
                    Category::Backend,
                    "prepared shape exceeds adapter texture dimensions",
                    "",
                ));
            }
            uploaded_texture_bytes += image.as_raw().len() as u64;
            textures.push(RasterSource::Texture(
                PreparedRasterTexture::create_and_upload(
                    device,
                    queue,
                    "vestra prepared shape",
                    image,
                    prepared.intrinsic_size(),
                ),
            ));
        }
        textures.extend(
            (0..plan.texts.len()).map(|index| RasterSource::Text(decoded.text(index).clone())),
        );
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
            uploaded_texture_bytes += image.as_raw().len() as u64;
            textures.push(RasterSource::Texture(
                PreparedRasterTexture::create_and_upload(
                    device,
                    queue,
                    "vestra dynamic video source",
                    image,
                    IntrinsicSize::new(image.width(), image.height()),
                ),
            ));
        }
        let mut glyph_atlases = Vec::with_capacity(plan.glyph_atlases.len());
        for atlas in decoded.glyph_atlases() {
            let image = &atlas.levels()[0];
            if image.width() > max_texture_dimension_2d || image.height() > max_texture_dimension_2d
            {
                return Err(Diagnostic::error(
                    "WGPU-SOURCE-DIMENSIONS",
                    Category::Backend,
                    "glyph atlas exceeds adapter texture dimensions",
                    "",
                ));
            }
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("vestra glyph atlas"),
                size: wgpu::Extent3d {
                    width: image.width(),
                    height: image.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 5,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            for (level, image) in atlas.levels().iter().enumerate() {
                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &texture,
                        mip_level: level as u32,
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
            }
            uploaded_texture_bytes += atlas.byte_len();
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            glyph_atlases.push(PreparedRasterTexture {
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
            prepared_raster_count: textures.len() - plan.texts.len(),
            rasters: textures,
            text_textures: ByteLruCache::new(
                TEXT_TEXTURE_CACHE_BYTES.min(plan.limits.maximum_cache_bytes),
            ),
            glyph_atlases,
            solid_texture: PreparedRasterTexture {
                _texture: texture,
                view,
                intrinsic_size: IntrinsicSize::new(1, 1),
            },
            uploaded_texture_bytes,
        })
    }

    pub(super) fn text_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source_index: usize,
        animation: TextAnimationFrame,
    ) -> Result<Arc<PreparedRasterTexture>, Diagnostic> {
        let Some(RasterSource::Text(text)) = self.rasters.get(source_index) else {
            return Err(Diagnostic::error(
                "WGPU-FRAME-PLAN",
                Category::Backend,
                format!("GPU frame operation references missing text source {source_index}"),
                "",
            ));
        };
        let state = text.raster_state(animation);
        if let Some((uploaded, texture)) = self.text_textures.get(&source_index)
            && *uploaded == state
        {
            return Ok(Arc::clone(texture));
        }

        let pixels = text.pixels_at(state);
        // Keep pixels immutable while encoded or submitted frames still bind this texture.
        let raster = Arc::new(PreparedRasterTexture::create_and_upload(
            device,
            queue,
            "vestra text source",
            &pixels,
            text.intrinsic_size(),
        ));
        self.text_textures.insert(
            source_index,
            (state, Arc::clone(&raster)),
            pixels.as_raw().len() as u64,
        );
        Ok(raster)
    }

    pub(super) fn text_cache_stats(&self) -> CacheStats {
        self.text_textures.stats()
    }

    pub(super) fn upload_video(
        &self,
        queue: &wgpu::Queue,
        source_index: usize,
        pixels: &RgbaImage,
    ) -> Result<(), Diagnostic> {
        let source = self
            .rasters
            .get(source_index)
            .and_then(RasterSource::prepared_texture)
            .ok_or_else(|| {
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
        source.upload(queue, pixels);
        Ok(())
    }
}

impl PreparedRasterTexture {
    fn create_and_upload(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        pixels: &RgbaImage,
        intrinsic_size: IntrinsicSize,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: pixels.width(),
                height: pixels.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let raster = Self {
            view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            _texture: texture,
            intrinsic_size,
        };
        raster.upload(queue, pixels);
        raster
    }

    fn upload(&self, queue: &wgpu::Queue, pixels: &RgbaImage) {
        debug_assert_eq!(
            (self.intrinsic_size.width, self.intrinsic_size.height),
            pixels.dimensions()
        );
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self._texture,
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
    }
}
