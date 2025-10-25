//! Image comparison tool for testing and debugging
//!
//! Compares two PNG images pixel-by-pixel and reports differences.
//! Useful for detecting visual regressions in rendering.

use anyhow::Result;
use clap::Parser;
use image::{ImageBuffer, Rgba};
use fractal_flame_wgpu::png_metadata::read_png_metadata;

#[derive(Parser, Debug)]
#[command(name = "compare_images")]
#[command(about = "Compare two PNG images and report differences", long_about = None)]
struct Args {
    /// First image path
    #[arg(short = '1', long)]
    image1: String,

    /// Second image path
    #[arg(short = '2', long)]
    image2: String,

    /// Output difference image (optional)
    #[arg(short, long)]
    output: Option<String>,

    /// Threshold for considering pixels different (0-255)
    #[arg(short, long, default_value = "0")]
    threshold: u8,

    /// Amplify differences in output image
    #[arg(short, long, default_value = "10")]
    amplify: u8,

    /// Enable advanced metrics (SSIM, MSE, PSNR)
    #[arg(long)]
    advanced: bool,

    /// Minimum average color intensity (0-255) to consider valid
    #[arg(long, default_value = "10")]
    min_color: u8,

    /// Skip color validation check
    #[arg(long)]
    skip_color_check: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("Image Comparison Tool");
    println!("====================");
    println!("Image 1: {}", args.image1);
    println!("Image 2: {}", args.image2);
    println!("Threshold: {}", args.threshold);
    println!();

    // Load images
    let img1 = image::open(&args.image1)?
        .to_rgba8();
    let img2 = image::open(&args.image2)?
        .to_rgba8();

    // Check dimensions
    if img1.dimensions() != img2.dimensions() {
        anyhow::bail!(
            "Images have different dimensions: {}x{} vs {}x{}",
            img1.width(),
            img1.height(),
            img2.width(),
            img2.height()
        );
    }

    let (width, height) = img1.dimensions();
    println!("Dimensions: {}x{} ({} pixels)", width, height, width * height);
    println!();

    // Read PNG metadata if available
    let metadata1 = std::fs::read(&args.image1)
        .ok()
        .and_then(|data| read_png_metadata(&data).ok());
    let metadata2 = std::fs::read(&args.image2)
        .ok()
        .and_then(|data| read_png_metadata(&data).ok());

    // Display metadata comparison
    if let (Some(ref m1), Some(ref m2)) = (&metadata1, &metadata2) {
        println!("Metadata Comparison:");
        println!("  Image 1: v{} ({})", m1.version, m1.git_hash);
        println!("  Image 2: v{} ({})", m2.version, m2.git_hash);

        if m1.git_hash != m2.git_hash {
            println!("  ⚠ Different git commits!");
        }

        println!("  Iterations: {} vs {}", m1.total_iterations, m2.total_iterations);
        println!("  Render time: {:.2}ms vs {:.2}ms", m1.render_time_ms, m2.render_time_ms);
        println!("  Iterations/thread: {} vs {}", m1.iterations_per_thread, m2.iterations_per_thread);
        println!("  Speed factor: {:.2}x vs {:.2}x", m1.speed_factor, m2.speed_factor);

        if m1.shader_compile_time_ms.is_some() || m2.shader_compile_time_ms.is_some() {
            println!("  Shader compile time: {} vs {}",
                m1.shader_compile_time_ms.map(|t| format!("{:.2}ms", t)).unwrap_or("N/A".to_string()),
                m2.shader_compile_time_ms.map(|t| format!("{:.2}ms", t)).unwrap_or("N/A".to_string())
            );
        }

        println!();
    } else if metadata1.is_some() || metadata2.is_some() {
        println!("⚠ Warning: Only one image has metadata\n");
    }

    // Color validation
    if !args.skip_color_check {
        let color1 = calculate_average_color(&img1);
        let color2 = calculate_average_color(&img2);

        println!("Color Validation:");
        println!("  Image 1 avg intensity: R={:.1} G={:.1} B={:.1}", color1.0, color1.1, color1.2);
        println!("  Image 2 avg intensity: R={:.1} G={:.1} B={:.1}", color2.0, color2.1, color2.2);

        let avg1 = (color1.0 + color1.1 + color1.2) / 3.0;
        let avg2 = (color2.0 + color2.1 + color2.2) / 3.0;

        if avg1 < args.min_color as f64 {
            println!("  ✗ Image 1 appears mostly black (avg: {:.1} < {})", avg1, args.min_color);
            println!("     This may indicate a bad reference image!");
            anyhow::bail!("Image 1 failed color validation - appears mostly black");
        }
        if avg2 < args.min_color as f64 {
            println!("  ✗ Image 2 appears mostly black (avg: {:.1} < {})", avg2, args.min_color);
            println!("     This may indicate a bad reference image!");
            anyhow::bail!("Image 2 failed color validation - appears mostly black");
        }

        println!("  ✓ Both images have sufficient color data\n");
    }

    // Compare pixels
    let mut total_diff = 0u64;
    let mut max_diff = 0u32;
    let mut different_pixels = 0u64;
    let mut diff_image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);

    // Per-channel statistics
    let mut total_diff_r = 0u64;
    let mut total_diff_g = 0u64;
    let mut total_diff_b = 0u64;
    let mut total_diff_a = 0u64;
    let mut max_diff_r = 0u32;
    let mut max_diff_g = 0u32;
    let mut max_diff_b = 0u32;
    let mut max_diff_a = 0u32;

    for y in 0..height {
        for x in 0..width {
            let p1 = img1.get_pixel(x, y);
            let p2 = img2.get_pixel(x, y);

            let diff_r = (p1[0] as i32 - p2[0] as i32).abs() as u32;
            let diff_g = (p1[1] as i32 - p2[1] as i32).abs() as u32;
            let diff_b = (p1[2] as i32 - p2[2] as i32).abs() as u32;
            let diff_a = (p1[3] as i32 - p2[3] as i32).abs() as u32;

            let pixel_diff = diff_r + diff_g + diff_b + diff_a;
            total_diff += pixel_diff as u64;
            max_diff = max_diff.max(pixel_diff);

            // Per-channel stats
            total_diff_r += diff_r as u64;
            total_diff_g += diff_g as u64;
            total_diff_b += diff_b as u64;
            total_diff_a += diff_a as u64;
            max_diff_r = max_diff_r.max(diff_r);
            max_diff_g = max_diff_g.max(diff_g);
            max_diff_b = max_diff_b.max(diff_b);
            max_diff_a = max_diff_a.max(diff_a);

            if pixel_diff > args.threshold as u32 {
                different_pixels += 1;
            }

            // Create difference image (amplified for visibility)
            let amplified_r = (diff_r * args.amplify as u32).min(255) as u8;
            let amplified_g = (diff_g * args.amplify as u32).min(255) as u8;
            let amplified_b = (diff_b * args.amplify as u32).min(255) as u8;
            let amplified_a = (diff_a * args.amplify as u32).min(255) as u8;

            diff_image.put_pixel(x, y, Rgba([amplified_r, amplified_g, amplified_b, amplified_a]));
        }
    }

    // Calculate statistics
    let total_pixels = (width * height) as u64;
    let avg_diff = total_diff as f64 / total_pixels as f64;
    let percent_different = (different_pixels as f64 / total_pixels as f64) * 100.0;

    // Per-channel averages
    let avg_diff_r = total_diff_r as f64 / total_pixels as f64;
    let avg_diff_g = total_diff_g as f64 / total_pixels as f64;
    let avg_diff_b = total_diff_b as f64 / total_pixels as f64;
    let avg_diff_a = total_diff_a as f64 / total_pixels as f64;

    // Report results
    println!("Results:");
    println!("  Total pixel difference: {}", total_diff);
    println!("  Average difference per pixel: {:.2}", avg_diff);
    println!("  Maximum pixel difference: {}", max_diff);
    println!("  Different pixels (threshold {}): {} ({:.2}%)",
        args.threshold, different_pixels, percent_different);
    println!();
    println!("Per-Channel Statistics:");
    println!("  Red   - Avg: {:.2} ({:.2}%), Max: {} ({:.2}%)",
        avg_diff_r, (avg_diff_r / 255.0) * 100.0, max_diff_r, (max_diff_r as f64 / 255.0) * 100.0);
    println!("  Green - Avg: {:.2} ({:.2}%), Max: {} ({:.2}%)",
        avg_diff_g, (avg_diff_g / 255.0) * 100.0, max_diff_g, (max_diff_g as f64 / 255.0) * 100.0);
    println!("  Blue  - Avg: {:.2} ({:.2}%), Max: {} ({:.2}%)",
        avg_diff_b, (avg_diff_b / 255.0) * 100.0, max_diff_b, (max_diff_b as f64 / 255.0) * 100.0);
    println!("  Alpha - Avg: {:.2} ({:.2}%), Max: {} ({:.2}%)",
        avg_diff_a, (avg_diff_a / 255.0) * 100.0, max_diff_a, (max_diff_a as f64 / 255.0) * 100.0);
    println!();

    // Advanced metrics if requested
    if args.advanced {
        println!("Advanced Metrics:");

        // MSE (Mean Squared Error)
        let mse = calculate_mse(&img1, &img2);
        println!("  MSE: {:.4}", mse);

        // PSNR (Peak Signal-to-Noise Ratio)
        let psnr = if mse > 0.0 {
            20.0 * (255.0_f64).log10() - 10.0 * mse.log10()
        } else {
            f64::INFINITY
        };
        println!("  PSNR: {:.2} dB{}", psnr, if psnr.is_infinite() { " (identical)" } else { "" });

        // SSIM (Structural Similarity Index)
        let ssim = calculate_ssim(&img1, &img2);
        println!("  SSIM: {:.4} (1.0 = identical)", ssim);

        println!();
        println!("Metric Interpretation:");
        println!("  MSE:  Lower is better (0 = identical)");
        println!("  PSNR: Higher is better (>40 dB = excellent, 30-40 dB = good)");
        println!("  SSIM: Higher is better (>0.99 = excellent, >0.95 = good)");
        println!();
    }

    if different_pixels == 0 {
        println!("✓ Images are identical!");
    } else if percent_different < 0.01 {
        println!("⚠ Images are nearly identical (< 0.01% different)");
    } else if percent_different < 1.0 {
        println!("⚠ Images have minor differences (< 1% different)");
    } else {
        println!("✗ Images are significantly different ({:.2}% different)", percent_different);
    }

    // Save difference image if requested
    if let Some(output_path) = args.output {
        diff_image.save(&output_path)?;
        println!();
        println!("Difference image saved to: {}", output_path);
        println!("  (Differences amplified by {}x for visibility)", args.amplify);
    }

    // Exit with error code if images differ significantly
    if different_pixels > 0 {
        std::process::exit(1);
    }

    Ok(())
}

/// Calculate Mean Squared Error
fn calculate_mse(img1: &ImageBuffer<Rgba<u8>, Vec<u8>>, img2: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> f64 {
    let (width, height) = img1.dimensions();
    let mut sum_squared_error = 0.0;
    let total_channels = (width * height * 4) as f64; // RGBA = 4 channels

    for y in 0..height {
        for x in 0..width {
            let p1 = img1.get_pixel(x, y);
            let p2 = img2.get_pixel(x, y);

            for i in 0..4 {
                let diff = p1[i] as f64 - p2[i] as f64;
                sum_squared_error += diff * diff;
            }
        }
    }

    sum_squared_error / total_channels
}

/// Calculate Structural Similarity Index (SSIM)
/// Simplified implementation using luminance comparison
fn calculate_ssim(img1: &ImageBuffer<Rgba<u8>, Vec<u8>>, img2: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> f64 {
    let (width, height) = img1.dimensions();

    // Constants for SSIM calculation
    let c1 = (0.01 * 255.0_f64).powi(2);
    let c2 = (0.03 * 255.0_f64).powi(2);

    // Use 8x8 windows for SSIM calculation
    let window_size = 8;
    let mut ssim_sum = 0.0;
    let mut window_count = 0;

    for y in (0..height).step_by(window_size) {
        for x in (0..width).step_by(window_size) {
            // Calculate window bounds
            let x_end = (x + window_size as u32).min(width);
            let y_end = (y + window_size as u32).min(height);

            if x_end - x < window_size as u32 || y_end - y < window_size as u32 {
                continue; // Skip partial windows at edges
            }

            // Calculate mean and variance for this window
            let mut mean1 = 0.0;
            let mut mean2 = 0.0;
            let pixels_in_window = (window_size * window_size) as f64;

            // Calculate means
            for wy in y..y_end {
                for wx in x..x_end {
                    let p1 = img1.get_pixel(wx, wy);
                    let p2 = img2.get_pixel(wx, wy);

                    // Convert to luminance (RGB only, ignore alpha)
                    let lum1 = 0.299 * p1[0] as f64 + 0.587 * p1[1] as f64 + 0.114 * p1[2] as f64;
                    let lum2 = 0.299 * p2[0] as f64 + 0.587 * p2[1] as f64 + 0.114 * p2[2] as f64;

                    mean1 += lum1;
                    mean2 += lum2;
                }
            }
            mean1 /= pixels_in_window;
            mean2 /= pixels_in_window;

            // Calculate variances and covariance
            let mut var1 = 0.0;
            let mut var2 = 0.0;
            let mut covar = 0.0;

            for wy in y..y_end {
                for wx in x..x_end {
                    let p1 = img1.get_pixel(wx, wy);
                    let p2 = img2.get_pixel(wx, wy);

                    let lum1 = 0.299 * p1[0] as f64 + 0.587 * p1[1] as f64 + 0.114 * p1[2] as f64;
                    let lum2 = 0.299 * p2[0] as f64 + 0.587 * p2[1] as f64 + 0.114 * p2[2] as f64;

                    let diff1 = lum1 - mean1;
                    let diff2 = lum2 - mean2;

                    var1 += diff1 * diff1;
                    var2 += diff2 * diff2;
                    covar += diff1 * diff2;
                }
            }
            var1 /= pixels_in_window;
            var2 /= pixels_in_window;
            covar /= pixels_in_window;

            // Calculate SSIM for this window
            let numerator = (2.0 * mean1 * mean2 + c1) * (2.0 * covar + c2);
            let denominator = (mean1 * mean1 + mean2 * mean2 + c1) * (var1 + var2 + c2);
            let window_ssim = numerator / denominator;

            ssim_sum += window_ssim;
            window_count += 1;
        }
    }

    if window_count > 0 {
        ssim_sum / window_count as f64
    } else {
        1.0 // If no windows, assume identical
    }
}

/// Calculate average color intensity (R, G, B) across all pixels
fn calculate_average_color(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> (f64, f64, f64) {
    let (width, height) = img.dimensions();
    let mut sum_r = 0u64;
    let mut sum_g = 0u64;
    let mut sum_b = 0u64;

    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(x, y);
            sum_r += pixel[0] as u64;
            sum_g += pixel[1] as u64;
            sum_b += pixel[2] as u64;
        }
    }

    let total_pixels = (width * height) as f64;
    (
        sum_r as f64 / total_pixels,
        sum_g as f64 / total_pixels,
        sum_b as f64 / total_pixels,
    )
}
