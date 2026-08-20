//! Adapter-independent shader parsing checks.

#[test]
fn texture_shaders_parse_without_a_gpu_adapter() {
    naga::front::wgsl::parse_str(include_str!("../../shaders/layer.wgsl"))
        .expect("layer WGSL must parse independently of adapter availability");
    naga::front::wgsl::parse_str(include_str!("../../shaders/composite_normal.wgsl"))
        .expect("composite WGSL must parse independently of adapter availability");
    naga::front::wgsl::parse_str(include_str!("../../shaders/spectrum2d.wgsl"))
        .expect("Spectrum2D WGSL must parse independently of adapter availability");
    naga::front::wgsl::parse_str(include_str!("../../shaders/particles.wgsl"))
        .expect("particle WGSL must parse independently of adapter availability");
    naga::front::wgsl::parse_str(include_str!("../../shaders/particle_resolve.wgsl"))
        .expect("particle resolve WGSL must parse independently of adapter availability");
    naga::front::wgsl::parse_str(include_str!("../../shaders/mask_raster.wgsl"))
        .expect("mask raster WGSL must parse independently of adapter availability");
    naga::front::wgsl::parse_str(include_str!("../../shaders/mask.wgsl"))
        .expect("mask WGSL must parse independently of adapter availability");
    naga::front::wgsl::parse_str(include_str!("../../shaders/mask_coverage.wgsl"))
        .expect("mask coverage WGSL must parse independently of adapter availability");
    naga::front::wgsl::parse_str(include_str!("../../shaders/mask_feather.wgsl"))
        .expect("mask feather WGSL must parse independently of adapter availability");
    for source in crate::kernel::EffectKernel::ALL {
        naga::front::wgsl::parse_str(source.source())
            .expect("effect kernel WGSL must parse independently of adapter availability");
    }
}
