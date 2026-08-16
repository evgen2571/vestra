fn main() {
    let adapters = vestra_render::discover_wgpu_adapters();
    if adapters.is_empty() {
        println!("no WGPU adapters discovered");
        return;
    }
    for adapter in adapters {
        println!(
            "backend={} adapter={:?} device_type={:?} classification={} driver={:?} driver_info={:?} vendor={} device={}",
            adapter.graphics_backend,
            adapter.adapter_name,
            adapter.device_type,
            adapter.performance_class().as_str(),
            adapter.driver_name,
            adapter.driver_info,
            adapter.vendor_id,
            adapter.device_id,
        );
    }
}
