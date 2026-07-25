//! Persistent source textures and frame buffers for one WGPU backend.

use crate::{Category, Diagnostic, plan::RenderPlan, render::DecodedAssets};

pub(super) struct FrameResources {
    pub(super) accumulation: wgpu::Buffer,
    pub(super) output: wgpu::Texture,
    pub(super) readback: wgpu::Buffer,
    pub(super) row_bytes: u32,
    pub(super) padded_row_bytes: u32,
    pub(super) frame_bytes: Vec<u8>,
}

impl FrameResources {
    pub(super) fn create(
        device: &wgpu::Device,
        plan: &RenderPlan,
        row_bytes: u32,
        padded_row_bytes: u32,
        buffer_size: u64,
    ) -> Self {
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
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let accumulation = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("video-editor layer accumulation"),
            size: buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        Self {
            accumulation,
            output,
            readback,
            row_bytes,
            padded_row_bytes,
            frame_bytes: vec![0; (u64::from(row_bytes) * u64::from(plan.canvas.height)) as usize],
        }
    }
}

pub(super) struct SourceResources {
    pub(super) _textures: Vec<wgpu::Texture>,
    pub(super) bind_groups: Vec<wgpu::BindGroup>,
    pub(super) _solid_texture: wgpu::Texture,
    pub(super) solid_bind_group: wgpu::BindGroup,
    pub(super) dimensions: Vec<(u32, u32)>,
    pub(super) uploaded_texture_bytes: u64,
}

impl SourceResources {
    #[expect(
        clippy::too_many_arguments,
        reason = "source upload needs the existing device, queue, plan, and persistent bindings without a second setup object"
    )]
    pub(super) fn create(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        plan: &RenderPlan,
        decoded: &DecodedAssets,
        bindings: &wgpu::BindGroupLayout,
        accumulation: &wgpu::Buffer,
        parameters: &wgpu::Buffer,
        max_texture_dimension_2d: u32,
    ) -> Result<Self, Diagnostic> {
        let mut textures = Vec::with_capacity(plan.images.len());
        let mut dimensions = Vec::with_capacity(plan.images.len());
        let mut uploaded_texture_bytes = 0_u64;
        for asset in 0..plan.images.len() {
            let image = decoded.image(asset);
            if image.width() > max_texture_dimension_2d || image.height() > max_texture_dimension_2d
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
            dimensions.push((image.width(), image.height()));
            textures.push(texture);
        }
        let bind_groups = textures
            .iter()
            .map(|texture| {
                create_layer_bind_group(
                    device,
                    bindings,
                    &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    accumulation,
                    parameters,
                )
            })
            .collect();
        let solid_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("video-editor solid source"),
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
        let solid_bind_group = create_layer_bind_group(
            device,
            bindings,
            &solid_texture.create_view(&wgpu::TextureViewDescriptor::default()),
            accumulation,
            parameters,
        );
        Ok(Self {
            _textures: textures,
            bind_groups,
            _solid_texture: solid_texture,
            solid_bind_group,
            dimensions,
            uploaded_texture_bytes,
        })
    }
}

fn create_layer_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: &wgpu::TextureView,
    accumulation: &wgpu::Buffer,
    parameters: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("video-editor source layer bindings"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(source),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: accumulation.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: parameters.as_entire_binding(),
            },
        ],
    })
}
