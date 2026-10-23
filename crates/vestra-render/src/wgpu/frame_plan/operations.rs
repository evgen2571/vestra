//! Logical operations and fixed working-texture roles.

use crate::{kernel::EffectKernel, render::effects::EffectPass};

/// Fixed full-frame working texture roles. Effect slots are allocated when the
/// compiled plan contains a non-transform visual effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(in crate::wgpu) enum TextureSlot {
    CanvasA,
    CanvasB,
    Layer,
    ParticleAccumulation,
    EffectA,
    EffectB,
    Auxiliary,
    MaskCoverage,
    MaskFeather,
    GroupCanvasA(usize),
    GroupCanvasB(usize),
}

impl TextureSlot {
    pub(in crate::wgpu) fn is_group_canvas(self) -> bool {
        matches!(self, Self::GroupCanvasA(_) | Self::GroupCanvasB(_))
    }

    pub(super) fn is_canvas(self) -> bool {
        matches!(
            self,
            Self::CanvasA | Self::CanvasB | Self::GroupCanvasA(_) | Self::GroupCanvasB(_)
        )
    }
}

/// Whether a pass belongs to a rendered layer or to the final canvas.  Keeping
/// this in the plan gives execution failures actionable context without making
/// the pass itself WGPU-specific.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::wgpu) enum EffectScope {
    Layer,
    Global,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::wgpu) enum GpuOperation {
    ClearCanvas {
        destination: TextureSlot,
        parameters_index: u32,
    },
    RenderRasterLayer {
        layer_index: usize,
        source_index: usize,
        destination: TextureSlot,
        parameters_index: u32,
    },
    RenderMask {
        layer_index: usize,
        mask_index: usize,
        source_index: usize,
        source_layer: bool,
        state_source: TextureSlot,
        expected_state_value: Option<u64>,
        parameters_index: u32,
    },
    UpdateMaskCoverage {
        layer_index: usize,
        mask_index: usize,
        source: TextureSlot,
        expected_source_value: u64,
        destination: TextureSlot,
        result_value: u64,
        parameters_index: u32,
    },
    FeatherMask {
        layer_index: usize,
        mask_index: usize,
        source: TextureSlot,
        expected_source_value: u64,
        destination: TextureSlot,
        result_value: u64,
        parameters_index: u32,
        horizontal: bool,
    },
    RenderSurfaceLayer {
        layer_index: usize,
        source: TextureSlot,
        destination: TextureSlot,
        parameters_index: u32,
    },
    RenderSolidLayer {
        layer_index: usize,
        destination: TextureSlot,
        parameters_index: u32,
    },
    RenderSpectrum2DLayer {
        layer_index: usize,
        destination: TextureSlot,
        parameters_index: u32,
    },
    RenderParticleLayer {
        layer_index: usize,
        destination: TextureSlot,
        parameters_index: u32,
        instance_offset: u32,
        instance_count: u32,
        blend_mode: crate::project::ParticleBlendMode,
    },
    ResolveParticleLayer {
        source: TextureSlot,
        destination: TextureSlot,
    },
    /// Retains an explicitly requested original effect input.
    /// This is a texture-to-texture copy in the frame's single encoder, not a
    /// new working-texture allocation.
    CopyForEffect {
        source: TextureSlot,
        destination: TextureSlot,
        value: u64,
    },
    /// An executable effect pass. The operation owns the logical pass so the
    /// executor never has to rediscover semantics from an evaluated layer.
    ApplyEffect {
        kernel: EffectKernel,
        scope: EffectScope,
        layer_index: Option<usize>,
        effect_index: usize,
        pass_index: usize,
        pass: EffectPass,
        source: TextureSlot,
        expected_source_value: u64,
        destination: TextureSlot,
        result_value: u64,
        auxiliary: Option<TextureSlot>,
        auxiliary_value: Option<u64>,
        parameters_index: u32,
    },
    ApplyMask {
        layer_index: usize,
        mask_index: usize,
        source: TextureSlot,
        expected_source_value: u64,
        destination: TextureSlot,
        result_value: u64,
        parameters_index: u32,
    },
    CompositeLayer {
        layer_index: usize,
        layer_source: TextureSlot,
        expected_layer_value: u64,
        canvas_source: TextureSlot,
        expected_canvas_value: u64,
        canvas_destination: TextureSlot,
        result_value: u64,
        parameters_index: u32,
    },
    StoreStaticLayer {
        cache_key: usize,
        source: TextureSlot,
        expected_source_value: u64,
    },
    CompositeCachedLayer {
        cache_key: usize,
        layer_index: usize,
        canvas_source: TextureSlot,
        expected_canvas_value: u64,
        canvas_destination: TextureSlot,
        result_value: u64,
        parameters_index: u32,
    },
    CopyForReadback {
        source: TextureSlot,
        expected_value: u64,
    },
}

pub(in crate::wgpu) fn alternate_canvas_for_bindings(current: TextureSlot) -> Option<TextureSlot> {
    match current {
        TextureSlot::CanvasA => Some(TextureSlot::CanvasB),
        TextureSlot::CanvasB => Some(TextureSlot::CanvasA),
        TextureSlot::GroupCanvasA(depth) => Some(TextureSlot::GroupCanvasB(depth)),
        TextureSlot::GroupCanvasB(depth) => Some(TextureSlot::GroupCanvasA(depth)),
        _ => None,
    }
}
