//! §3 CLI contract: option parsing and validation.
//! Long options only. Duplicate non-repeatable options are errors (no last-wins).
//! Paths are handled as OS bytes (§3.3); non-path values must be UTF-8.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Backend {
    #[default]
    Auto,
    Nvdec,
    Vaapi,
    D3d11va,
    Cpu,
}

/// `--backend` names this platform's build offers (Unix: NVDEC/VAAPI,
/// Windows: D3D11VA).
#[cfg(unix)]
const BACKEND_NAMES: &str = "auto|nvdec|vaapi|cpu";
#[cfg(windows)]
const BACKEND_NAMES: &str = "auto|d3d11va|cpu";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnalysisBackend {
    #[default]
    Auto,
    Cpu,
    Gpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdoutFormat {
    Json,
    Chapter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamSel {
    /// `v:N` / `a:N` — 0-based ordinal among that media type
    Ordinal(u32),
    /// bare integer — absolute stream index in the container
    AbsIndex(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioStreamSel {
    Sel(StreamSel),
    None,
}

pub const LOGO_PARAM_NAMES: [&str; 15] = [
    "fadein", "fadeout", "mrgleft", "mrgright", "onwidth", "onlevel", "offwidth", "yedge",
    "yoffedg", "ymax", "ymin", "ydif", "ysetdif", "areaset", "clrrate",
];

#[derive(Debug, Default, Clone)]
pub struct DecodeConfig {
    pub backend: Backend,
    pub device: Option<String>,
    pub analysis_backend: AnalysisBackend,
    pub video_stream: Option<StreamSel>,
    pub audio_stream: Option<AudioStreamSel>,
    pub tlv_service: Option<i64>,
}

#[derive(Debug, Default, Clone)]
pub struct SceneConfig {
    pub mute_threshold: i64,
    pub mute_min_frames: i64,
    pub mute_search_gap: i64,
    pub mute_extend: i64,
}

#[derive(Debug, Default, Clone)]
pub struct LogoConfig {
    /// CLI order preserved; logo number (1-based) is index+1 (§3.2.1)
    pub paths: Vec<PathBuf>,
    pub disabled: bool,
    pub params: Vec<(String, i64)>,
}

#[derive(Debug, Default, Clone)]
pub struct JudgeConfig {
    pub script: Option<PathBuf>,
    pub jls_sets: Vec<(String, String)>,
    pub jls_flags: Vec<String>,
    // raw tokens: converted on the REF setOptionsCnvCutMrg path (core::timeaxis)
    pub cut_margin_in: Option<String>,
    pub cut_margin_out: Option<String>,
    pub cut_margin_wide_in: Option<i64>,
    pub cut_margin_wide_out: Option<i64>,
    // §5.2 scopes the 30fps-based conversion to the -CutMrgIn family only.
    // -lastcut is nominal-fps frames (REF-JLS JlsIF.cpp:523-525 uses getMsecAdjustFrmFromMsec).
    pub lastcut: i64,
}

#[derive(Debug, Default, Clone)]
pub struct OutputConfig {
    pub json: Option<PathBuf>,
    pub chapter: Option<PathBuf>,
    pub div: Option<PathBuf>,
    pub stdout: Option<StdoutFormat>,
    pub overwrite: bool,
}

#[derive(Debug, Default, Clone)]
pub struct Options {
    pub input: PathBuf,
    pub decode: DecodeConfig,
    pub scene: SceneConfig,
    pub logo: LogoConfig,
    pub judge: JudgeConfig,
    pub output: OutputConfig,
}

#[derive(Debug)]
pub struct ArgError(pub String);

#[derive(Debug)]
pub enum Cmd {
    Run(Box<Options>),
    Help,
    Version,
}

fn default_scene() -> SceneConfig {
    // §3.2.3 defaults
    SceneConfig {
        mute_threshold: 50,
        mute_min_frames: 10,
        mute_search_gap: 60,
        mute_extend: 1,
    }
}

struct Parser {
    opts: Options,
    scene: SceneConfig,
    seen: Vec<String>,
    help: bool,
    version: bool,
    positional: Vec<PathBuf>,
    logo_disabled_given: bool,
}

impl Parser {
    fn mark_once(&mut self, name: &str) -> Result<(), ArgError> {
        if self.seen.iter().any(|s| s == name) {
            return Err(ArgError(format!(
                "duplicate option: {name} (repeated specification is not allowed)"
            )));
        }
        self.seen.push(name.to_string());
        Ok(())
    }
}

fn os_to_str(v: &OsStr) -> Result<String, ArgError> {
    v.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| ArgError("value is not valid UTF-8 text".to_string()))
}

/// §3.3 relative paths are absolutized against cwd at startup
fn absolutize(cwd: &Path, p: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    }
}

fn take_value(
    flag: &str,
    inline: Option<String>,
    it: &mut std::vec::IntoIter<OsString>,
) -> Result<OsString, ArgError> {
    if let Some(v) = inline {
        return Ok(OsString::from(v));
    }
    it.next()
        .ok_or_else(|| ArgError(format!("missing value for {flag}")))
}

fn parse_int(v: &OsStr, name: &str) -> Result<i64, ArgError> {
    let s = os_to_str(v)?;
    s.parse::<i64>()
        .map_err(|_| ArgError(format!("option {name}: expected an integer, got {s:?}")))
}

fn parse_float(v: &OsStr, name: &str) -> Result<f64, ArgError> {
    let s = os_to_str(v)?;
    let f: f64 = s
        .parse()
        .map_err(|_| ArgError(format!("option {name}: expected a number, got {s:?}")))?;
    if f.is_finite() {
        Ok(f)
    } else {
        Err(ArgError(format!(
            "option {name}: number must be finite, got {s:?}"
        )))
    }
}

fn parse_stream_sel(v: &OsStr, name: &str, prefix: &str) -> Result<StreamSel, ArgError> {
    let s = os_to_str(v)?;
    if let Some(rest) = s.strip_prefix(prefix) {
        let n = rest
            .parse::<u32>()
            .map_err(|_| ArgError(format!("option {name}: invalid spec {s:?}")))?;
        Ok(StreamSel::Ordinal(n))
    } else {
        let n = s.parse::<i64>().map_err(|_| {
            ArgError(format!(
                "option {name}: expected {prefix}N or an integer, got {s:?}"
            ))
        })?;
        if n < 0 {
            return Err(ArgError(format!(
                "option {name}: negative absolute index {n} is invalid"
            )));
        }
        Ok(StreamSel::AbsIndex(n as i32))
    }
}

fn split_kv<'a>(s: &'a str, name: &str) -> Result<(&'a str, &'a str), ArgError> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| ArgError(format!("option {name}: expected NAME=VALUE, got {s:?}")))?;
    if k.is_empty() {
        return Err(ArgError(format!("option {name}: empty NAME")));
    }
    Ok((k, v))
}

/// Split `--name=value` into (`--name`, Some(`value`))
fn split_arg(a: &OsStr) -> (String, Option<String>) {
    let s = a.to_string_lossy().to_string();
    if s.starts_with("--") {
        if let Some((name, val)) = s.split_once('=') {
            return (name.to_string(), Some(val.to_string()));
        }
    }
    (s, None)
}

pub fn parse_args<I>(args: I, cwd: &Path) -> Result<Cmd, ArgError>
where
    I: IntoIterator<Item = OsString>,
{
    let mut p = Parser {
        opts: Options::default(),
        scene: default_scene(),
        seen: Vec::new(),
        help: false,
        version: false,
        positional: Vec::new(),
        logo_disabled_given: false,
    };

    let mut v: Vec<OsString> = args.into_iter().collect();
    if !v.is_empty() {
        v.remove(0); // argv[0] is the program path
    }
    let mut it = v.into_iter();
    while let Some(arg) = it.next() {
        let (flagname, inline) = split_arg(&arg);
        let flag = flagname.as_str();
        match flag {
            "--help" => p.help = true,
            "--version" => p.version = true,
            "--logo" => {
                let val = take_value(flag, inline, &mut it)?;
                p.opts.logo.paths.push(absolutize(cwd, Path::new(&val)));
            }
            "--no-logo" => {
                if inline.is_some() {
                    return Err(ArgError("--no-logo takes no value".to_string()));
                }
                p.mark_once("--no-logo")?;
                p.logo_disabled_given = true;
            }
            "--script" => {
                p.mark_once("--script")?;
                let val = take_value(flag, inline, &mut it)?;
                p.opts.judge.script = Some(absolutize(cwd, Path::new(&val)));
            }
            "--video-stream" => {
                p.mark_once("--video-stream")?;
                let val = take_value(flag, inline, &mut it)?;
                p.opts.decode.video_stream = Some(parse_stream_sel(&val, "--video-stream", "v:")?);
            }
            "--audio-stream" => {
                p.mark_once("--audio-stream")?;
                let val = take_value(flag, inline, &mut it)?;
                p.opts.decode.audio_stream = Some(if val == OsStr::new("none") {
                    AudioStreamSel::None
                } else {
                    AudioStreamSel::Sel(parse_stream_sel(&val, "--audio-stream", "a:")?)
                });
            }
            "--tlv-service" => {
                p.mark_once("--tlv-service")?;
                let val = take_value(flag, inline, &mut it)?;
                p.opts.decode.tlv_service = Some(parse_int(&val, "--tlv-service")?);
            }
            "--backend" => {
                p.mark_once("--backend")?;
                let val = os_to_str(&take_value(flag, inline, &mut it)?)?;
                p.opts.decode.backend = match val.as_str() {
                    "auto" => Backend::Auto,
                    "nvdec" => Backend::Nvdec,
                    "vaapi" => Backend::Vaapi,
                    "d3d11va" => Backend::D3d11va,
                    "cpu" => Backend::Cpu,
                    _ => {
                        return Err(ArgError(format!(
                            "--backend: expected {BACKEND_NAMES}, got {val:?}"
                        )))
                    }
                };
                let platform_ok = match p.opts.decode.backend {
                    Backend::Auto | Backend::Cpu => true,
                    Backend::Nvdec | Backend::Vaapi => cfg!(unix),
                    Backend::D3d11va => cfg!(windows),
                };
                if !platform_ok {
                    return Err(ArgError(format!(
                        "--backend: {val} is not available on this platform (expected {BACKEND_NAMES})"
                    )));
                }
            }
            "--device" => {
                p.mark_once("--device")?;
                let val = os_to_str(&take_value(flag, inline, &mut it)?)?;
                p.opts.decode.device = Some(val);
            }
            "--analysis-backend" => {
                p.mark_once("--analysis-backend")?;
                let val = os_to_str(&take_value(flag, inline, &mut it)?)?;
                p.opts.decode.analysis_backend = match val.as_str() {
                    "auto" => AnalysisBackend::Auto,
                    "cpu" => AnalysisBackend::Cpu,
                    "gpu" => AnalysisBackend::Gpu,
                    _ => {
                        return Err(ArgError(format!(
                            "--analysis-backend: expected auto|cpu|gpu, got {val:?}"
                        )))
                    }
                };
            }
            "--mute-threshold" => {
                p.mark_once("--mute-threshold")?;
                let val = take_value(flag, inline, &mut it)?;
                let n = parse_int(&val, "--mute-threshold")?;
                if !(1..=32767).contains(&n) {
                    return Err(ArgError(
                        "--mute-threshold: out of range 1..32767".to_string(),
                    ));
                }
                p.scene.mute_threshold = n;
            }
            "--mute-min-frames" => {
                p.mark_once("--mute-min-frames")?;
                let val = take_value(flag, inline, &mut it)?;
                let n = parse_int(&val, "--mute-min-frames")?;
                if n < 1 {
                    return Err(ArgError("--mute-min-frames: must be >= 1".to_string()));
                }
                p.scene.mute_min_frames = n;
            }
            "--mute-search-gap" => {
                p.mark_once("--mute-search-gap")?;
                let val = take_value(flag, inline, &mut it)?;
                let n = parse_int(&val, "--mute-search-gap")?;
                if n < 1 {
                    return Err(ArgError("--mute-search-gap: must be >= 1".to_string()));
                }
                p.scene.mute_search_gap = n;
            }
            "--mute-extend" => {
                p.mark_once("--mute-extend")?;
                let val = take_value(flag, inline, &mut it)?;
                let n = parse_int(&val, "--mute-extend")?;
                if n < 0 {
                    return Err(ArgError("--mute-extend: must be >= 0".to_string()));
                }
                p.scene.mute_extend = n;
            }
            "--logo-param" => {
                let val = os_to_str(&take_value(flag, inline, &mut it)?)?;
                let (name, num) = split_kv(&val, "--logo-param")?;
                if !LOGO_PARAM_NAMES.contains(&name) {
                    return Err(ArgError(format!("--logo-param: unknown NAME {name:?}")));
                }
                let n = parse_int(OsStr::new(num), "--logo-param")?;
                p.opts.logo.params.push((name.to_string(), n));
            }
            "--jls-set" => {
                let val = os_to_str(&take_value(flag, inline, &mut it)?)?;
                let (name, v2) = split_kv(&val, "--jls-set")?;
                p.opts
                    .judge
                    .jls_sets
                    .push((name.to_string(), v2.to_string()));
            }
            "--jls-flags" => {
                p.mark_once("--jls-flags")?;
                let val = os_to_str(&take_value(flag, inline, &mut it)?)?;
                p.opts.judge.jls_flags = val.split(',').map(|s| s.trim().to_string()).collect();
            }
            "--cut-margin-in" => {
                p.mark_once("--cut-margin-in")?;
                let val = take_value(flag, inline, &mut it)?;
                parse_float(&val, "--cut-margin-in")?;
                p.opts.judge.cut_margin_in = Some(os_to_str(&val)?);
            }
            "--cut-margin-out" => {
                p.mark_once("--cut-margin-out")?;
                let val = take_value(flag, inline, &mut it)?;
                parse_float(&val, "--cut-margin-out")?;
                p.opts.judge.cut_margin_out = Some(os_to_str(&val)?);
            }
            "--cut-margin-wide-in" => {
                p.mark_once("--cut-margin-wide-in")?;
                let val = take_value(flag, inline, &mut it)?;
                p.opts.judge.cut_margin_wide_in = Some(parse_int(&val, "--cut-margin-wide-in")?);
            }
            "--cut-margin-wide-out" => {
                p.mark_once("--cut-margin-wide-out")?;
                let val = take_value(flag, inline, &mut it)?;
                p.opts.judge.cut_margin_wide_out = Some(parse_int(&val, "--cut-margin-wide-out")?);
            }
            "--lastcut" => {
                p.mark_once("--lastcut")?;
                let val = take_value(flag, inline, &mut it)?;
                let n = parse_int(&val, "--lastcut")?;
                if n < 0 {
                    return Err(ArgError("--lastcut: must be >= 0".to_string()));
                }
                p.opts.judge.lastcut = n;
            }
            "--output-json" => {
                p.mark_once("--output-json")?;
                let val = take_value(flag, inline, &mut it)?;
                p.opts.output.json = Some(absolutize(cwd, Path::new(&val)));
            }
            "--output-chapter" => {
                p.mark_once("--output-chapter")?;
                let val = take_value(flag, inline, &mut it)?;
                p.opts.output.chapter = Some(absolutize(cwd, Path::new(&val)));
            }
            "--output-div" => {
                p.mark_once("--output-div")?;
                let val = take_value(flag, inline, &mut it)?;
                p.opts.output.div = Some(absolutize(cwd, Path::new(&val)));
            }
            "--output-stdout" => {
                p.mark_once("--output-stdout")?;
                let val = os_to_str(&take_value(flag, inline, &mut it)?)?;
                p.opts.output.stdout = Some(match val.as_str() {
                    "json" => StdoutFormat::Json,
                    "chapter" => StdoutFormat::Chapter,
                    _ => {
                        return Err(ArgError(format!(
                            "--output-stdout: expected json|chapter, got {val:?}"
                        )))
                    }
                });
            }
            "--overwrite" => {
                if inline.is_some() {
                    return Err(ArgError("--overwrite takes no value".to_string()));
                }
                p.mark_once("--overwrite")?;
                p.opts.output.overwrite = true;
            }
            _ => {
                if flag.starts_with("--") {
                    return Err(ArgError(format!("unknown option: {flag}")));
                }
                if flag.starts_with('-') && flag != "-" {
                    return Err(ArgError(format!(
                        "short options are not provided (use long form): {flag}"
                    )));
                }
                p.positional.push(absolutize(cwd, Path::new(&arg)));
            }
        }
    }

    if p.help {
        return Ok(Cmd::Help);
    }
    if p.version {
        return Ok(Cmd::Version);
    }

    match p.positional.len() {
        1 => p.opts.input = p.positional.remove(0),
        n => {
            return Err(ArgError(format!(
                "expected exactly one INPUT file, got {n}"
            )))
        }
    }

    p.opts.logo.disabled = p.logo_disabled_given;
    p.opts.scene = p.scene;
    validate(&p.opts)?;
    Ok(Cmd::Run(Box::new(p.opts)))
}

/// §3.6 code-2 validation: combinations, required options, output same-file (§3.2.6)
fn validate(o: &Options) -> Result<(), ArgError> {
    if o.logo.disabled && !o.logo.paths.is_empty() {
        return Err(ArgError(
            "--no-logo and --logo cannot be combined".to_string(),
        ));
    }
    if !o.logo.disabled && o.logo.paths.is_empty() {
        return Err(ArgError(
            "either --logo (one or more) or --no-logo is required".to_string(),
        ));
    }
    if o.decode.device.is_some() && o.decode.backend == Backend::Cpu {
        return Err(ArgError(
            "--device cannot be used with --backend cpu".to_string(),
        ));
    }
    if o.output.json.is_none() && o.output.chapter.is_none() && o.output.stdout.is_none() {
        return Err(ArgError(
            "at least one of --output-json/--output-chapter/--output-stdout is required"
                .to_string(),
        ));
    }
    let mut out_paths: Vec<(&'static str, PathBuf)> = Vec::new();
    if let Some(x) = &o.output.json {
        out_paths.push(("--output-json", canonical_like(x)));
    }
    if let Some(x) = &o.output.chapter {
        out_paths.push(("--output-chapter", canonical_like(x)));
    }
    if let Some(x) = &o.output.div {
        out_paths.push(("--output-div", canonical_like(x)));
    }
    for i in 0..out_paths.len() {
        for j in i + 1..out_paths.len() {
            if out_paths[i].1 == out_paths[j].1 {
                return Err(ArgError(format!(
                    "output paths {} and {} point to the same file",
                    out_paths[i].0, out_paths[j].0
                )));
            }
        }
    }
    Ok(())
}

/// Resolve existing paths; otherwise canonicalize the parent and keep the file name (§3.2.6)
fn canonical_like(p: &Path) -> PathBuf {
    if let Ok(r) = p.canonicalize() {
        return r;
    }
    if let Some(parent) = p.parent().filter(|x| !x.as_os_str().is_empty()) {
        if let Ok(rp) = parent.canonicalize() {
            return rp.join(p.file_name().unwrap_or_default());
        }
    }
    p.to_path_buf()
}
