//! Pipeline wiring (design 7.2): decode -> scene/mute analysis -> scp rows
//! -> dataset -> JLS execution -> trim generation. JSON/chapter output (§9)
//! and shared native-resolution logo detection feed the one JLS execution.

use std::path::{Path, PathBuf};

extern "C" {
    fn signal(sig: i32, handler: usize) -> usize;
    #[cfg(unix)]
    fn poll(fds: *mut PollFd, nfds: u64, timeout: i32) -> i32;
}
#[cfg(unix)]
#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}
#[cfg(unix)]
const POLLOUT: i16 = 0x004;
/// §4.6/§8.5: the one cancel flag shared by the signal handler, decode IO,
/// the JLS loop and output generation. Set before the handlers are installed,
/// so the handler only performs an atomic load and store.
static INTERRUPT: std::sync::OnceLock<Arc<AtomicBool>> = std::sync::OnceLock::new();
unsafe extern "C" fn on_signal(_sig: i32) {
    if let Some(f) = INTERRUPT.get() {
        f.store(true, Ordering::Relaxed);
    }
}
fn interrupt_flag() -> Arc<AtomicBool> {
    INTERRUPT
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone()
}
fn interrupted() -> bool {
    INTERRUPT
        .get()
        .map(|f| f.load(Ordering::Relaxed))
        .unwrap_or(false)
}
fn install_signal_hooks() {
    let _ = interrupt_flag();
    unsafe {
        signal(2 /*SIGINT*/, on_signal as *const () as usize);
        // Windows has no SIGTERM delivery; its CRT maps Ctrl+C to SIGINT and
        // resets the handler after one delivery, so a second Ctrl+C kills.
        #[cfg(unix)]
        signal(15 /*SIGTERM*/, on_signal as *const () as usize);
    }
}
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use susuki_core::jls::cmdarg::CmdArg;
use susuki_core::jls::cmdlimit::CmdLimit;
use susuki_core::jls::cnvstr::Cnv;
use susuki_core::jls::dataset::Dataset;
use susuki_core::jls::exec::{AutoCtx, Exec, ExecHost};
use susuki_core::jls::forbid::scan_forbidden;
use susuki_core::jls::gstate::GState;
use susuki_core::jls::input::{logos_from_records, scpos_from_records, ScpRow};
use susuki_core::jls::vars::Vars;
use susuki_core::mvec::mvec_pair;
use susuki_core::scene::{MuteDetector, SceneState};
use susuki_core::timeaxis::TimeConverter;

use crate::cli::{AnalysisBackend, AudioStreamSel, Backend, Options, StreamSel};
use susuki_decode::device::{select_auto, BackendKind, Probe};
use susuki_decode::engine::{DecoderEngine, FrameEvent, InputSpec};
use susuki_decode::luma::{downscale, scale_factor, Luma};

struct Runner {
    feats: Vec<Option<susuki_core::mvec::PairFeat>>,
    prev: Option<Arc<Luma>>,
    lx: usize,
    ly: usize,
    pict_struct: i32,
    /// §6.5 Vulkan compute path (None = CPU); failures fall back per-call
    /// and record `analysis_gpu_fallback` (never an error).
    gpu: Option<susuki_core::gpu::GpuEngine>,
    offs: Vec<u32>,
    gpu_fallback: bool,
}

impl Runner {
    fn push_frame(&mut self, idx: usize, small: &Arc<Luma>) {
        // feats[0] stays None: pair_feat mirrors the reference frame-0
        // self-comparison; later frames pair (x-1, x) as §6.2.2.
        let prev = self.prev.take();
        let feat = match prev {
            Some(p) => {
                let mut pre: Option<Vec<susuki_core::gpu::BlockStat>> = None;
                if self.gpu.is_some() {
                    let lx2 = (self.lx as i32 * self.pict_struct) as u32;
                    let bh = (16 / self.pict_struct) as u32;
                    match self.gpu.as_ref().unwrap().block_stats(
                        &small.data,
                        &p.data,
                        &self.offs,
                        lx2,
                        bh,
                    ) {
                        Some(t) => pre = Some(t),
                        None => {
                            self.gpu_fallback = true;
                            self.gpu = None;
                        }
                    }
                }
                Some(match pre {
                    Some(t) => susuki_core::mvec::mvec_pair_opt(
                        &small.data,
                        &p.data,
                        self.lx,
                        self.ly,
                        5000,
                        self.pict_struct,
                        idx as i32,
                        Some(&t),
                    ),
                    None => mvec_pair(
                        &small.data,
                        &p.data,
                        self.lx,
                        self.ly,
                        5000, // (100-0)*(100/FIELD_PICTURE), §6.2.2 both modes
                        self.pict_struct,
                        idx as i32,
                    ),
                })
            }
            None => None,
        };
        self.feats.push(feat);
        self.prev = Some(small.clone());
    }
}

/// ExecHost backed by CLI policy (§3.5: Echo goes to stderr as ECHO lines;
/// file-writing script commands are statically forbidden, §7.5).
struct BinHost {
    cnv: Rc<std::cell::RefCell<Cnv>>,
}

impl ExecHost for BinHost {
    fn stderr_line(&mut self, s: &[u8]) {
        let mut out = String::from_utf8_lossy(s).into_owned();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        eprint!("{}", out);
    }
    fn stdout_line(&mut self, s: &[u8]) {
        // The exec trace/echo channel is reserved: stdout carries only
        // --output-stdout results (§3.5), so route to stderr ECHO lines.
        let text = String::from_utf8_lossy(s).trim_end().to_string();
        eprintln!("ECHO\t{}", text);
    }
    fn read_script_lines(&mut self, path: &[u8]) -> Option<Vec<Vec<u8>>> {
        if path == EMBEDDED_STD_NAME {
            let mut lines: Vec<Vec<u8>> = STD_SCRIPT_BYTES
                .split(|&c| c == b'\n')
                .map(|l| l.to_vec())
                .collect();
            if lines.last().map(|l| l.is_empty()).unwrap_or(false) {
                lines.pop();
            }
            return Some(lines);
        }
        let p = Path::new(std::str::from_utf8(path).ok()?);
        let data = std::fs::read(p).ok()?;
        let mut lines: Vec<Vec<u8>> = data.split(|&c| c == b'\n').map(|l| l.to_vec()).collect();
        if lines.last().map(|l| l.is_empty()).unwrap_or(false) {
            lines.pop();
        }
        Some(lines)
    }
    fn file_exists(&mut self, path: &[u8]) -> bool {
        match std::str::from_utf8(path) {
            Ok(p) => std::fs::metadata(Path::new(p))
                .map(|m| m.is_file())
                .unwrap_or(false),
            Err(_) => false,
        }
    }
    fn env_var(&mut self, _name: &[u8]) -> Option<Vec<u8>> {
        None
    }
    fn file_open(&mut self, _name: &[u8], _append: bool) -> bool {
        false
    }
    fn file_close(&mut self) {}
    fn file_output(&mut self, s: &[u8]) {
        for line in s.split(|&c| c == b'\n') {
            if line.is_empty() {
                continue;
            }
            eprintln!("ECHO\t{}", String::from_utf8_lossy(line));
        }
    }
    fn reform_adjust(&mut self, ctx: &mut AutoCtx) {
        susuki_core::jls::reform_data::adjust_data(ctx.ds, &self.cnv.borrow());
    }
    fn auto_script(
        &mut self,
        ctx: &mut AutoCtx,
        arg: &mut CmdArg,
        lim: &mut CmdLimit,
        setup_only: bool,
    ) -> bool {
        susuki_core::jls::autoexec::start_cmd(ctx.ds, &self.cnv.borrow(), arg, lim, setup_only)
    }
    fn display_logo_scp(&mut self) {}
    fn interrupted(&mut self) -> bool {
        interrupted()
    }
}

/// §9.5: cancel before output generation starts -> no outputs, exit 130
fn interrupted_in_analysis() -> ExitCode {
    eprintln!("ERROR\tinterrupt\tinterrupted during analysis");
    ExitCode::from(130)
}

pub fn run(opts: &Options) -> ExitCode {
    install_signal_hooks();
    match run_inner(opts) {
        Ok(code) => code,
        Err((msg, code)) => {
            eprintln!("ERROR\tpipeline\t{}", msg);
            ExitCode::from(code)
        }
    }
}

fn spec_from_cli(opts: &Options) -> InputSpec {
    let mut spec = InputSpec {
        video_ordinal: None,
        video_abs: None,
        audio_ordinal: None,
        audio_abs: None,
        audio_none: false,
        tlv_service: opts.decode.tlv_service,
    };
    match opts.decode.video_stream {
        Some(StreamSel::Ordinal(n)) => spec.video_ordinal = Some(n),
        Some(StreamSel::AbsIndex(i)) => spec.video_abs = Some(i),
        None => {}
    }
    match &opts.decode.audio_stream {
        Some(AudioStreamSel::None) => spec.audio_none = true,
        Some(AudioStreamSel::Sel(StreamSel::Ordinal(n))) => spec.audio_ordinal = Some(*n),
        Some(AudioStreamSel::Sel(StreamSel::AbsIndex(i))) => spec.audio_abs = Some(*i),
        None => {}
    }
    spec
}

fn ms_of(cnv: &Cnv, f: i32) -> i32 {
    cnv.tc.get_msec_from_frm(f as i64) as i32
}

fn scan_script_forbidden(script: &Path, fps_num: i64, fps_den: i64) -> bool {
    let Ok(text) = std::fs::read(script) else {
        return false;
    };
    let tc = match TimeConverter::new(fps_num, fps_den) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let cnv = Cnv::new(tc, 0);
    let dir = script.parent().unwrap_or(Path::new(".")).to_path_buf();
    let entry: Vec<u8> = script.to_string_lossy().as_bytes().to_vec();
    let mut read_call = |cur: &[u8], arg: &[u8]| -> Option<(&'static [u8], Vec<u8>)> {
        let _ = cur;
        let p = dir.join(std::str::from_utf8(arg).ok()?);
        let body = std::fs::read(&p).ok()?;
        let canon: &'static [u8] = Box::leak(
            p.to_string_lossy()
                .into_owned()
                .into_bytes()
                .into_boxed_slice(),
        );
        Some((canon, body))
    };
    match scan_forbidden(&cnv, &entry, &text, &mut read_call) {
        Some((path, line, cmd)) => {
            eprintln!(
                "ERROR\tjudge\tforbidden command {} at {}:{}",
                String::from_utf8_lossy(cmd),
                String::from_utf8_lossy(&path),
                line
            );
            true
        }
        None => false,
    }
}

/// §7.5: script source; None means the byte-verified embedded standard script
pub enum ScriptSrc {
    Path(PathBuf),
    Embedded,
}

/// pseudo-name used to fetch the embedded script through ExecHost::read_script_lines
const EMBEDDED_STD_NAME: &[u8] = b"@EMBEDDED_STD";
const STD_SCRIPT_BYTES: &[u8] = include_bytes!("../resources/JL_標準.txt");

fn run_inner(opts: &Options) -> Result<ExitCode, (String, u8)> {
    let mut logos = crate::logo::LogoRunner::new(&opts.logo)?;
    // ---- script resolution + static forbid check (§7.5) ----
    let script: ScriptSrc = match &opts.judge.script {
        Some(p) => {
            let pb = p.clone();
            if !pb.is_file() {
                eprintln!("ERROR\tjudge\tcannot open script");
                return Ok(ExitCode::from(3));
            }
            ScriptSrc::Path(pb)
        }
        None => ScriptSrc::Embedded,
    };

    // ---- decode ----
    // §4.2/§4.3: explicit nvdec/vaapi/d3d11va uses the reference selection; device
    // absence or init failure exits 5 either way (§3.6), with no CPU fallback.
    let kind = match opts.decode.backend {
        Backend::Auto => {
            let probe = Probe::new();
            select_auto(&probe, opts.decode.device.as_deref())
                .map_err(|e| (format!("device selection failed: {:?}", e), e.exit_code()))?
                .0
        }
        Backend::Cpu => BackendKind::Cpu,
        Backend::Nvdec => BackendKind::Nvdec,
        Backend::Vaapi => BackendKind::Vaapi,
        Backend::D3d11va => BackendKind::D3d11va,
    };
    let interrupt = interrupt_flag();
    let mut engine = match DecoderEngine::open(
        &opts.input,
        &spec_from_cli(opts),
        Some(kind),
        opts.decode.device.as_deref(),
        interrupt.clone(),
    ) {
        Ok(e) => e,
        Err(susuki_decode::error::DecodeError::Interrupted) => return Ok(interrupted_in_analysis()),
        Err(e) => return Err((format!("decode open failed: {:?}", e), e.exit_code())),
    };

    let info = engine.info.clone();
    if info.width <= 0 || info.height <= 0 {
        return Err(("no usable video stream".to_string(), 4u8));
    }
    // §7.5 forbid scan needs the input framerate context only for tokens;
    // scan with a nominal converter (pure text scan uses getStrItem only).
    match &script {
        // §7.5: the embedded script is hash-verified at build time and
        // contains no forbidden commands, so only --script files are scanned.
        ScriptSrc::Path(pb) => {
            if scan_script_forbidden(pb, info.fps_num, info.fps_den) {
                return Ok(ExitCode::from(6));
            }
        }
        ScriptSrc::Embedded => {}
    }

    let s = scale_factor(info.width as usize);
    // REF-CHAPTER always passes FIELD_PICTURE, including progressive input (N6).
    let pict_struct = 2;
    // §6.5/REQ-DC-006: auto/gpu try the Vulkan compute path; failure or
    // explicit cpu keeps CPU analysis (analysis-side fallback always allowed).
    let mut gpu_engine = match opts.decode.analysis_backend {
        AnalysisBackend::Cpu => None,
        AnalysisBackend::Auto | AnalysisBackend::Gpu => susuki_core::gpu::GpuEngine::init(),
    };
    // 明示 gpu で利用不能はフォールバック発生（§6.5 warnings）、auto の
    // GPU 不在は通常選択（CPU 決定論）なので警告を立てない。
    let gpu_init_fallback =
        matches!(opts.decode.analysis_backend, AnalysisBackend::Gpu) && gpu_engine.is_none();
    if gpu_init_fallback {
        gpu_engine = None;
    }
    let mut runner = Runner {
        feats: Vec::new(),
        prev: None,
        lx: 0,
        ly: 0,
        pict_struct,
        gpu: gpu_engine,
        offs: Vec::new(),
        gpu_fallback: gpu_init_fallback,
    };
    // Audio is reduced and consumed at its native rate; only the current
    // window peak survives, so PCM never accumulates for the whole program.
    let mut mutedet = MuteDetector::new(
        info.fps_num,
        info.fps_den,
        opts.scene.mute_threshold as i32,
        opts.scene.mute_min_frames as i32,
    );
    let mut total_frames: i64 = 0;
    let mut last_progress = std::time::Instant::now();

    loop {
        if interrupted() {
            return Ok(interrupted_in_analysis());
        }
        let ev = match engine.next_event() {
            Ok(ev) => ev,
            Err(susuki_decode::error::DecodeError::Interrupted) => {
                return Ok(interrupted_in_analysis())
            }
            Err(e) => return Err((format!("decode failed: {:?}", e), e.exit_code())),
        };
        let Some(ev) = ev else { break };
        match ev {
            FrameEvent::Video(f) => {
                logos.push_frame(&f.luma);
                let small = Arc::new(downscale(&f.luma, s));
                if runner.lx != small.width || runner.ly != small.height {
                    // Frame pairs require the same geometry; a resolution
                    // change starts a new pair context without resetting time.
                    runner.prev = None;
                    runner.lx = small.width;
                    runner.ly = small.height;
                    runner.offs.clear();
                }
                if runner.offs.is_empty() {
                    runner.offs =
                        susuki_core::mvec::block_offsets(runner.lx, runner.ly, runner.pict_struct);
                }
                runner.push_frame(f.index as usize, &small);
                total_frames = f.index + 1;
                if last_progress.elapsed().as_secs() >= 1 {
                    eprintln!("PROGRESS\tscan\t{}\t0", total_frames);
                    last_progress = std::time::Instant::now();
                }
            }
            FrameEvent::Audio(c) => mutedet
                .push(&c.peaks, c.sample_rate)
                .map_err(|e| (e.to_string(), 5u8))?,
        }
    }
    if total_frames <= 0 {
        return Err(("no video frames decoded".to_string(), 5u8));
    }
    let lx = runner.lx;
    let ly = runner.ly;
    let feats = std::mem::take(&mut runner.feats);
    let _ = (lx, ly);

    // ---- scene / mute (§6.2) ----
    let mut scene = SceneState::new();
    let intervals = mutedet.finish(total_frames as i32);
    for iv in &intervals {
        scene.process_segment(
            &feats,
            total_frames as i32,
            iv.start_fr,
            iv.len,
            opts.scene.mute_min_frames as i32,
            opts.scene.mute_search_gap as i32,
            opts.scene.mute_extend as i32,
        );
    }
    if intervals.is_empty() {
        eprintln!("WARN\tjudge\tno mute intervals found");
    }
    scene.append_tail(total_frames as i32);

    let logo_results = logos.finish();
    // ---- dataset + JLS entry ----
    let tc = TimeConverter::new(info.fps_num, info.fps_den)
        .map_err(|e| (format!("bad framerate: {:?}", e), 4u8))?;
    let _ = tc;
    let cnv = Rc::new(std::cell::RefCell::new(Cnv::new(tc, 0)));
    let g = GState::new();
    let mut vars = Vars::new(cnv.clone());
    vars.msg = g.msg.clone();
    vars.stop_err = g.stop_err.clone();
    let mut ds = Dataset::new();
    ds.ext_opt.frm_lastcut = opts.judge.lastcut as i32;

    // The merge entry inserts head 0; scene.records includes the n-1 tail.
    let rows: Vec<ScpRow> = scene
        .records
        .iter()
        .map(|r| ScpRow {
            msec: ms_of(&cnv.borrow(), r.pos_sc),
            msbk: ms_of(&cnv.borrow(), r.pos_bk),
            smute_s: if r.mute_start_fr < 0 {
                -1
            } else {
                ms_of(&cnv.borrow(), r.mute_start_fr)
            },
            smute_w: if r.seri < 0 {
                -1
            } else {
                ms_of(&cnv.borrow(), r.seri)
            },
            still: r.still,
        })
        .collect();
    scpos_from_records(&cnv.borrow(), &mut ds, &rows);

    // ---- logos (LogoFind + REF -oasel 0 merge; original candidate results stay separate) ----
    logos_from_records(
        &cnv.borrow(),
        &mut ds,
        &logo_results.merged.res[..logo_results.merged.num_find as usize],
    );
    if opts.logo.disabled || ds.empty_logo() {
        ds.ext_opt.flag_no_logo = 1;
    }

    // ---- exec ----
    let mut ex = Exec {
        g,
        ds,
        vars,
        host: BinHost { cnv: cnv.clone() },
        frames: Vec::new(),
        script_error: false,
    };
    // §3.7: external settings are applied before script execution
    {
        let mut toks: Vec<Vec<u8>> = Vec::new();
        toks.push(b"-flags".to_vec());
        toks.push(opts.judge.jls_flags.join(",").into_bytes());
        for (name, val) in &opts.judge.jls_sets {
            toks.push(b"-set".to_vec());
            toks.push(name.clone().into_bytes());
            toks.push(val.clone().into_bytes());
        }
        if let Some(v) = &opts.judge.cut_margin_in {
            toks.push(b"-cutmrgin".to_vec());
            toks.push(v.clone().into_bytes());
        }
        if let Some(v) = &opts.judge.cut_margin_out {
            toks.push(b"-cutmrgout".to_vec());
            toks.push(v.clone().into_bytes());
        }
        if let Some(v) = opts.judge.cut_margin_wide_in {
            toks.push(b"-cutmrgwi".to_vec());
            toks.push(v.to_string().into_bytes());
        }
        if let Some(v) = opts.judge.cut_margin_wide_out {
            toks.push(b"-cutmrgwo".to_vec());
            toks.push(v.to_string().into_bytes());
        }
        let mut i = 0usize;
        while i < toks.len() {
            let rest = (toks.len() - i) as i32;
            let s1 = toks.get(i + 1).map(|v| v.as_slice());
            let s2 = toks.get(i + 2).map(|v| v.as_slice());
            let n = ex.set_options_get_one(rest, &toks[i], s1, s2, true);
            i += if n > 0 { n as usize } else { 1 };
        }
    }
    let errnum = match &script {
        ScriptSrc::Path(pb) => ex.start_cmd(pb.to_string_lossy().as_bytes()),
        ScriptSrc::Embedded => ex.start_cmd(EMBEDDED_STD_NAME),
    };
    if errnum == 2 {
        eprintln!("ERROR\tjudge\tscript open failed");
        return Ok(ExitCode::from(3));
    }
    // flush accumulated script errors (Echo/checkErrorGlobalState paths)
    if let Some(msg) = ex.g.pop_msg_error() {
        let text = String::from_utf8_lossy(&msg).into_owned();
        for line in text.lines() {
            eprintln!("ECHO\t{}", line);
        }
    }
    if interrupted() {
        return Ok(interrupted_in_analysis());
    }
    // §3.6/§7.3: syntax, unsupported command, nesting and Call failures are
    // judge errors; the analysis fails before any output is generated
    if ex.script_error {
        eprintln!("ERROR\tjudge\tscript error");
        return Ok(ExitCode::from(6));
    }

    // ---- trim generation (§7.2 step 4) + §9 output ----
    ex.ds.output_result_trim_gen(&cnv.borrow());
    eprintln!("PROGRESS\tjudge\t{}\t{}", total_frames, total_frames);
    if interrupted() {
        return Ok(interrupted_in_analysis());
    }
    let decode_meta = DecodeMeta {
        backend: engine.backend,
        gpu_unsupported: engine.gpu_unsupported,
        fallback_reason: engine.fallback_reason.clone(),
    };
    let code = write_outputs(
        opts,
        &ex,
        &cnv.borrow(),
        &info,
        &intervals,
        &scene,
        total_frames,
        &decode_meta,
        &script,
        logo_results,
        runner.gpu_fallback,
    );
    Ok(ExitCode::from(code))
}

#[allow(clippy::too_many_arguments)]
fn write_outputs(
    opts: &Options,
    ex: &Exec<BinHost>,
    cnv: &Cnv,
    info: &susuki_decode::engine::StreamInfo,
    intervals: &[susuki_core::scene::MuteInterval],
    scene: &SceneState,
    total_frames: i64,
    decode: &DecodeMeta,
    script: &ScriptSrc,
    logo_results: crate::logo::LogoResults,
    gpu_fallback: bool,
) -> u8 {
    use crate::output::{self as out, OutputData};

    let mut warnings = logo_results.warnings;
    if gpu_fallback {
        // §6.5: 特徴量の GPU 計算に失敗し CPU 経路へフォールバックした
        warnings.push("analysis_gpu_fallback");
    }
    if info.audio_stream_index.is_none() {
        warnings.push("no_audio");
    }
    if logo_results.merged.num_find == 0 && !opts.logo.disabled {
        warnings.push("no_logo_detected");
    }

    let scene_changes: Vec<(i64, bool)> = {
        let mut v: Vec<(i64, bool)> = Vec::new();
        for r in &scene.records {
            // ScRecord keeps §6.2.3 frame numbers
            let frame = r.pos_sc as i64;
            if !v.iter().any(|(x, _)| *x == frame) {
                v.push((frame, r.still));
            }
        }
        v
    };

    let d = OutputData {
        ds: &ex.ds,
        cnv,
        total_frames,
        input_path: opts.input.to_string_lossy().into_owned(),
        width: info.width,
        height: info.height,
        video_codec: info.video_codec.clone(),
        interlaced: info.interlaced,
        bit_depth: info.bit_depth as i64,
        video_stream_index: info.video_stream_index,
        audio_stream_index: info.audio_stream_index,
        tlv_service: opts.decode.tlv_service,
        backend: match decode.backend {
            BackendKind::Cpu => "cpu",
            BackendKind::Nvdec => "nvdec",
            BackendKind::Vaapi => "vaapi",
            BackendKind::D3d11va => "d3d11va",
        },
        device: opts.decode.device.clone(),
        gpu_unsupported: decode.gpu_unsupported,
        fallback_reason: decode.fallback_reason.clone(),
        script_name: match script {
            ScriptSrc::Path(pb) => pb.to_string_lossy().into_owned(),
            ScriptSrc::Embedded => "standard".to_string(),
        },
        mute_intervals: intervals
            .iter()
            .map(|iv| (iv.start_fr as i64, (iv.start_fr + iv.len) as i64))
            .collect(),
        scene_changes,
        logos: logo_results.entries,
        warnings,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    let keep = out::normalize_keep(cnv, &ex.ds.result_trim, total_frames);
    let segs = out::collect_segments(&d);
    let json = out::build_json(&d, &keep, &segs);
    let chapter = out::build_chapter(&d, &keep);
    let div = out::build_div(&d);
    let ow = opts.output.overwrite;

    // §9.5: an existing output without --overwrite is a code-2 failure
    // detected before the first write, so nothing is committed
    let file_targets: [(&Option<std::path::PathBuf>, Vec<u8>); 3] = [
        (&opts.output.json, json.clone().into_bytes()),
        (&opts.output.chapter, chapter),
        (&opts.output.div, div),
    ];
    for (path, _) in &file_targets {
        if let Some(p) = path {
            if let Err(msg) = out::check_not_existing(p, ow) {
                eprintln!("ERROR\toutput\t{}", msg);
                return 2;
            }
        }
    }

    // §9.5: files first, stdout last; a cancel stops further commits and the
    // exit code depends on whether any output was already committed
    let mut rc: u8 = 0;
    let mut committed = 0usize;
    let cancel = |committed: usize| -> u8 {
        eprintln!("ERROR\tinterrupt\tinterrupted during output");
        if committed > 0 {
            3
        } else {
            130
        }
    };
    for (path, bytes) in file_targets {
        let Some(p) = path else { continue };
        if interrupted() {
            return cancel(committed);
        }
        match out::write_atomic(p, &bytes, ow, &interrupted) {
            Ok(()) => committed += 1,
            Err(out::WriteError::Interrupted) => return cancel(committed),
            Err(out::WriteError::Failed(msg, code)) => {
                eprintln!("ERROR\toutput\t{}", msg);
                rc = rc.max(code);
            }
        }
    }
    if let Some(fmt) = &opts.output.stdout {
        if interrupted() {
            return cancel(committed);
        }
        let bytes: Vec<u8> = match fmt {
            crate::cli::StdoutFormat::Json => json.into_bytes(),
            crate::cli::StdoutFormat::Chapter => out::build_chapter(&d, &keep),
        };
        match write_stdout(&bytes) {
            Ok(()) => committed += 1,
            // §9.5: the stdout output is cut off; files are already committed
            Err(None) => return cancel(committed),
            Err(Some(e)) => {
                eprintln!("ERROR\toutput\tstdout write failed: {}", e);
                rc = rc.max(3);
            }
        }
    }
    if interrupted() {
        return cancel(committed);
    }
    rc
}

/// §9.5 stdout write that stays cancellable: the signal may be delivered to
/// any thread (decoder threads included), so instead of relying on EINTR the
/// loop waits for POLLOUT in short slices, checks the cancel flag, and writes
/// at most PIPE_BUF bytes, which a pipe reporting POLLOUT accepts without
/// blocking. Err(None) = cancelled; output is cut off at that point.
#[cfg(unix)]
fn write_stdout(bytes: &[u8]) -> Result<(), Option<std::io::Error>> {
    use std::io::Write;
    use std::os::fd::FromRawFd;
    const PIPE_BUF: usize = 4096;
    // fd 1 is owned by the process; ManuallyDrop keeps it open
    let mut out = std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(1) });
    let mut off = 0usize;
    while off < bytes.len() {
        if interrupted() {
            return Err(None);
        }
        let mut p = PollFd {
            fd: 1,
            events: POLLOUT,
            revents: 0,
        };
        let r = unsafe { poll(&mut p, 1, 100) };
        if r < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(Some(e));
        }
        if r == 0 {
            continue;
        }
        let end = (off + PIPE_BUF).min(bytes.len());
        match out.write(&bytes[off..end]) {
            Ok(0) => return Err(Some(std::io::ErrorKind::WriteZero.into())),
            Ok(n) => off += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(Some(e)),
        }
    }
    Ok(())
}

/// §9.5 stdout write on Windows: pipes cannot be polled for writability, so
/// the cancel flag is checked between PIPE_BUF-sized writes only. A write
/// blocked on a reader that stopped draining the pipe is not cancellable.
/// Err(None) = cancelled; output is cut off at that point.
#[cfg(windows)]
fn write_stdout(bytes: &[u8]) -> Result<(), Option<std::io::Error>> {
    use std::io::Write;
    const PIPE_BUF: usize = 4096;
    let mut out = std::io::stdout().lock();
    for chunk in bytes.chunks(PIPE_BUF) {
        if interrupted() {
            return Err(None);
        }
        out.write_all(chunk).map_err(Some)?;
    }
    out.flush().map_err(Some)
}

/// decode result as actually opened by the engine (§4.4 static CPU fallback)
struct DecodeMeta {
    backend: BackendKind,
    gpu_unsupported: bool,
    fallback_reason: Option<String>,
}
