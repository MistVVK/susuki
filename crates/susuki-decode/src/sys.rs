//! bindgen-generated bindings for the dedicated FFmpeg build + the error-code shim.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(clippy::all)]
#![allow(unknown_lints, unnecessary_transmutes)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

extern "C" {
    fn susuki_averror_eof() -> i32;
    fn susuki_averror_eagain() -> i32;
    fn susuki_averror_enosys() -> i32;
    fn susuki_averror_enomem() -> i32;
    fn susuki_averror_einval() -> i32;
    fn susuki_averror_invaliddata() -> i32;
    fn susuki_averror_errno(e: i32) -> i32;
}

// AVERROR(x) values are C function-like macros; exposed via the shim as fns.
pub fn averror_eof() -> i32 {
    unsafe { susuki_averror_eof() }
}
pub fn averror_eagain() -> i32 {
    unsafe { susuki_averror_eagain() }
}
pub fn averror_enosys() -> i32 {
    unsafe { susuki_averror_enosys() }
}
pub fn averror_enomem() -> i32 {
    unsafe { susuki_averror_enomem() }
}
pub fn averror_einval() -> i32 {
    unsafe { susuki_averror_einval() }
}
pub fn averror_invaliddata() -> i32 {
    unsafe { susuki_averror_invaliddata() }
}
pub fn averror_errno(e: i32) -> i32 {
    unsafe { susuki_averror_errno(e) }
}

/// Decode an FFmpeg error code into a short message (for fallback_reason and stderr).
pub fn err_str(rc: i32) -> String {
    unsafe {
        let mut buf = [0i8; 256];
        if av_strerror(rc, buf.as_mut_ptr(), buf.len()) == 0 {
            let bytes: Vec<u8> = buf
                .iter()
                .take_while(|c| **c != 0)
                .map(|c| *c as u8)
                .collect();
            String::from_utf8_lossy(&bytes).into_owned()
        } else {
            format!("ffmpeg error {rc}")
        }
    }
}
