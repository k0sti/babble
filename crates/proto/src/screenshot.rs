//! Screenshot capture utilities for debug mode
//!
//! This module provides functionality to capture and save screenshots
//! when the application exits in debug mode with a frame limit.

use egui::{ColorImage, Context, Event, UserData, ViewportCommand};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tracing::{error, info};

/// Ensures the output directory exists
fn ensure_output_dir() -> std::io::Result<()> {
    let output_dir = Path::new("output");
    if !output_dir.exists() {
        fs::create_dir_all(output_dir)?;
    }
    Ok(())
}

/// Requests a screenshot capture for the next frame
///
/// The screenshot will be returned via `Event::Screenshot` in the next frame's input events.
pub fn request_screenshot(ctx: &Context, name: &str) {
    info!("[SCREENSHOT] Requesting screenshot: {}", name);
    ctx.send_viewport_cmd(ViewportCommand::Screenshot(UserData::new(name.to_string())));
}

/// Processes input events to handle any pending screenshot results
///
/// Returns true if a screenshot was successfully saved.
pub fn process_screenshot_events(ctx: &Context) -> bool {
    let mut screenshot_saved = false;

    ctx.input(|input| {
        for event in &input.events {
            if let Event::Screenshot {
                viewport_id: _,
                user_data,
                image,
            } = event
            {
                // Extract the name from user_data.data (Arc<dyn Any>)
                let name = user_data
                    .data
                    .as_ref()
                    .and_then(|arc| arc.downcast_ref::<String>());

                if let Some(name) = name {
                    if let Err(e) = save_screenshot(image, name) {
                        error!("[SCREENSHOT] Failed to save screenshot '{}': {}", name, e);
                    } else {
                        screenshot_saved = true;
                    }
                } else {
                    error!("[SCREENSHOT] Screenshot event missing name in user_data");
                }
            }
        }
    });

    screenshot_saved
}

/// Saves a ColorImage to a PNG file in the output directory
fn save_screenshot(image: &Arc<ColorImage>, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Ensure output directory exists
    ensure_output_dir()?;

    let path = format!("output/{}.png", name);
    info!(
        "[SCREENSHOT] Saving screenshot to {} ({}x{})",
        path,
        image.width(),
        image.height()
    );

    // Convert ColorImage to image crate format
    let width = image.width() as u32;
    let height = image.height() as u32;

    // ColorImage stores pixels as Color32 (RGBA), we need to convert to bytes
    let rgba_bytes: Vec<u8> = image
        .pixels
        .iter()
        .flat_map(|color| color.to_array())
        .collect();

    // Create image buffer and save as PNG
    let img_buffer: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
        image::ImageBuffer::from_raw(width, height, rgba_bytes)
            .ok_or("Failed to create image buffer")?;

    img_buffer.save(&path)?;

    info!("[SCREENSHOT] Screenshot saved successfully: {}", path);
    Ok(())
}
