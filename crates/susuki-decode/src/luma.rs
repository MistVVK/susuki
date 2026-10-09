//! Design 6.0: canonical 8-bit luma generation from decoded frames.
//! Bit-depth reduction matches Kikyo logoframe-native-luma.patch
//! (`build_luma_to_8bit`), the design's compatibility anchor.

use crate::sys;

#[derive(Debug, Clone)]
pub struct Luma {
    pub width: usize,
    pub height: usize,
    /// one byte per pixel, row stride = width (data is copied out of the frame)
    pub data: Vec<u8>,
}

/// bit depth of the luma component of an AVFrame's pixel format.
pub fn frame_bit_depth(frame: &sys::AVFrame) -> u32 {
    unsafe {
        let desc = sys::av_pix_fmt_desc_get(frame.format);
        if desc.is_null() {
            return 8;
        }
        (*desc).comp[0].depth.max(8) as u32
    }
}

/// §6.0 rule 2-3: limited/full range from frame->color_range; no range expansion
/// for 8-bit limited (design: analyze the same raw luma values the reference got).
pub fn frame_is_full_range(frame: &sys::AVFrame) -> bool {
    frame.color_range == sys::AVColorRange_AVCOL_RANGE_JPEG
}

/// Extract 8-bit Y plane from a system-memory frame (after hw transfer).
/// Interlaced field selection (§6.0 rule 4) is applied by analyzers via
/// line stride; the plane itself is delivered whole.
///
/// # Safety
/// `frame` must be a valid, complete decoded AVFrame in software pixel formats.
pub unsafe fn extract_luma(frame: &sys::AVFrame) -> Option<Luma> {
    let (w, h) = (frame.width as usize, frame.height as usize);
    if w == 0 || h == 0 {
        return None;
    }
    let plane = frame.data[0];
    if plane.is_null() {
        return None;
    }
    let stride = frame.linesize[0] as usize;
    let depth = frame_bit_depth(frame);
    let full = frame_is_full_range(frame);
    let mut out = vec![0u8; w * h];
    if depth <= 8 {
        for y in 0..h {
            let src = std::slice::from_raw_parts(plane.add(y * stride), w);
            out[y * w..y * w + w].copy_from_slice(src);
        }
    } else {
        // >8bit: samples are little-endian 16-bit units
        let max: i64 = (1i64 << depth) - 1;
        let shift = depth - 8;
        // P010 stores its 10-bit component in the high bits of each u16;
        // planar 10-bit formats store it in the low bits.
        let component_shift = (*sys::av_pix_fmt_desc_get(frame.format)).comp[0].shift as u32;
        for y in 0..h {
            let row = std::slice::from_raw_parts((plane.add(y * stride)) as *const u16, w);
            for x in 0..w {
                let v = (row[x] >> component_shift) as i64;
                let v8 = if full {
                    ((v * 255 + max / 2) / max).min(255) as u8
                } else {
                    ((v + (1i64 << (shift - 1))) >> shift).min(255) as u8
                };
                out[y * w + x] = v8;
            }
        }
    }
    Some(Luma {
        width: w,
        height: h,
        data: out,
    })
}

/// §6.1 shrink ratio: s = ceil(w16 / 1920) with w16 = floor(width/16)*16
/// (<= 1920 gives s = 1).
pub fn scale_factor(width: usize) -> usize {
    let w16 = (width / 16) * 16;
    if w16 == 0 {
        return 1;
    }
    w16.div_ceil(1920)
}

/// §6.1 box-average downscale, deterministic integer arithmetic.
/// dst size is floor(w16/s) x floor(h16/s); leftover right/bottom pixels dropped.
/// Returns an image whose width/height are the mvec `lx`/`ly` inputs (§6.1).
pub fn downscale(src: &Luma, s: usize) -> Luma {
    let w16 = (src.width / 16) * 16;
    let h16 = (src.height / 16) * 16;
    if s <= 1 {
        // s = 1 is the reference-equivalent input; crop to w16/h16 boundary
        // exactly as larger scales do (mvec reads lx x ly only)
        let mut data = vec![0u8; w16 * h16];
        for y in 0..h16 {
            let row = &src.data[y * src.width..y * src.width + w16];
            data[y * w16..y * w16 + w16].copy_from_slice(row);
        }
        return Luma {
            width: w16,
            height: h16,
            data,
        };
    }
    let dw = w16 / s;
    let dh = h16 / s;
    let half = (s * s) / 2;
    let mut data = vec![0u8; dw * dh];
    for y in 0..dh {
        for x in 0..dw {
            let mut acc: u32 = 0;
            for dy in 0..s {
                let sy = y * s + dy;
                let row = &src.data[sy * src.width..];
                for dx in 0..s {
                    acc += row[x * s + dx] as u32;
                }
            }
            data[y * dw + x] = ((acc + half as u32) / (s * s) as u32) as u8;
        }
    }
    Luma {
        width: dw,
        height: dh,
        data,
    }
}
