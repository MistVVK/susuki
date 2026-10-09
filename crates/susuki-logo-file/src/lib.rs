//! Derived from logoframe (https://github.com/tobitti0/logoframe,
//! commit c93aec0c89838e9b4dd3a9a3d5496dfaaa41311d), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Design Appendix A: logo file reading and validation (normative byte layout
//! REF-LOGO `src/logo.h` @c93aec0c; validation rules mirror the Kikyo
//! CMLogoScanner.py reference implementation per Appendix A.2).

use std::path::Path;

/// validation error codes (all map to CLI exit code 2, design 6.3.4)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogoFileError(pub &'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoFormat {
    AviUtlV01,
    AmatsukazeExtendedV1,
}

/// One logo as defined by Appendix A. Luminance channels only are kept
/// (A.1: chroma channels are read and discarded; REF-LOGO detection uses Y only).
#[derive(Debug, Clone)]
pub struct LogoFile {
    pub format: LogoFormat,
    /// decoded name (CP932, U+FFFD replacement per A.3; extended name when present)
    pub name: String,
    /// base header name (for the extended fallback rule)
    pub base_name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub fade_in: i16,
    pub fade_out: i16,
    pub start: i16,
    pub end: i16,
    /// Amatsukaze extension fields (None for bare v0.1)
    pub image_width: Option<i32>,
    pub image_height: Option<i32>,
    pub service_id: Option<i32>,
    /// dp_y per pixel (0..1000 opacity)
    pub dp_y: Vec<i16>,
    /// y luminance per pixel
    pub lum: Vec<i16>,
}

impl LogoFile {
    /// effective name (Kikyo reference rule: extended name wins, base fallback)
    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            &self.base_name
        } else {
            &self.name
        }
    }
}

const MAGIC_TEXT: &[u8; 23] = b"<logo data file ver0.1>";
const V02_PREFIX: &[u8] = b"<logo data file ver0.2";
const FILE_HEADER_SIZE: usize = 32; // str[28] + logonum u32 BE
const LOGO_HEADER_SIZE: usize = 48; // name[32] + 8 x i16 LE
const PIXEL_SIZE: usize = 12; // 6 x i16 LE
const MAX_LOGO_PIXELS: i64 = 4_194_304;
const EXT_HEADER_SIZE: usize = 540; // struct '<10i255sx i60i'
const EXT_MAGIC: i32 = 0x12345;
const EXT_VERSION: i32 = 1;

/// Read and validate one logo file (Appendix A rules).
pub fn read_logo(path: &Path) -> Result<LogoFile, LogoFileError> {
    if let Some(ext) = path.extension() {
        if ext.to_string_lossy().eq_ignore_ascii_case("lgd2") {
            return Err(LogoFileError("LogoFormatUnsupported"));
        }
    }
    let content = std::fs::read(path).map_err(|_| LogoFileError("LogoFileTruncated"))?;
    parse_logo(&content)
}

/// Pure parser over file bytes (shared by CLI and self-checks).
pub fn parse_logo(content: &[u8]) -> Result<LogoFile, LogoFileError> {
    let minimum = FILE_HEADER_SIZE + LOGO_HEADER_SIZE;
    if content.len() < minimum {
        return Err(LogoFileError("LogoFileTruncated"));
    }
    // LOGO_FILE_HEADER
    let str_field = &content[0..28];
    let text = trim_nul(str_field);
    if text.starts_with(V02_PREFIX) {
        return Err(LogoFileError("LogoFormatUnsupported"));
    }
    if text != MAGIC_TEXT {
        return Err(LogoFileError("LogoHeaderInvalid"));
    }
    let logo_count = u32::from_be_bytes([content[28], content[29], content[30], content[31]]);
    if logo_count != 1 {
        return Err(LogoFileError("LogoFormatUnsupported"));
    }
    // LOGO_HEADER (LE)
    let base = &content[FILE_HEADER_SIZE..FILE_HEADER_SIZE + LOGO_HEADER_SIZE];
    let name_bytes = &base[0..32];
    let i16le = |o: usize| i16::from_le_bytes([base[o], base[o + 1]]) as i32;
    let x = i16le(32);
    let y = i16le(34);
    let height = i16le(36);
    let width = i16le(38);
    let fi = i16le(40) as i16;
    let fo = i16le(42) as i16;
    let st = i16le(44) as i16;
    let ed = i16le(46) as i16;

    let pixel_count = width as i64 * height as i64;
    if width <= 0 || height <= 0 || x < 0 || y < 0 || pixel_count > MAX_LOGO_PIXELS {
        return Err(LogoFileError("LogoDimensionsInvalid"));
    }
    let pixel_offset = FILE_HEADER_SIZE + LOGO_HEADER_SIZE;
    let pixel_end = pixel_offset + (pixel_count as usize) * PIXEL_SIZE;
    if content.len() < pixel_end {
        return Err(LogoFileError("LogoFileTruncated"));
    }
    let mut dp_y = vec![0i16; pixel_count as usize];
    let mut yy = vec![0i16; pixel_count as usize];
    for i in 0..pixel_count as usize {
        let p = &content[pixel_offset + i * PIXEL_SIZE..];
        dp_y[i] = i16::from_le_bytes([p[0], p[1]]);
        yy[i] = i16::from_le_bytes([p[2], p[3]]);
    }
    let base_name = decode_cp932(name_bytes.split(|&b| b == 0).next().unwrap_or(name_bytes));

    if content.len() == pixel_end {
        return Ok(LogoFile {
            format: LogoFormat::AviUtlV01,
            name: base_name.clone(),
            base_name,
            x,
            y,
            width,
            height,
            fade_in: fi,
            fade_out: fo,
            start: st,
            end: ed,
            image_width: None,
            image_height: None,
            service_id: None,
            dp_y,
            lum: yy,
        });
    }

    let ext_end = pixel_end + EXT_HEADER_SIZE;
    if content.len() < ext_end {
        return Err(LogoFileError("LogoFileTruncated"));
    }
    let i32le =
        |o: usize| i32::from_le_bytes([content[o], content[o + 1], content[o + 2], content[o + 3]]);
    let magic = i32le(pixel_end);
    let version = i32le(pixel_end + 4);
    let ext_w = i32le(pixel_end + 8);
    let ext_h = i32le(pixel_end + 12);
    let uv_x = i32le(pixel_end + 16);
    let uv_y = i32le(pixel_end + 20);
    let image_width = i32le(pixel_end + 24);
    let image_height = i32le(pixel_end + 28);
    let ext_x = i32le(pixel_end + 32);
    let ext_y = i32le(pixel_end + 36);
    let ext_name_bytes = &content[pixel_end + 40..pixel_end + 295];
    let service_id = i32le(pixel_end + 296);
    if magic != EXT_MAGIC || version != EXT_VERSION {
        return Err(LogoFileError("LogoExtendedHeaderInvalid"));
    }
    if !(0..=2).contains(&uv_x) || !(0..=2).contains(&uv_y) {
        return Err(LogoFileError("LogoExtendedHeaderInvalid"));
    }
    if ext_w != width
        || ext_h != height
        || ext_x != x
        || ext_y != y
        || image_width <= 0
        || image_height <= 0
        || ext_x + ext_w > image_width
        || ext_y + ext_h > image_height
        || !(1..=0xFFFF).contains(&service_id)
    {
        return Err(LogoFileError("LogoExtendedHeaderInvalid"));
    }
    let uv_width = ext_w as i64 >> uv_x;
    let uv_height = ext_h as i64 >> uv_y;
    let float_count = (ext_w as i64 * ext_h as i64 + uv_width * uv_height * 2) * 2;
    let expected = ext_end as i64 + float_count * 4;
    if content.len() as i64 != expected {
        return Err(LogoFileError("LogoExtendedDataInvalid"));
    }
    let ext_name = decode_cp932(
        ext_name_bytes
            .split(|&b| b == 0)
            .next()
            .unwrap_or(ext_name_bytes),
    );
    Ok(LogoFile {
        format: LogoFormat::AmatsukazeExtendedV1,
        name: ext_name,
        base_name,
        x,
        y,
        width,
        height,
        fade_in: fi,
        fade_out: fo,
        start: st,
        end: ed,
        image_width: Some(image_width),
        image_height: Some(image_height),
        service_id: Some(service_id),
        dp_y,
        lum: yy,
    })
}

fn trim_nul(mut b: &[u8]) -> &[u8] {
    while b.last() == Some(&0) {
        b = &b[..b.len() - 1];
    }
    b
}

/// CP932 decode with U+FFFD replacement (Appendix A.3: never rejected).
fn decode_cp932(bytes: &[u8]) -> String {
    let (cow, _, _) = encoding_rs::SHIFT_JIS.decode(bytes);
    cow.into_owned()
}
