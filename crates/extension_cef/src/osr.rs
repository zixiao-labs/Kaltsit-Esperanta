//! Off-screen rendering helpers: BGRA paint buffers → platform frames.
//!
//! Frames cross the async host channel as CPU BGRA ([`SharedPaintFrame`]), which
//! is `Send`. On macOS the GPUI thread converts into a `CVPixelBuffer` for
//! `gpui::surface` (NV12).

use std::sync::Arc;

use anyhow::{Result, anyhow};

use crate::host::{BrowserId, PaintBuffer};

/// Sendable BGRA frame shared across the host → UI boundary.
#[derive(Clone, Debug)]
pub struct SharedPaintFrame {
    pub browser_id: BrowserId,
    pub width: u32,
    pub height: u32,
    pub bgra: Arc<[u8]>,
}

impl SharedPaintFrame {
    pub fn from_paint(paint: &PaintBuffer) -> Result<Self> {
        let expected = paint
            .width
            .checked_mul(paint.height)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| anyhow!("paint buffer dimensions overflow"))?
            as usize;
        if paint.bytes.len() < expected {
            return Err(anyhow!(
                "paint buffer too small: got {} want {expected}",
                paint.bytes.len()
            ));
        }
        Ok(Self {
            browser_id: paint.browser_id,
            width: paint.width,
            height: paint.height,
            bgra: Arc::from(&paint.bytes[..expected]),
        })
    }

    /// Convert to an NV12 `CVPixelBuffer` for `gpui::surface` (call on UI thread).
    #[cfg(target_os = "macos")]
    pub fn to_cv_pixel_buffer(&self) -> Result<core_video::pixel_buffer::CVPixelBuffer> {
        bgra_to_nv12_pixel_buffer(self.width, self.height, &self.bgra)
    }
}

/// Create a solid-color stub paint buffer (BGRA) for tests and unavailable CEF.
pub fn solid_color_paint(
    browser_id: BrowserId,
    width: u32,
    height: u32,
    bgra: [u8; 4],
) -> PaintBuffer {
    let pixels = (width as usize).saturating_mul(height as usize);
    let mut bytes = Vec::with_capacity(pixels.saturating_mul(4));
    for _ in 0..pixels {
        bytes.extend_from_slice(&bgra);
    }
    PaintBuffer {
        browser_id,
        width,
        height,
        bytes,
    }
}

pub fn paint_buffer_to_shared_frame(paint: &PaintBuffer) -> Result<SharedPaintFrame> {
    SharedPaintFrame::from_paint(paint)
}

/// BT.601 full-range style BGRA → NV12 conversion into a CVPixelBuffer.
#[cfg(target_os = "macos")]
pub fn bgra_to_nv12_pixel_buffer(
    width: u32,
    height: u32,
    bgra: &[u8],
) -> Result<core_video::pixel_buffer::CVPixelBuffer> {
    use core_foundation::base::TCFType;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_video::pixel_buffer::{
        self, CVPixelBuffer, kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
    };
    use core_video::pixel_buffer_io_surface::kCVPixelBufferIOSurfaceCoreAnimationCompatibilityKey;
    use core_video::r#return::kCVReturnSuccess;

    let width_key: CFString =
        unsafe { CFString::wrap_under_get_rule(pixel_buffer::kCVPixelBufferWidthKey) };
    let height_key: CFString =
        unsafe { CFString::wrap_under_get_rule(pixel_buffer::kCVPixelBufferHeightKey) };
    let animation_key: CFString = unsafe {
        CFString::wrap_under_get_rule(kCVPixelBufferIOSurfaceCoreAnimationCompatibilityKey)
    };
    let format_key: CFString =
        unsafe { CFString::wrap_under_get_rule(pixel_buffer::kCVPixelBufferPixelFormatTypeKey) };

    let yes: CFNumber = 1.into();
    let width_number: CFNumber = (width as i32).into();
    let height_number: CFNumber = (height as i32).into();
    let format: CFNumber = (kCVPixelFormatType_420YpCbCr8BiPlanarFullRange as i64).into();

    let attrs = CFDictionary::from_CFType_pairs(&[
        (width_key, width_number.into_CFType()),
        (height_key, height_number.into_CFType()),
        (animation_key, yes.into_CFType()),
        (format_key, format.into_CFType()),
    ]);

    let pixel_buffer = CVPixelBuffer::new(
        kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
        width as usize,
        height as usize,
        Some(&attrs),
    )
    .map_err(|cv_return| anyhow!("CVPixelBuffer::new failed: CVReturn({cv_return})"))?;

    if pixel_buffer.lock_base_address(0) != kCVReturnSuccess {
        return Err(anyhow!("failed to lock CVPixelBuffer"));
    }

    let result: Result<()> = {
        let y_base = unsafe { pixel_buffer.get_base_address_of_plane(0) as *mut u8 };
        let y_stride = pixel_buffer.get_bytes_per_row_of_plane(0);
        let uv_base = unsafe { pixel_buffer.get_base_address_of_plane(1) as *mut u8 };
        let uv_stride = pixel_buffer.get_bytes_per_row_of_plane(1);

        for row in 0..height as usize {
            for col in 0..width as usize {
                let i = (row * width as usize + col) * 4;
                let b = bgra[i] as i32;
                let g = bgra[i + 1] as i32;
                let r = bgra[i + 2] as i32;
                let y = ((66 * r + 129 * g + 25 * b + 128) >> 8) + 16;
                let y = y.clamp(0, 255) as u8;
                unsafe {
                    *y_base.add(row * y_stride + col) = y;
                }

                if row % 2 == 0 && col % 2 == 0 {
                    let u = ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128;
                    let v = ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128;
                    let uv_index = (row / 2) * uv_stride + (col / 2) * 2;
                    unsafe {
                        *uv_base.add(uv_index) = u.clamp(0, 255) as u8;
                        *uv_base.add(uv_index + 1) = v.clamp(0, 255) as u8;
                    }
                }
            }
        }
        Ok(())
    };

    pixel_buffer.unlock_base_address(0);
    result?;
    Ok(pixel_buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solid_paint_has_expected_size() {
        let paint = solid_color_paint(BrowserId(1), 4, 3, [0, 0, 255, 255]);
        assert_eq!(paint.bytes.len(), 4 * 3 * 4);
        let frame = paint_buffer_to_shared_frame(&paint).expect("convert");
        assert_eq!(frame.width, 4);
        assert_eq!(frame.height, 3);
        assert_eq!(frame.bgra.len(), 48);

        #[cfg(target_os = "macos")]
        {
            let buffer = frame.to_cv_pixel_buffer().expect("nv12");
            assert_eq!(buffer.get_width(), 4);
            assert_eq!(buffer.get_height(), 3);
        }
    }
}
