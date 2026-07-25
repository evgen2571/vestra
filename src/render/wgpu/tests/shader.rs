//! Adapter-independent shader parsing checks.

#[test]
fn layer_shader_parses_without_a_gpu_adapter() {
    naga::front::wgsl::parse_str(include_str!("../../shaders/layer.wgsl"))
        .expect("layer WGSL must parse independently of adapter availability");
}
