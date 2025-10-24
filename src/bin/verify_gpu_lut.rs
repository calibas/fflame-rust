/// Verify GPU texture upload by reading back the LUT texture
/// This confirms the data reaches the GPU intact
use fractal_flame_wgpu::scene::tonemap::ToneCurve;
use half::f16;
use wgpu::*;

fn main() {
    println!("=== GPU LUT Upload Verification ===\n");

    // Create wgpu instance and device
    let instance = Instance::new(InstanceDescriptor {
        backends: Backends::all(),
        ..Default::default()
    });

    let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .expect("Failed to find adapter");

    println!("Using GPU: {}", adapter.get_info().name);
    println!("Backend: {:?}\n", adapter.get_info().backend);

    let (device, queue) = pollster::block_on(adapter.request_device(
        &DeviceDescriptor {
            label: Some("Debug Device"),
            required_features: Features::empty(),
            required_limits: Limits::default(),
            memory_hints: Default::default(),
        },
        None,
    ))
    .expect("Failed to create device");

    // Generate CPU-side LUT
    let curve = ToneCurve::linear();
    let cpu_lut_bytes = curve.generate_lut();
    println!("CPU LUT generated: {} bytes", cpu_lut_bytes.len());

    // Create GPU texture
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("Debug Curve LUT"),
        size: Extent3d {
            width: 256,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D1,
        format: TextureFormat::Rgba16Float,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC | TextureUsages::COPY_DST,
        view_formats: &[],
    });

    // Upload to GPU
    queue.write_texture(
        ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        &cpu_lut_bytes,
        ImageDataLayout {
            offset: 0,
            bytes_per_row: None,
            rows_per_image: None,
        },
        Extent3d {
            width: 256,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    println!("✓ Uploaded to GPU texture\n");

    // Create staging buffer for readback
    let buffer_size = cpu_lut_bytes.len() as u64;
    let staging_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Staging Buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Copy texture to buffer
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Copy Encoder"),
    });

    encoder.copy_texture_to_buffer(
        ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        ImageCopyBuffer {
            buffer: &staging_buffer,
            layout: ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(256 * 8), // 256 pixels * 8 bytes (4 * f16)
                rows_per_image: None,
            },
        },
        Extent3d {
            width: 256,
            height: 1,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(Some(encoder.finish()));
    println!("✓ Copied texture to staging buffer");

    // Read back data
    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(MapMode::Read, move |result| {
        tx.send(result).unwrap();
    });

    device.poll(Maintain::Wait);
    rx.recv().unwrap().expect("Failed to map buffer");

    let gpu_data = buffer_slice.get_mapped_range();
    let gpu_lut_bytes: Vec<u8> = gpu_data.to_vec();
    drop(gpu_data);
    staging_buffer.unmap();

    println!("✓ Read back {} bytes from GPU\n", gpu_lut_bytes.len());

    // Compare CPU vs GPU
    println!("=== Byte-for-Byte Comparison ===\n");

    let mut total_diffs = 0;
    let mut first_diffs = Vec::new();

    for i in 0..cpu_lut_bytes.len() {
        if cpu_lut_bytes[i] != gpu_lut_bytes[i] {
            total_diffs += 1;
            if first_diffs.len() < 10 {
                first_diffs.push((i, cpu_lut_bytes[i], gpu_lut_bytes[i]));
            }
        }
    }

    if total_diffs == 0 {
        println!("✅ PERFECT MATCH - All {} bytes identical!", cpu_lut_bytes.len());
        println!("GPU texture upload is working correctly.");
        println!("\nConclusion: The bug is NOT in texture upload.");
        println!("The bug must be in how the shader samples the texture.");
    } else {
        println!("❌ MISMATCH - {} bytes differ", total_diffs);
        println!("\nFirst 10 differences:");
        for (idx, cpu_byte, gpu_byte) in first_diffs {
            let pixel_idx = idx / 8;
            let component = (idx % 8) / 2;
            let component_name = ["R", "G", "B", "A"][component];
            println!("  Byte {}: pixel={}, component={}, CPU={:02x}, GPU={:02x}",
                     idx, pixel_idx, component_name, cpu_byte, gpu_byte);
        }
        println!("\nConclusion: Texture upload is corrupting data!");
    }

    // Compare f16 values
    println!("\n=== Value Comparison (R channel) ===\n");
    println!("{:>5} {:>12} {:>12} {:>12}", "Index", "CPU Value", "GPU Value", "Difference");
    println!("{}", "-".repeat(50));

    let mut max_diff = 0.0f32;
    let mut max_diff_idx = 0;

    for i in [0, 1, 63, 64, 127, 128, 191, 192, 254, 255] {
        let cpu_offset = i * 8;
        let gpu_offset = i * 8;

        let cpu_r = f16::from_le_bytes([cpu_lut_bytes[cpu_offset], cpu_lut_bytes[cpu_offset + 1]]).to_f32();
        let gpu_r = f16::from_le_bytes([gpu_lut_bytes[gpu_offset], gpu_lut_bytes[gpu_offset + 1]]).to_f32();

        let diff = (cpu_r - gpu_r).abs();
        if diff > max_diff {
            max_diff = diff;
            max_diff_idx = i;
        }

        println!("{:5} {:12.8} {:12.8} {:12.8}", i, cpu_r, gpu_r, diff);
    }

    println!("\nMax difference: {:.8} at index {}", max_diff, max_diff_idx);

    println!("\n=== Verification Complete ===");
}
