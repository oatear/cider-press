use wasm_bindgen::prelude::*;
use lcms2::*;

/// Initialize panic hook for better browser console errors (debug builds).
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// A stateful proofer that caches the LittleCMS transform for performance.
#[wasm_bindgen]
pub struct SoftProofer {
    transform: Transform<[u8; 4], [u8; 4]>,
}

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct UnsharpMaskOptions {
    pub enabled: bool,
    pub radius: f32,
    pub amount: f32,
    pub threshold: f32,
}

#[derive(Serialize, Deserialize)]
pub struct PostProcessOptions {
    pub unsharp_mask: Option<UnsharpMaskOptions>,
}

#[wasm_bindgen]
impl SoftProofer {
    #[wasm_bindgen(constructor)]
    pub fn new(printer_profile_icc: &[u8], intent: u32, _use_16bit: bool) -> Result<SoftProofer, JsError> {
        let srgb = Profile::new_srgb();
        let printer = Profile::new_icc(printer_profile_icc)
            .map_err(|e| JsError::new(&format!("Invalid ICC profile: {:?}", e)))?;
        
        let intent = intent_from_u32(intent)?;

        // Create a proofing transform: sRGB -> Printer -> sRGB
        // This is the standard way to do soft proofing in LittleCMS.
        // We use RGBA_8 for both input and output to match the JS ImageData format.
        let transform = Transform::new_proofing(
            &srgb,
            PixelFormat::RGBA_8,
            &srgb,
            PixelFormat::RGBA_8,
            &printer,
            intent,
            Intent::AbsoluteColorimetric,
            Flags::SOFT_PROOFING | Flags::BLACKPOINT_COMPENSATION,
        ).map_err(|e| JsError::new(&format!("Failed to create proofing transform: {:?}", e)))?;

        Ok(SoftProofer {
            transform,
        })
    }

    /// Apply the soft-proof transform and optional post-processing.
    pub fn apply(&self, pixels: &[u8], width: u32, height: u32, options: JsValue) -> Result<Vec<u8>, JsError> {
        let options: PostProcessOptions = serde_wasm_bindgen::from_value(options)
            .map_err(|e| JsError::new(&format!("Invalid options: {:?}", e)))?;

        let expected_len = (width as usize) * (height as usize) * 4;
        if pixels.len() != expected_len {
            return Err(JsError::new(&format!(
                "Expected {} bytes ({}x{}x4), got {}",
                expected_len, width, height, pixels.len()
            )));
        }

        let mut output = vec![0u8; expected_len];

        // Apply the cached proofing transform
        self.transform.transform_pixels(
            bytemuck::cast_slice::<u8, [u8; 4]>(pixels),
            bytemuck::cast_slice_mut::<u8, [u8; 4]>(&mut output),
        );

        // Restore alpha channel (restore from source)
        for i in 0..(pixels.len() / 4) {
            output[i * 4 + 3] = pixels[i * 4 + 3];
        }

        // --- Post-Processing ---
        if let Some(unsharp) = options.unsharp_mask {
            if unsharp.enabled {
                output = self.apply_unsharp_mask(output, width, height, &unsharp)?;
            }
        }

        Ok(output)
    }

    fn apply_unsharp_mask(&self, pixels: Vec<u8>, width: u32, height: u32, options: &UnsharpMaskOptions) -> Result<Vec<u8>, JsError> {
        use image::{ImageBuffer, Rgba};
        
        // Convert to image buffer for easy Gaussian blur
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(width, height, pixels.clone())
            .ok_or_else(|| JsError::new("Failed to create image buffer"))?;

        // 1. Create blurred version
        // sigma corresponds to radius in many tools
        let blurred = image::imageops::blur(&img, options.radius);

        // 2. Perform Unsharp Mask: original + (original - blurred) * amount
        // Threshold is percentage (0-100)
        let threshold_value = (options.threshold / 100.0 * 255.0) as i16;

        for (x, y, pixel) in img.enumerate_pixels_mut() {
            let original = pixel.0;
            let blur_pixel = blurred.get_pixel(x, y).0;

            for i in 0..3 { // RGB only
                let diff = original[i] as i16 - blur_pixel[i] as i16;
                if diff.abs() > threshold_value {
                    let adjusted = original[i] as f32 + (diff as f32 * options.amount);
                    pixel.0[i] = adjusted.clamp(0.0, 255.0) as u8;
                }
            }
        }

        Ok(img.into_raw())
    }
}

/// Apply a soft-proofing transform to RGBA pixel data (stateless compatibility wrapper).
#[wasm_bindgen]
pub fn apply_soft_proof(
    pixels: &[u8],
    width: u32,
    height: u32,
    printer_profile_icc: &[u8],
    intent: u32,
) -> Result<Vec<u8>, JsError> {
    let proofer = SoftProofer::new(printer_profile_icc, intent, false)?;
    proofer.apply(pixels, width, height, JsValue::NULL)
}

/// Apply a soft-proofing transform using 16-bit intermediate precision (stateless compatibility wrapper).
#[wasm_bindgen]
pub fn apply_soft_proof_16bit(
    pixels: &[u8],
    width: u32,
    height: u32,
    printer_profile_icc: &[u8],
    intent: u32,
) -> Result<Vec<u8>, JsError> {
    let proofer = SoftProofer::new(printer_profile_icc, intent, true)?;
    proofer.apply(pixels, width, height, JsValue::NULL)
}

/// Simple profile-to-profile color transform (no proofing).
#[wasm_bindgen]
pub fn transform_pixels(
    pixels: &[u8],
    width: u32,
    height: u32,
    source_profile_icc: &[u8],
    dest_profile_icc: &[u8],
    intent: u32,
) -> Result<Vec<u8>, JsError> {
    let expected_len = (width as usize) * (height as usize) * 4;
    if pixels.len() != expected_len {
        return Err(JsError::new(&format!(
            "Expected {} bytes, got {}", expected_len, pixels.len()
        )));
    }

    let source = Profile::new_icc(source_profile_icc)
        .map_err(|e| JsError::new(&format!("Invalid source profile: {:?}", e)))?;
    let dest = Profile::new_icc(dest_profile_icc)
        .map_err(|e| JsError::new(&format!("Invalid dest profile: {:?}", e)))?;

    let rendering_intent = intent_from_u32(intent)?;

    let transform = Transform::new(
        &source,
        PixelFormat::RGBA_8,
        &dest,
        PixelFormat::RGBA_8,
        rendering_intent,
    )
    .map_err(|e| JsError::new(&format!("Transform failed: {:?}", e)))?;

    let mut output = vec![0u8; expected_len];
    transform.transform_pixels(
        bytemuck::cast_slice::<u8, [u8; 4]>(pixels),
        bytemuck::cast_slice_mut::<u8, [u8; 4]>(&mut output),
    );
    
    // Preserve alpha channel
    for i in 0..(pixels.len() / 4) {
        output[i * 4 + 3] = pixels[i * 4 + 3];
    }

    Ok(output)
}

fn intent_from_u32(value: u32) -> Result<Intent, JsError> {
    match value {
        0 => Ok(Intent::Perceptual),
        1 => Ok(Intent::RelativeColorimetric),
        2 => Ok(Intent::Saturation),
        3 => Ok(Intent::AbsoluteColorimetric),
        _ => Err(JsError::new(&format!("Unknown intent: {}", value))),
    }
}
