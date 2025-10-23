//! Image comparison tool for testing and debugging
//!
//! Compares two PNG images pixel-by-pixel and reports differences.
//! Useful for detecting visual regressions in rendering.

use anyhow::Result;
use clap::Parser;
use image::{ImageBuffer, Rgba};

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

    // Compare pixels
    let mut total_diff = 0u64;
    let mut max_diff = 0u32;
    let mut different_pixels = 0u64;
    let mut diff_image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);

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

    // Report results
    println!("Results:");
    println!("  Total pixel difference: {}", total_diff);
    println!("  Average difference per pixel: {:.2}", avg_diff);
    println!("  Maximum pixel difference: {}", max_diff);
    println!("  Different pixels (threshold {}): {} ({:.2}%)",
        args.threshold, different_pixels, percent_different);
    println!();

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
