//! Adapter-independent shader parsing checks.

#[test]
fn texture_shaders_parse_without_a_gpu_adapter() {
    naga::front::wgsl::parse_str(include_str!("../../shaders/layer.wgsl"))
        .expect("layer WGSL must parse independently of adapter availability");
    naga::front::wgsl::parse_str(include_str!("../../shaders/composite_normal.wgsl"))
        .expect("composite WGSL must parse independently of adapter availability");
    naga::front::wgsl::parse_str(include_str!("../../shaders/effects.wgsl"))
        .expect("effect WGSL must parse independently of adapter availability");
}
