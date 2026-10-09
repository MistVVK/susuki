//! Design 4.1-4.6 / 8.2: the single DecoderEngine.
//! One demuxer + custom file IO (§4.6 interrupt callback), one video decoder
//! owner; frame sharing is done by the caller (§8.3).

use std::collections::VecDeque;
use std::ffi::CString;
use std::io::{Read, Seek, SeekFrom};
use std::os::raw::c_void;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::audio::audio_peaks;
use crate::device::{self, BackendKind, HwDevice};
use crate::error::DecodeError;
use crate::frame::{AudioChunk, DecodedFrame};
use crate::luma;
use crate::sys;

const IO_BUF_SIZE: usize = 4096;
// Linux errno values used for AVERROR mapping in the IO callbacks
const EINTR: i32 = 4;
const EIO: i32 = 5;

/// Neutral input-selection view of the CLI options (§3.2.1, §4.5).
#[derive(Debug, Default, Clone)]
pub struct InputSpec {
    pub video_ordinal: Option<u32>,
    pub video_abs: Option<i32>,
    pub audio_ordinal: Option<u32>,
    pub audio_abs: Option<i32>,
    pub audio_none: bool,
    pub tlv_service: Option<i64>,
}

/// Metadata for JSON input/decode blocks (§9.2) and §4.5 selection reasons.
#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub video_stream_index: i64,
    pub audio_stream_index: Option<i64>,
    pub width: i64,
    pub height: i64,
    pub video_codec: String,
    pub interlaced: bool,
    pub bit_depth: u32,
    pub fps_num: i64,
    pub fps_den: i64,
    pub video_selection_reason: String,
    pub audio_selection_reason: String,
}

pub enum FrameEvent {
    Video(DecodedFrame),
    Audio(AudioChunk),
}

/// State consulted by the get_format callback (§4.4 hw pix fmt request).
struct HwState {
    allowed: Vec<i32>,
    /// none of the requested hw formats were offered by the decoder
    rejected: bool,
}

/// Custom IO backing: our own File handle + the §4.6 interrupt flag.
struct IoState {
    file: std::fs::File,
    interrupt: Arc<AtomicBool>,
}

extern "C" fn io_read(opaque: *mut c_void, buf: *mut u8, size: i32) -> i32 {
    let st = unsafe { &mut *(opaque as *mut IoState) };
    if st.interrupt.load(Ordering::Relaxed) {
        return sys::averror_errno(EINTR);
    }
    let slice = unsafe { std::slice::from_raw_parts_mut(buf, size.max(0) as usize) };
    match st.file.read(slice) {
        Ok(0) => sys::averror_eof(),
        Ok(n) => n as i32,
        Err(_) => sys::averror_errno(EIO),
    }
}

extern "C" fn io_seek(opaque: *mut c_void, offset: i64, whence: i32) -> i64 {
    let st = unsafe { &mut *(opaque as *mut IoState) };
    let target = match whence {
        0 => SeekFrom::Start(offset.max(0) as u64),
        1 => SeekFrom::Current(offset),
        2 => SeekFrom::End(offset),
        _ => return -1,
    };
    match st.file.seek(target) {
        Ok(p) => p as i64,
        Err(_) => -1,
    }
}

extern "C" fn get_format_cb(
    ctx: *mut sys::AVCodecContext,
    fmts: *const sys::AVPixelFormat,
) -> sys::AVPixelFormat {
    let p = unsafe { (*ctx).opaque } as *mut HwState;
    if p.is_null() {
        return unsafe { *fmts };
    }
    let state = unsafe { &mut *p };
    let mut i = 0usize;
    loop {
        let f = unsafe { *fmts.add(i) };
        if f == sys::AVPixelFormat_AV_PIX_FMT_NONE {
            break;
        }
        if state.allowed.contains(&{ f }) {
            return f;
        }
        i += 1;
    }
    state.rejected = true;
    // reject silently falling back to software: for GPU-supported inputs the
    // auto-switch is forbidden (REQ-DC-002 / §4.4)
    sys::AVPixelFormat_AV_PIX_FMT_NONE
}

pub struct DecoderEngine {
    fmt: *mut sys::AVFormatContext,
    iobuf: *mut sys::AVIOContext,
    io_state: Option<Box<IoState>>,
    vdec: *mut sys::AVCodecContext,
    /// keeps the HwState box alive while AVCodecContext.opaque points into it
    _hw_state: Option<Box<HwState>>,
    adec: Option<*mut sys::AVCodecContext>,
    pub info: StreamInfo,
    _hw: Option<HwDevice>,
    pub backend: BackendKind,
    pub gpu_unsupported: bool,
    pub fallback_reason: Option<String>,
    video_index: i32,
    audio_index: Option<i32>,
    frame_index: i64,
    interrupt: Arc<AtomicBool>,
    queued: VecDeque<FrameEvent>,
    draining: bool,
    video_done: bool,
    audio_done: bool,
}

unsafe impl Send for DecoderEngine {}

fn cstr(s: &str) -> Result<CString, DecodeError> {
    CString::new(s).map_err(|_| DecodeError::Io("string contains NUL byte".to_string()))
}

fn codec_name(id: sys::AVCodecID) -> String {
    unsafe {
        let n = sys::avcodec_get_name(id);
        std::ffi::CStr::from_ptr(n).to_string_lossy().into_owned()
    }
}

/// §5.2 nominal frame rate: avg_frame_rate, fallback r_frame_rate; both
/// indeterminate -> exit 4 with this reason (card-1 #14 note).
fn nominal_fps(st: *mut sys::AVStream) -> Option<(i64, i64)> {
    unsafe {
        let avg = (*st).avg_frame_rate;
        let r = (*st).r_frame_rate;
        if avg.num > 0 && avg.den > 0 {
            Some((avg.num as i64, avg.den as i64))
        } else if r.num > 0 && r.den > 0 {
            Some((r.num as i64, r.den as i64))
        } else {
            None
        }
    }
}

fn stream_at(fmt: *mut sys::AVFormatContext, i: usize) -> *mut sys::AVStream {
    unsafe { *(*fmt).streams.add(i) }
}

fn reason_of(ordinal: Option<u32>, abs: Option<i32>) -> String {
    match (ordinal, abs) {
        (Some(n), _) => format!("explicit ordinal {n}"),
        (None, Some(i)) => format!("explicit absolute index {i}"),
        _ => "av_find_best_stream".to_string(),
    }
}

impl DecoderEngine {
    /// Open + analyze + select (§4.5) + backend (§4.3) + decoders (§4.4).
    pub fn open(
        path: &Path,
        spec: &InputSpec,
        backend: Option<BackendKind>,
        device: Option<&str>,
        interrupt: Arc<AtomicBool>,
    ) -> Result<Self, DecodeError> {
        let flag = interrupt.clone();
        match Self::open_inner(path, spec, backend, device, interrupt) {
            // §4.6: io_read aborts with EINTR once a cancel is requested, and
            // FFmpeg reports that as an open/probe failure; it is a cancel (130),
            // not an input error (3) or an unsupported input (4)
            Err(_) if flag.load(Ordering::Relaxed) => Err(DecodeError::Interrupted),
            r => r,
        }
    }

    fn open_inner(
        path: &Path,
        spec: &InputSpec,
        backend: Option<BackendKind>,
        device: Option<&str>,
        interrupt: Arc<AtomicBool>,
    ) -> Result<Self, DecodeError> {
        // §3.4: nonexistent / unreadable input -> exit 3
        let file =
            std::fs::File::open(path).map_err(|e| DecodeError::io_err("cannot open input", e))?;
        let meta = file
            .metadata()
            .map_err(|e| DecodeError::io_err("cannot stat input", e))?;
        if !meta.is_file() {
            return Err(DecodeError::Io("input is not a regular file".to_string()));
        }

        let cpath = cstr(&path.to_string_lossy())?;
        let mut opts: *mut sys::AVDictionary = std::ptr::null_mut();
        // §4.5: --tlv-service maps to demuxer option `service`; default -1 (all)
        let svc = spec
            .tlv_service
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-1".to_string());
        let csvc = cstr(&svc)?;
        unsafe { sys::av_dict_set(&mut opts, c"service".as_ptr(), csvc.as_ptr(), 0) };

        // §4.6: custom AVIO so SIGINT/SIGTERM stops reads at the callback level
        let io_state = Box::new(IoState {
            file,
            interrupt: interrupt.clone(),
        });
        let io_ptr: *mut IoState = Box::into_raw(io_state);
        let iobuffer = unsafe { sys::av_malloc(IO_BUF_SIZE) as *mut u8 };
        let iobuf = unsafe {
            sys::avio_alloc_context(
                iobuffer,
                IO_BUF_SIZE as i32,
                0,
                io_ptr as *mut c_void,
                Some(io_read),
                None,
                Some(io_seek),
            )
        };
        if iobuf.is_null() || iobuffer.is_null() {
            unsafe {
                if !iobuf.is_null() {
                    sys::av_free((*iobuf).buffer as *mut c_void);
                    sys::avio_context_free(&mut { iobuf });
                } else if !iobuffer.is_null() {
                    sys::av_free(iobuffer as *mut c_void);
                }
                let _ = Box::from_raw(io_ptr);
            }
            return Err(DecodeError::Runtime(
                "avio_alloc_context failed".to_string(),
            ));
        }
        let mut fmt = unsafe { sys::avformat_alloc_context() };
        if fmt.is_null() {
            unsafe {
                sys::av_free(iobuffer as *mut c_void);
                sys::avio_context_free(&mut { iobuf });
                let _ = Box::from_raw(io_ptr);
            }
            return Err(DecodeError::Runtime(
                "avformat_alloc_context failed".to_string(),
            ));
        }
        unsafe {
            (*fmt).pb = iobuf;
            // §4.6: our own IO context - avformat must never close/free it
            // (and never reallocates ownership semantics we rely on below)
            (*fmt).flags |= sys::AVFMT_FLAG_CUSTOM_IO as i32;
        }
        let rc = unsafe {
            sys::avformat_open_input(&mut fmt, cpath.as_ptr(), std::ptr::null_mut(), &mut opts)
        };
        unsafe {
            if !opts.is_null() {
                sys::av_dict_free(&mut opts);
            }
        }
        if rc != 0 {
            // open-stage failures are input errors (§3.4/§3.6; spec-notes N2)
            unsafe {
                cleanup_io(iobuf, io_ptr);
            }
            return Err(DecodeError::Io(format!(
                "avformat_open_input: {}",
                sys::err_str(rc)
            )));
        }

        if unsafe { sys::avformat_find_stream_info(fmt, std::ptr::null_mut()) } < 0 {
            unsafe { cleanup_live(fmt, iobuf, io_ptr) };
            return Err(DecodeError::Unsupported(
                "no media streams detected (unsupported format)".to_string(),
            ));
        }

        let video_index: i32 = match select_stream(
            fmt,
            spec.video_ordinal,
            spec.video_abs,
            sys::AVMediaType_AVMEDIA_TYPE_VIDEO,
        ) {
            Ok(Some(x)) => x,
            Ok(None) => {
                unsafe { cleanup_live(fmt, iobuf, io_ptr) };
                return Err(DecodeError::Unsupported("no video stream".to_string()));
            }
            Err(e) => {
                unsafe { cleanup_live(fmt, iobuf, io_ptr) };
                return Err(e);
            }
        };
        let audio_index: Option<i32> = if spec.audio_none {
            None
        } else {
            match select_stream(
                fmt,
                spec.audio_ordinal,
                spec.audio_abs,
                sys::AVMediaType_AVMEDIA_TYPE_AUDIO,
            ) {
                Ok(x) => x,
                Err(e) => {
                    unsafe { cleanup_live(fmt, iobuf, io_ptr) };
                    return Err(e);
                }
            }
        };

        // §4.3 backend selection (existence vs init failure; REQ-DC-002 limits)
        let sel = match backend {
            None => device::select_auto(&device::Probe::new(), device),
            Some(k) => device::select_explicit(k, device).map(|hw| (k, hw)),
        };
        let (chosen, hw) = match sel {
            Ok(x) => x,
            Err(e) => {
                unsafe { cleanup_live(fmt, iobuf, io_ptr) };
                return Err(e);
            }
        };

        let par = unsafe { (*stream_at(fmt, video_index as usize)).codecpar };
        let vcodec = unsafe { sys::avcodec_find_decoder((*par).codec_id) };
        if vcodec.is_null() {
            unsafe { cleanup_live(fmt, iobuf, io_ptr) };
            return Err(DecodeError::Unsupported(format!(
                "no decoder for video codec {}",
                codec_name(unsafe { (*par).codec_id })
            )));
        }
        let inited = match init_video_decoder(par, vcodec, chosen, hw.as_ref()) {
            Ok(x) => x,
            Err(e) => {
                unsafe { cleanup_live(fmt, iobuf, io_ptr) };
                return Err(e);
            }
        };
        let (vdec, hw_state, backend_used, gpu_unsupported, fallback_reason) = inited;

        let info = match build_info(
            fmt,
            video_index,
            audio_index,
            vdec,
            &reason_of(spec.video_ordinal, spec.video_abs),
            &if spec.audio_none {
                "audio-stream=none".to_string()
            } else {
                reason_of(spec.audio_ordinal, spec.audio_abs)
            },
        ) {
            Ok(x) => x,
            Err(e) => {
                unsafe {
                    let mut m = vdec;
                    sys::avcodec_free_context(&mut m);
                    cleanup_live(fmt, iobuf, io_ptr);
                }
                return Err(e);
            }
        };

        let mut adec = None;
        if let Some(ai) = audio_index {
            let apar = unsafe { (*stream_at(fmt, ai as usize)).codecpar };
            let acodec = unsafe { sys::avcodec_find_decoder((*apar).codec_id) };
            if acodec.is_null() {
                unsafe {
                    let mut m = vdec;
                    sys::avcodec_free_context(&mut m);
                    cleanup_live(fmt, iobuf, io_ptr);
                }
                return Err(DecodeError::Unsupported(format!(
                    "no decoder for audio codec {}",
                    codec_name(unsafe { (*apar).codec_id })
                )));
            }
            let ctx = unsafe { sys::avcodec_alloc_context3(acodec) };
            if ctx.is_null() {
                unsafe { cleanup_live(fmt, iobuf, io_ptr) };
                return Err(DecodeError::Runtime(
                    "avcodec_alloc_context3(audio) failed".to_string(),
                ));
            }
            let rc = unsafe { sys::avcodec_parameters_to_context(ctx, apar) };
            if rc < 0 {
                unsafe {
                    let mut m = ctx;
                    sys::avcodec_free_context(&mut m);
                    let mut v = vdec;
                    sys::avcodec_free_context(&mut v);
                    cleanup_live(fmt, iobuf, io_ptr);
                }
                return Err(DecodeError::Runtime(format!(
                    "audio parameters_to_context: {}",
                    sys::err_str(rc)
                )));
            }
            let rc = unsafe { sys::avcodec_open2(ctx, acodec, std::ptr::null_mut()) };
            if rc < 0 {
                unsafe {
                    let mut m = ctx;
                    sys::avcodec_free_context(&mut m);
                    let mut v = vdec;
                    sys::avcodec_free_context(&mut v);
                    cleanup_live(fmt, iobuf, io_ptr);
                }
                return Err(DecodeError::Unsupported(format!(
                    "audio decoder open failed: {}",
                    sys::err_str(rc)
                )));
            }
            adec = Some(ctx);
        }

        Ok(DecoderEngine {
            fmt,
            iobuf,
            io_state: Some(unsafe { Box::from_raw(io_ptr) }),
            vdec,
            _hw_state: hw_state,
            adec,
            info,
            _hw: hw,
            backend: backend_used,
            gpu_unsupported,
            fallback_reason,
            video_index,
            audio_index,
            frame_index: 0,
            interrupt,
            queued: VecDeque::new(),
            draining: false,
            video_done: false,
            audio_done: audio_index.is_none(),
        })
    }

    /// Next decoded event; None at clean EOF (§4.6 flush/drain).
    pub fn next_event(&mut self) -> Result<Option<FrameEvent>, DecodeError> {
        loop {
            if self.interrupt.load(Ordering::Relaxed) {
                return Err(DecodeError::Interrupted);
            }
            if let Some(ev) = self.queued.pop_front() {
                return Ok(Some(ev));
            }
            if self.draining {
                if self.video_done && self.audio_done {
                    return Ok(None);
                }
                if !self.video_done {
                    self.video_done = self.pump_video(std::ptr::null_mut())?;
                }
                if !self.audio_done {
                    self.audio_done = self.pump_audio(std::ptr::null_mut())?;
                }
                continue;
            }
            let mut pkt = unsafe { sys::av_packet_alloc() };
            if pkt.is_null() {
                return Err(DecodeError::Runtime("av_packet_alloc failed".to_string()));
            }
            let rc = unsafe { sys::av_read_frame(self.fmt, pkt) };
            if rc == 0 {
                let si = unsafe { (*pkt).stream_index };
                if si == self.video_index {
                    if self.pump_video(pkt)? {
                        self.video_done = true;
                    }
                } else if Some(si) == self.audio_index && self.pump_audio(pkt)? {
                    self.audio_done = true;
                }
                unsafe { sys::av_packet_unref(pkt) };
            } else if rc == sys::averror_eof() {
                self.draining = true;
            } else {
                unsafe { sys::av_packet_free(&mut pkt) };
                // io_read reports the §4.6 interrupt as EINTR; that is a cancel, not an I/O error
                if self.interrupt.load(Ordering::Relaxed) {
                    return Err(DecodeError::Interrupted);
                }
                return Err(DecodeError::Io(format!(
                    "av_read_frame: {}",
                    sys::err_str(rc)
                )));
            }
            unsafe { sys::av_packet_free(&mut pkt) };
        }
    }

    /// Send packet (NULL to flush) then receive until needs-more or EOF.
    /// Returns true when the decoder reported EOF.
    fn pump_video(&mut self, pkt: *mut sys::AVPacket) -> Result<bool, DecodeError> {
        let rc = unsafe { sys::avcodec_send_packet(self.vdec, pkt) };
        if rc < 0 && rc != sys::averror_eagain() && rc != sys::averror_eof() {
            return Err(DecodeError::Runtime(format!(
                "avcodec_send_packet(video): {}",
                sys::err_str(rc)
            )));
        }
        let mut eof = false;
        loop {
            let mut frame = unsafe { sys::av_frame_alloc() };
            if frame.is_null() {
                return Err(DecodeError::Runtime("av_frame_alloc failed".to_string()));
            }
            let rc = unsafe { sys::avcodec_receive_frame(self.vdec, frame) };
            if rc == sys::averror_eagain() {
                unsafe { sys::av_frame_free(&mut frame) };
                break;
            } else if rc == sys::averror_eof() {
                unsafe { sys::av_frame_free(&mut frame) };
                eof = true;
                break;
            } else if rc < 0 {
                unsafe { sys::av_frame_free(&mut frame) };
                // §4.4: decode-time errors are never classified as unsupported -> 5
                return Err(DecodeError::Runtime(format!(
                    "avcodec_receive_frame(video): {}",
                    sys::err_str(rc)
                )));
            }
            let ev = self.emit_frame(frame);
            unsafe { sys::av_frame_free(&mut frame) };
            self.queued.push_back(ev?);
        }
        Ok(eof)
    }

    fn pump_audio(&mut self, pkt: *mut sys::AVPacket) -> Result<bool, DecodeError> {
        let adec = match self.adec {
            Some(a) => a,
            None => return Ok(true),
        };
        let rc = unsafe { sys::avcodec_send_packet(adec, pkt) };
        if rc < 0 && rc != sys::averror_eagain() && rc != sys::averror_eof() {
            return Err(DecodeError::Runtime(format!(
                "avcodec_send_packet(audio): {}",
                sys::err_str(rc)
            )));
        }
        let mut eof = false;
        loop {
            let mut frame = unsafe { sys::av_frame_alloc() };
            if frame.is_null() {
                return Err(DecodeError::Runtime("av_frame_alloc failed".to_string()));
            }
            let rc = unsafe { sys::avcodec_receive_frame(adec, frame) };
            if rc == sys::averror_eagain() {
                unsafe { sys::av_frame_free(&mut frame) };
                break;
            } else if rc == sys::averror_eof() {
                unsafe { sys::av_frame_free(&mut frame) };
                eof = true;
                break;
            } else if rc < 0 {
                unsafe { sys::av_frame_free(&mut frame) };
                return Err(DecodeError::Runtime(format!(
                    "avcodec_receive_frame(audio): {}",
                    sys::err_str(rc)
                )));
            }
            let chunk = unsafe { audio_peaks(&*frame) };
            unsafe { sys::av_frame_free(&mut frame) };
            self.queued.push_back(FrameEvent::Audio(chunk?));
        }
        Ok(eof)
    }

    /// §4.2 hw->system transfer + §6.0 luma + §5.1 sequential index.
    fn emit_frame(&mut self, frame: *mut sys::AVFrame) -> Result<FrameEvent, DecodeError> {
        // get_format negotiates during decoding, after open2, and may run again
        // on a format change. The received frame owns the HW context required
        // by av_hwframe_transfer_data; open-time ctx.pix_fmt is not authoritative.
        let is_hw = unsafe { !(*frame).hw_frames_ctx.is_null() };
        let luma_opt: Option<luma::Luma>;
        if is_hw {
            let mut sw = unsafe { sys::av_frame_alloc() };
            if sw.is_null() {
                return Err(DecodeError::Runtime(
                    "av_frame_alloc(sw) failed".to_string(),
                ));
            }
            let rc = unsafe { sys::av_hwframe_transfer_data(sw, frame, 0) };
            if rc < 0 {
                unsafe { sys::av_frame_free(&mut sw) };
                // §4.4: transfer failure is a runtime failure (5), not unsupported
                return Err(DecodeError::Runtime(format!(
                    "av_hwframe_transfer_data: {}",
                    sys::err_str(rc)
                )));
            }
            // HW transfer copies pixels, not frame properties. Normalization
            // must retain the source range, especially for full-range >8bit.
            unsafe { (*sw).color_range = (*frame).color_range };
            luma_opt = unsafe { luma::extract_luma(&*sw) };
            unsafe { sys::av_frame_free(&mut sw) };
        } else {
            luma_opt = unsafe { luma::extract_luma(&*frame) };
        }
        let luma = luma_opt.ok_or_else(|| {
            DecodeError::Unsupported("no luma plane in decoded frame".to_string())
        })?;
        let index = self.frame_index;
        self.frame_index += 1;
        Ok(FrameEvent::Video(DecodedFrame {
            index,
            pts: unsafe { (*frame).pts },
            luma: Arc::new(luma),
            interlaced: self.info.interlaced,
        }))
    }

    /// frames decoded so far (output end boundary §5.3 uses total frames)
    pub fn frames_emitted(&self) -> i64 {
        self.frame_index
    }
}

/// Cleanup where fmt was never successfully opened. With AVFMT_FLAG_CUSTOM_IO
/// avformat_open_input does not touch our pb; we release everything ourselves.
/// The context buffer may have been reallocated internally, so free through
/// the current (*iobuf).buffer, not the original pointer.
unsafe fn cleanup_io(mut iobuf: *mut sys::AVIOContext, io_ptr: *mut IoState) {
    if !iobuf.is_null() {
        sys::av_free((*iobuf).buffer as *mut c_void);
        sys::avio_context_free(&mut iobuf);
    }
    let _ = Box::from_raw(io_ptr);
}

/// Cleanup for the post-open failure paths (fmt alive, pb = our iobuf).
unsafe fn cleanup_live(
    mut fmt: *mut sys::AVFormatContext,
    mut iobuf: *mut sys::AVIOContext,
    io_ptr: *mut IoState,
) {
    if !fmt.is_null() {
        // detach pb so close_input does not attempt to free it
        (*fmt).pb = std::ptr::null_mut();
        sys::avformat_close_input(&mut fmt);
    }
    if !iobuf.is_null() {
        sys::av_free((*iobuf).buffer as *mut c_void);
        sys::avio_context_free(&mut iobuf);
    }
    let _ = Box::from_raw(io_ptr);
}

fn select_stream(
    fmt: *mut sys::AVFormatContext,
    ordinal: Option<u32>,
    abs: Option<i32>,
    mtype: sys::AVMediaType,
) -> Result<Option<i32>, DecodeError> {
    unsafe {
        let nb = (*fmt).nb_streams;
        if let Some(ai) = abs {
            if ai < 0 || (ai as u64) >= nb as u64 {
                return Err(DecodeError::Io(format!(
                    "stream index {ai} out of range (0..{nb})"
                )));
            }
            return Ok(Some(ai));
        }
        if let Some(ord) = ordinal {
            let mut seen: u32 = 0;
            for i in 0..nb {
                let st = stream_at(fmt, i as usize);
                if (*(*st).codecpar).codec_type == mtype {
                    if seen == ord {
                        return Ok(Some(i as i32));
                    }
                    seen += 1;
                }
            }
            return Err(DecodeError::Io(format!(
                "no stream with ordinal {ord} of requested type (found {seen})"
            )));
        }
    }
    // §4.5 default rule
    let idx = unsafe { sys::av_find_best_stream(fmt, mtype, -1, -1, std::ptr::null_mut(), 0) };
    if idx < 0 {
        Ok(None)
    } else {
        Ok(Some(idx))
    }
}

/// §4.4: static AVCodecHWConfig scan, attach device ctx, request hw pix fmt.
#[allow(clippy::type_complexity)]
fn init_video_decoder(
    par: *mut sys::AVCodecParameters,
    codec: *const sys::AVCodec,
    chosen: BackendKind,
    hw: Option<&HwDevice>,
) -> Result<
    (
        *mut sys::AVCodecContext,
        Option<Box<HwState>>,
        BackendKind,
        bool,
        Option<String>,
    ),
    DecodeError,
> {
    let alloc = || -> Result<*mut sys::AVCodecContext, DecodeError> {
        let ctx = unsafe { sys::avcodec_alloc_context3(codec) };
        if ctx.is_null() {
            return Err(DecodeError::Runtime(
                "avcodec_alloc_context3 failed".to_string(),
            ));
        }
        let rc = unsafe { sys::avcodec_parameters_to_context(ctx, par) };
        if rc < 0 {
            unsafe {
                let mut m = ctx;
                sys::avcodec_free_context(&mut m);
            }
            return Err(DecodeError::Runtime(format!(
                "parameters_to_context: {}",
                sys::err_str(rc)
            )));
        }
        Ok(ctx)
    };

    let want_hw = chosen != BackendKind::Cpu && hw.is_some();
    let cfgs = if want_hw {
        hw_configs(codec, chosen)
    } else {
        Vec::new()
    };
    let static_unsupported = want_hw && cfgs.is_empty();
    if static_unsupported {
        // §4.4: statically GPU-unsupported input -> CPU decode permitted, record reason
        let ctx = alloc()?;
        let rc = unsafe { sys::avcodec_open2(ctx, codec, std::ptr::null_mut()) };
        if rc < 0 {
            unsafe {
                let mut m = ctx;
                sys::avcodec_free_context(&mut m);
            }
            return Err(DecodeError::Unsupported(format!(
                "CPU decoder open failed for {}: {}",
                codec_name(unsafe { (*par).codec_id }),
                sys::err_str(rc)
            )));
        }
        let reason = format!(
            "no {} hwaccel config for codec {} profile {}",
            backend_name(chosen),
            codec_name(unsafe { (*par).codec_id }),
            unsafe { (*par).profile }
        );
        return Ok((ctx, None, BackendKind::Cpu, true, Some(reason)));
    }

    if want_hw {
        let hwd = hw.unwrap();
        let ctx = alloc()?;
        let mut state = Box::new(HwState {
            allowed: cfgs.clone(),
            rejected: false,
        });
        let state_ptr: *mut HwState = &mut *state as *mut HwState;
        unsafe {
            (*ctx).hw_device_ctx = sys::av_buffer_ref(hwd.buffer);
            (*ctx).opaque = state_ptr as *mut c_void;
            (*ctx).get_format = Some(get_format_cb);
        }
        let rc = unsafe { sys::avcodec_open2(ctx, codec, std::ptr::null_mut()) };
        let rejected = state.rejected;
        if rc == 0 && !rejected {
            return Ok((ctx, Some(state), chosen, false, None));
        }
        unsafe {
            (*ctx).opaque = std::ptr::null_mut();
            let mut m = ctx;
            sys::avcodec_free_context(&mut m);
        }
        // decoder-open failure for a GPU-supported input in a GPU-present
        // environment: initialization failure -> exit 5, no CPU switch
        // (§4.3 step 3 / §4.4; REQ-DC-002)
        return Err(DecodeError::Runtime(format!(
            "GPU decoder open for {} failed: {} (CPU auto-switch forbidden, §4.4)",
            codec_name(unsafe { (*par).codec_id }),
            if rejected {
                "decoder offered no hw pixel format".to_string()
            } else {
                sys::err_str(rc)
            }
        )));
    }

    // CPU path: --backend cpu, or auto with all GPU candidates absent (§4.3 step 2)
    let ctx = alloc()?;
    let rc = unsafe { sys::avcodec_open2(ctx, codec, std::ptr::null_mut()) };
    if rc < 0 {
        unsafe {
            let mut m = ctx;
            sys::avcodec_free_context(&mut m);
        }
        return Err(DecodeError::Unsupported(format!(
            "CPU decoder open failed for {}: {}",
            codec_name(unsafe { (*par).codec_id }),
            sys::err_str(rc)
        )));
    }
    Ok((ctx, None, BackendKind::Cpu, false, None))
}

fn backend_name(b: BackendKind) -> &'static str {
    match b {
        BackendKind::Nvdec => "nvdec",
        BackendKind::Vaapi => "vaapi",
        BackendKind::D3d11va => "d3d11va",
        BackendKind::Cpu => "cpu",
    }
}

/// hw pix fmts for the chosen backend type (§4.4 static determination).
fn hw_configs(codec: *const sys::AVCodec, chosen: BackendKind) -> Vec<i32> {
    let want: sys::AVHWDeviceType = match chosen {
        BackendKind::Nvdec => sys::AVHWDeviceType_AV_HWDEVICE_TYPE_CUDA,
        BackendKind::Vaapi => sys::AVHWDeviceType_AV_HWDEVICE_TYPE_VAAPI,
        BackendKind::D3d11va => sys::AVHWDeviceType_AV_HWDEVICE_TYPE_D3D11VA,
        BackendKind::Cpu => return Vec::new(),
    };
    let mut out = Vec::new();
    unsafe {
        for i in 0..64i32 {
            let cfg = sys::avcodec_get_hw_config(codec, i);
            if cfg.is_null() {
                break;
            }
            if (*cfg).device_type == want {
                out.push((*cfg).pix_fmt);
            }
        }
    }
    out
}

fn build_info(
    fmt: *mut sys::AVFormatContext,
    video_index: i32,
    audio_index: Option<i32>,
    vdec: *mut sys::AVCodecContext,
    vreason: &str,
    areason: &str,
) -> Result<StreamInfo, DecodeError> {
    let vst = stream_at(fmt, video_index as usize);
    let par = unsafe { (*vst).codecpar };
    let (fps_num, fps_den) = nominal_fps(vst)
        .ok_or_else(|| DecodeError::Unsupported("indeterminate frame rate".to_string()))?;
    let bit_depth = unsafe {
        let desc = sys::av_pix_fmt_desc_get((*vdec).pix_fmt);
        if desc.is_null() {
            8
        } else {
            (*desc).comp[0].depth.max(8) as u32
        }
    };
    // §5.1 scan premise; §9.2 single value from the first segment
    let interlaced = unsafe {
        matches!(
            (*vdec).field_order,
            sys::AVFieldOrder_AV_FIELD_TT
                | sys::AVFieldOrder_AV_FIELD_BB
                | sys::AVFieldOrder_AV_FIELD_TB
                | sys::AVFieldOrder_AV_FIELD_BT
        )
    };
    Ok(StreamInfo {
        video_stream_index: video_index as i64,
        audio_stream_index: audio_index.map(|x| x as i64),
        width: unsafe { (*par).width } as i64,
        height: unsafe { (*par).height } as i64,
        video_codec: codec_name(unsafe { (*par).codec_id }),
        interlaced,
        bit_depth,
        fps_num,
        fps_den,
        video_selection_reason: vreason.to_string(),
        audio_selection_reason: areason.to_string(),
    })
}

impl Drop for DecoderEngine {
    fn drop(&mut self) {
        unsafe {
            if !self.vdec.is_null() {
                (*self.vdec).opaque = std::ptr::null_mut();
                (*self.vdec).get_format = None;
                sys::avcodec_free_context(&mut self.vdec);
            }
            if let Some(a) = self.adec.as_mut() {
                if !a.is_null() {
                    sys::avcodec_free_context(a);
                }
            }
            if !self.fmt.is_null() {
                (*self.fmt).pb = std::ptr::null_mut();
                sys::avformat_close_input(&mut self.fmt);
            }
            if !self.iobuf.is_null() {
                // internal seeks (probe/seekback) may have reallocated the
                // buffer; free the current one exactly once
                sys::av_free((*self.iobuf).buffer as *mut c_void);
                sys::avio_context_free(&mut self.iobuf);
            }
            // io_state Box was reconstructed from io_ptr at end of open(); the
            // raw pointer's owner is the engine itself, so drop via the Option.
            if let Some(b) = self.io_state.take() {
                drop(b);
            }
        }
    }
}
