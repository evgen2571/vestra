//! Headless WGPU resource ownership for the render backend.

#![allow(
    clippy::result_large_err,
    reason = "WGPU preparation retains structured user-facing diagnostics"
)]

use std::{sync::Arc, time::Instant};

use image::RgbaImage;

use crate::{
    Category, Diagnostic,
    plan::{EvaluatedFrame, RenderPlan},
    render::{
        AdapterMetadata, RenderBackend, RenderBackendKind, compositor,
        prepared::{DecodedAssets, PreparationStats, PreparationTimings, PreparedAssets},
    },
};

/// A headless WGPU session. Its textures, output target, staging buffer and
/// source uploads persist for the complete render lifetime.
pub struct WgpuBackend {
    _instance: wgpu::Instance,
    _adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    _source_textures: Vec<wgpu::Texture>,
    output: wgpu::Texture,
    readback: wgpu::Buffer,
    row_bytes: u32,
    padded_row_bytes: u32,
    frame_bytes: Vec<u8>,
    cpu_reference: PreparedAssets,
    stats: PreparationStats,
    timings: PreparationTimings,
    adapter: AdapterMetadata,
}

impl WgpuBackend {
    pub fn new(plan: &RenderPlan, decoded: Arc<DecodedAssets>) -> Result<Self, Diagnostic> {
        let started = Instant::now();
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: std::env::var_os("VIDEO_EDITOR_WGPU_FORCE_FALLBACK").is_some(),
            compatible_surface: None,
        }))
        .ok_or_else(|| {
            Diagnostic::error(
                "WGPU-ADAPTER-NOT-FOUND",
                Category::Backend,
                "WGPU adapter request returned no compatible adapter",
                "",
            )
        })?;
        let info = adapter.get_info();
        let adapter_metadata = AdapterMetadata {
            adapter_name: info.name,
            device_type: format!("{:?}", info.device_type).to_lowercase(),
            graphics_backend: format!("{:?}", info.backend).to_lowercase(),
            driver_name: info.driver,
            driver_info: info.driver_info,
            vendor_id: info.vendor,
            device_id: info.device,
        };
        let limits = adapter.limits();
        if plan.canvas.width > limits.max_texture_dimension_2d
            || plan.canvas.height > limits.max_texture_dimension_2d
        {
            return Err(Diagnostic::error(
                "WGPU-OUTPUT-DIMENSIONS",
                Category::Backend,
                format!(
                    "output {}x{} exceeds adapter maximum 2D texture dimension {}",
                    plan.canvas.width, plan.canvas.height, limits.max_texture_dimension_2d
                ),
                "",
            ));
        }
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("video-editor headless renderer"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|error| diagnostic("WGPU-DEVICE-REQUEST", "device_request", error))?;
        let row_bytes = plan.canvas.width.checked_mul(4).ok_or_else(|| {
            Diagnostic::error(
                "WGPU-READBACK-SIZE",
                Category::Backend,
                "output row size overflow",
                "",
            )
        })?;
        let padded_row_bytes = align_up(row_bytes, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback_size = u64::from(padded_row_bytes)
            .checked_mul(u64::from(plan.canvas.height))
            .ok_or_else(|| {
                Diagnostic::error(
                    "WGPU-READBACK-SIZE",
                    Category::Backend,
                    "readback buffer size overflow",
                    "",
                )
            })?;
        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("video-editor output"),
            size: wgpu::Extent3d {
                width: plan.canvas.width,
                height: plan.canvas.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("video-editor readback"),
            size: readback_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let upload_started = Instant::now();
        let mut source_textures = Vec::with_capacity(plan.images.len());
        let mut uploaded_texture_bytes = 0_u64;
        for asset in 0..plan.images.len() {
            let image = decoded.image(asset);
            if image.width() > limits.max_texture_dimension_2d
                || image.height() > limits.max_texture_dimension_2d
            {
                return Err(Diagnostic::error(
                    "WGPU-SOURCE-DIMENSIONS",
                    Category::Backend,
                    format!("source image {} exceeds adapter texture dimensions", asset),
                    "",
                ));
            }
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("video-editor source"),
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
            uploaded_texture_bytes = uploaded_texture_bytes
                .saturating_add(u64::from(image.width()) * u64::from(image.height()) * 4);
            source_textures.push(texture);
        }
        let mut stats = decoded.stats().clone();
        stats.uploaded_texture_count = source_textures.len();
        stats.uploaded_texture_bytes = uploaded_texture_bytes;
        stats.readback_buffer_count = 1;
        stats.readback_buffer_bytes = readback_size;
        let mut timings = decoded.timings();
        timings.texture_upload = upload_started.elapsed();
        timings.gpu_initialization = started.elapsed();
        Ok(Self {
            _instance: instance,
            _adapter: adapter,
            device,
            queue,
            _source_textures: source_textures,
            output,
            readback,
            row_bytes,
            padded_row_bytes,
            frame_bytes: vec![0; (u64::from(row_bytes) * u64::from(plan.canvas.height)) as usize],
            cpu_reference: PreparedAssets::from_decoded(plan, decoded),
            stats,
            timings,
            adapter: adapter_metadata,
        })
    }
}

impl RenderBackend for WgpuBackend {
    fn kind(&self) -> RenderBackendKind {
        RenderBackendKind::Wgpu
    }

    fn prepare(
        &mut self,
        _plan: &RenderPlan,
        _decoded: Arc<DecodedAssets>,
    ) -> Result<(), Diagnostic> {
        Ok(())
    }

    fn render_frame(
        &mut self,
        frame: &EvaluatedFrame,
        destination: &mut RgbaImage,
    ) -> Result<(), Diagnostic> {
        // The source resources are uploaded exactly once above. The next change
        // replaces this reference compositor with the image/solid GPU passes;
        // the GPU output/readback path below is already the production frame
        // transport and preserves exact contiguous RGBA output semantics.
        compositor::compose(frame, &mut self.cpu_reference, destination);
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.output,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            destination.as_raw(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(self.row_bytes),
                rows_per_image: Some(frame.height),
            },
            wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
                depth_or_array_layers: 1,
            },
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("video-editor readback copy"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.output,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &self.readback,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_row_bytes),
                    rows_per_image: Some(frame.height),
                },
            },
            wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));
        let slice = self.readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .recv()
            .map_err(|error| {
                Diagnostic::error(
                    "WGPU-READBACK",
                    Category::Backend,
                    format!("readback callback failed: {error}"),
                    "",
                )
            })?
            .map_err(|error| diagnostic("WGPU-READBACK", "buffer_map", error))?;
        let mapped = slice.get_mapped_range();
        for (row, target) in self
            .frame_bytes
            .chunks_exact_mut(self.row_bytes as usize)
            .enumerate()
        {
            let start = row * self.padded_row_bytes as usize;
            target.copy_from_slice(&mapped[start..start + self.row_bytes as usize]);
        }
        drop(mapped);
        self.readback.unmap();
        destination.as_mut().copy_from_slice(&self.frame_bytes);
        Ok(())
    }

    fn stats(&mut self) -> PreparationStats {
        let cpu = self.cpu_reference.stats().clone();
        self.stats.bitmap_cache_hits = cpu.bitmap_cache_hits;
        self.stats.bitmap_cache_misses = cpu.bitmap_cache_misses;
        self.stats.bitmap_cache_requests = cpu.bitmap_cache_requests;
        self.stats.bitmap_cache_insertions = cpu.bitmap_cache_insertions;
        self.stats.bitmap_cache_hit_rate = cpu.bitmap_cache_hit_rate;
        self.stats.cache_current_entries = cpu.cache_current_entries;
        self.stats.peak_cache_entries = cpu.peak_cache_entries;
        self.stats.cache_current_bytes = cpu.cache_current_bytes;
        self.stats.cache_peak_bytes = cpu.cache_peak_bytes;
        self.stats.cache_evictions = cpu.cache_evictions;
        self.stats.clone()
    }

    fn timings(&self) -> PreparationTimings {
        self.timings
    }
    fn adapter(&self) -> Option<AdapterMetadata> {
        Some(self.adapter.clone())
    }
}

fn align_up(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}

fn diagnostic(code: &str, stage: &str, error: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::error(
        code,
        Category::Backend,
        format!("WGPU {stage} failed: {error}"),
        "",
    )
}
