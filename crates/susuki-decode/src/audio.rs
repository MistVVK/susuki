//! Native-rate audio peaks for the selected stream. Format conversion only:
//! no resampling, channel mixing, channel exclusion, or delayed samples.

use crate::error::DecodeError;
use crate::frame::AudioChunk;
use crate::sys;

/// Reduce every channel at each native sample position to its s16-equivalent
/// absolute amplitude. `extended_data` includes planar channels beyond eight.
///
/// # Safety
/// `frame` must be a valid decoded audio AVFrame with live sample buffers.
pub unsafe fn audio_peaks(frame: &sys::AVFrame) -> Result<AudioChunk, DecodeError> {
    if frame.sample_rate <= 0 || frame.ch_layout.nb_channels <= 0 || frame.nb_samples < 0 {
        return Err(DecodeError::Runtime(
            "invalid decoded audio parameters".into(),
        ));
    }
    let format = frame.format as sys::AVSampleFormat;
    let packed = sys::av_get_packed_sample_fmt(format);
    let planar = sys::av_sample_fmt_is_planar(format) != 0;
    let bytes = sys::av_get_bytes_per_sample(format);
    if bytes <= 0 || frame.extended_data.is_null() {
        return Err(DecodeError::Runtime(
            "unsupported decoded audio format".into(),
        ));
    }
    let channels = frame.ch_layout.nb_channels as usize;
    let mut peaks = vec![0u16; frame.nb_samples as usize];
    for channel in 0..channels {
        let data = *frame.extended_data.add(if planar { channel } else { 0 });
        if data.is_null() {
            return Err(DecodeError::Runtime("missing decoded audio plane".into()));
        }
        for (sample, peak) in peaks.iter_mut().enumerate() {
            let index = if planar {
                sample
            } else {
                sample * channels + channel
            };
            let p = data.add(index * bytes as usize);
            let value = match packed {
                sys::AVSampleFormat_AV_SAMPLE_FMT_U8 => (i16::from(*p) - 128) << 8,
                sys::AVSampleFormat_AV_SAMPLE_FMT_S16 => (p as *const i16).read_unaligned(),
                sys::AVSampleFormat_AV_SAMPLE_FMT_S32 => {
                    ((p as *const i32).read_unaligned() >> 16) as i16
                }
                sys::AVSampleFormat_AV_SAMPLE_FMT_S64 => {
                    ((p as *const i64).read_unaligned() >> 48) as i16
                }
                sys::AVSampleFormat_AV_SAMPLE_FMT_FLT => {
                    float_s16(f64::from((p as *const f32).read_unaligned()))
                }
                sys::AVSampleFormat_AV_SAMPLE_FMT_DBL => {
                    float_s16((p as *const f64).read_unaligned())
                }
                _ => {
                    return Err(DecodeError::Runtime(
                        "unsupported decoded audio format".into(),
                    ))
                }
            };
            // -32768 has amplitude 32768; signed abs would overflow.
            *peak = (*peak).max(value.unsigned_abs());
        }
    }
    Ok(AudioChunk {
        peaks,
        sample_rate: frame.sample_rate as u32,
    })
}

fn float_s16(value: f64) -> i16 {
    // Match format-only PCM conversion: scale, nearest-even, saturate.
    // A non-finite sample must not become silence.
    if !value.is_finite() {
        return i16::MIN;
    }
    (value * 32768.0).round_ties_even().clamp(-32768.0, 32767.0) as i16
}
