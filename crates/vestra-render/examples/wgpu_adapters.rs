fn main() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..wgpu::InstanceDescriptor::default()
    });
    let adapters = instance.enumerate_adapters(wgpu::Backends::all());
    if adapters.is_empty() {
        println!("no WGPU adapters discovered");
        return;
    }
    for adapter in adapters {
        let info = adapter.get_info();
        println!(
            "backend={} adapter={:?} device_type={:?} driver={:?} driver_info={:?} vendor={} device={}",
            info.backend,
            info.name,
            info.device_type,
            info.driver,
            info.driver_info,
            info.vendor,
            info.device,
        );
    }
}
