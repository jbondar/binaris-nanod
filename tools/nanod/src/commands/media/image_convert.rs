use anyhow::{Context, Result};

/// Target size for album art (fills the 240x240 circular display).
pub const ART_SIZE: u32 = 240;

/// Download image from URL and convert to 240x240 RGB565 bytes.
pub fn fetch_and_convert_artwork(url: &str) -> Result<Vec<u8>> {
    // Fetch the image via curl (simple, no extra HTTP deps needed)
    let output = std::process::Command::new("curl")
        .args(["-sL", "--max-time", "5", url])
        .output()
        .context("failed to run curl")?;

    if !output.status.success() || output.stdout.is_empty() {
        anyhow::bail!("failed to download artwork from {}", url);
    }

    convert_to_rgb565(&output.stdout, ART_SIZE)
}

/// Generate a colorful test gradient pattern (240x240 RGB565).
pub fn generate_test_pattern() -> Vec<u8> {
    let size = ART_SIZE as usize;
    let mut rgb565 = Vec::with_capacity(size * size * 2);

    for y in 0..size {
        for x in 0..size {
            // Gradient: red increases left-to-right, blue increases top-to-bottom
            let r = (x * 255 / size) as u16;
            let g = 80u16; // subtle green
            let b = (y * 255 / size) as u16;
            let val = ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3);
            rgb565.push(val as u8);
            rgb565.push((val >> 8) as u8);
        }
    }
    rgb565
}

/// Convert raw image bytes (PNG/JPEG) to RGB565 at target_size x target_size.
/// Returns little-endian RGB565 bytes (matching ESP32 native byte order).
pub fn convert_to_rgb565(image_data: &[u8], target_size: u32) -> Result<Vec<u8>> {
    let img = image::load_from_memory(image_data)
        .context("failed to decode image")?;

    let resized = img.resize_exact(
        target_size,
        target_size,
        image::imageops::FilterType::Lanczos3,
    );
    let rgb = resized.to_rgb8();

    let pixel_count = (target_size * target_size) as usize;
    let mut rgb565 = Vec::with_capacity(pixel_count * 2);

    for pixel in rgb.pixels() {
        let r = pixel[0] as u16;
        let g = pixel[1] as u16;
        let b = pixel[2] as u16;
        let val = ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3);
        // Little-endian: low byte first (ESP32 native order)
        rgb565.push(val as u8);
        rgb565.push((val >> 8) as u8);
    }

    Ok(rgb565)
}
