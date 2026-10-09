//! §9 output generation: JSON / .chapter.txt / div with atomic replacement
//! (§9.5). All values derive from the one settled result (dataset after trim
//! generation + detail walk).

use std::path::Path;

use susuki_core::jls::cnvstr::Cnv;
use susuki_core::jls::dataset::Dataset;
use susuki_core::jls::dataset_output::DetailRec;
use susuki_core::jls::dataset_search::*;
use susuki_core::jls::tables::ConfigVarType;

pub struct LogoIntervals {
    pub start: i64,
    pub end: i64,
    pub fade_in: i64,
    pub fade_out: i64,
    pub interlace_start: &'static str,
    pub interlace_end: &'static str,
    pub uncertain_left: i64,
    pub uncertain_right: i64,
}

pub struct LogoEntry {
    pub path: String,
    pub intervals: Vec<LogoIntervals>,
    pub used: bool,
}

pub struct OutputData<'a> {
    pub ds: &'a Dataset,
    pub cnv: &'a Cnv,
    pub total_frames: i64,
    pub input_path: String,
    pub width: i64,
    pub height: i64,
    pub video_codec: String,
    pub interlaced: bool,
    pub bit_depth: i64,
    pub video_stream_index: i64,
    pub audio_stream_index: Option<i64>,
    pub tlv_service: Option<i64>,
    pub backend: &'static str,
    pub device: Option<String>,
    pub gpu_unsupported: bool,
    pub fallback_reason: Option<String>,
    pub script_name: String,
    pub mute_intervals: Vec<(i64, i64)>,
    pub scene_changes: Vec<(i64, bool)>,
    pub logos: Vec<LogoEntry>,
    pub warnings: Vec<&'static str>,
    pub version: String,
}

pub fn frm_of(cnv: &Cnv, ms: i32) -> i64 {
    cnv.tc.get_frm_from_msec(ms as i64)
}

pub fn sec_of(cnv: &Cnv, f: i64) -> f64 {
    cnv.tc.get_msec_from_frm(f) as f64 / 1000.0
}

fn fmt_sec(v: f64) -> String {
    format!("{:.3}", (v * 1000.0).round() / 1000.0)
}

/// §7.1/§9.2: frame-space keep ranges, ascending, disjoint, adjacent-merged
pub fn normalize_keep(cnv: &Cnv, trim_ms: &[i32], total: i64) -> Vec<(i64, i64)> {
    let mut pairs: Vec<(i64, i64)> = Vec::new();
    let mut i = 0;
    while i + 1 < trim_ms.len() {
        let mut s = frm_of(cnv, trim_ms[i]);
        let mut e = frm_of(cnv, trim_ms[i + 1]);
        if s < 0 {
            s = 0;
        }
        if e > total {
            e = total;
        }
        if s < e {
            pairs.push((s, e));
        }
        i += 2;
    }
    pairs.sort_by_key(|p| p.0);
    let mut out: Vec<(i64, i64)> = Vec::new();
    for (s, e) in pairs {
        match out.last_mut() {
            Some(last) if s <= last.1 => {
                if e > last.1 {
                    last.1 = e;
                }
            }
            _ => out.push((s, e)),
        }
    }
    out
}

fn kind_label(arstat: i32) -> &'static str {
    match arstat {
        SCP_AR_N_UNIT | SCP_AR_N_AUNIT | SCP_AR_N_BUNIT => "cm",
        SCP_AR_N_OTHER => "nologo",
        SCP_AR_B_UNIT => "border15s",
        SCP_AR_B_OTHER => "border",
        SCP_AR_L_UNIT | SCP_AR_L_OTHER => "logo",
        SCP_AR_L_MIXED => "mix",
        _ => "unknown",
    }
}

fn note_label(ds: &Dataset, arext: i32) -> Option<&'static str> {
    let cut_tr = ds.get_config(ConfigVarType::flagCutTR) == 1;
    let cut_sp = ds.get_config(ConfigVarType::flagCutSP) == 1;
    Some(match arext {
        AREXT_L_TRKEEP => "trailer-add",
        AREXT_L_ECCUT => "trailer-cut",
        AREXT_L_TRRAW => "trailer",
        AREXT_L_TRCUT => {
            if cut_tr {
                "trailer-cut"
            } else {
                "trailer-cut-cancel"
            }
        }
        AREXT_L_SP => {
            if cut_sp {
                "sponsor-cut"
            } else {
                "sponsor-add"
            }
        }
        AREXT_L_EC => "endcard-add",
        AREXT_L_LGCUT => "l-edge-cut",
        AREXT_L_LGADD => "l-edge-add",
        AREXT_N_TRCUT => "nologo-cut",
        AREXT_N_LGCUT => "n-edge-cut",
        AREXT_N_LGADD => "n-edge-add",
        _ => return None,
    })
}

/// same walk as §7.2 detail generation (one record per config boundary)
pub fn collect_segments(d: &OutputData) -> Vec<DetailRec> {
    let mut out = Vec::new();
    let mut ds = d.ds.clone();
    ds.output_result_detail_reset();
    loop {
        let (eof, rec) = ds.output_result_detail_get_line(d.cnv);
        if eof {
            break;
        }
        out.push(rec);
        if out.len() > 100_000 {
            break; // resource bound guard (§8.4)
        }
    }
    out
}

fn join_json(items: Vec<String>) -> String {
    items.join(", ")
}

fn opt_json<T: std::fmt::Display>(v: Option<T>) -> String {
    match v {
        Some(x) => x.to_string(),
        None => "null".to_string(),
    }
}

/// JSON string body escaping (RFC 8259 §7): `"`, `\` and U+0000-U+001F
fn json_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

pub fn build_json(d: &OutputData, keep: &[(i64, i64)], segs: &[DetailRec]) -> String {
    let mut j = String::new();
    j.push_str("{\n  \"schema_version\": 1,\n  \"result\": {\n    \"keep_ranges\": [");
    j.push_str(&join_json(
        keep.iter()
            .map(|(s, e)| {
                format!(
                    "{{\"start_frame\": {}, \"end_frame\": {}, \"start_sec\": {}, \"end_sec\": {}}}",
                    s,
                    e,
                    fmt_sec(sec_of(d.cnv, *s)),
                    fmt_sec(sec_of(d.cnv, *e))
                )
            })
            .collect(),
    ));
    j.push_str("    ],\n    \"segments\": [");
    j.push_str(&join_json(
        segs.iter()
            .map(|r| {
                format!(
                    "{{\"start_frame\": {}, \"end_frame\": {}, \"start_sec\": {}, \"end_sec\": {}, \"duration_sec\": {}, \"logo_sec\": {}, \"kind\": \"{}\", \"note\": {}}}",
                    r.frm_from,
                    r.frm_to,
                    fmt_sec(sec_of(d.cnv, r.frm_from as i64)),
                    fmt_sec(sec_of(d.cnv, r.frm_to as i64)),
                    r.sec_dif,
                    r.sec_logo,
                    kind_label(r.arstat),
                    match note_label(d.ds, r.arext) {
                        Some(n) => format!("\"{}\"", json_escape(n)),
                        None => "null".to_string(),
                    }
                )
            })
            .collect(),
    ));
    j.push_str("    ]\n  },\n  \"input\": {\n");
    j.push_str(&format!(
        "    \"path\": \"{}\",\n    \"duration_sec\": {},\n    \"total_frames\": {},\n    \"fps_num\": {}, \"fps_den\": {},\n    \"width\": {}, \"height\": {},\n    \"video_codec\": \"{}\",\n    \"interlaced\": {},\n    \"bit_depth\": {},\n    \"video_stream_index\": {},\n    \"audio_stream_index\": {},\n    \"tlv_service\": {}\n  }},\n",
        json_escape(&d.input_path),
        fmt_sec(sec_of(d.cnv, d.total_frames)),
        d.total_frames,
        d.cnv.tc.frate_n,
        d.cnv.tc.frate_d,
        d.width,
        d.height,
        json_escape(&d.video_codec),
        d.interlaced,
        d.bit_depth,
        d.video_stream_index,
        opt_json(d.audio_stream_index),
        opt_json(d.tlv_service)
    ));
    j.push_str(&format!(
        "  \"decode\": {{\n    \"backend\": \"{}\",\n    \"device\": {},\n    \"gpu_unsupported\": {},\n    \"fallback_reason\": {}\n  }},\n",
        d.backend,
        match &d.device {
            Some(v) => format!("\"{}\"", json_escape(v)),
            None => "null".to_string(),
        },
        d.gpu_unsupported,
        match &d.fallback_reason {
            Some(v) => format!("\"{}\"", json_escape(v)),
            None => "null".to_string(),
        }
    ));
    j.push_str("  \"logos\": [");
    j.push_str(&join_json(
        d.logos
            .iter()
            .map(|le| {
                let mut iv = String::new();
                iv.push_str(&format!("{{\"path\": \"{}\", \"intervals\": [", json_escape(&le.path)));
                iv.push_str(&join_json(
                    le.intervals
                        .iter()
                        .map(|v| {
                            format!(
                                "{{\"start_frame\": {}, \"end_frame\": {}, \"fade_in\": {}, \"fade_out\": {}, \"interlace_start\": \"{}\", \"interlace_end\": \"{}\", \"uncertain_left\": {}, \"uncertain_right\": {}}}",
                                v.start,
                                v.end,
                                v.fade_in,
                                v.fade_out,
                                v.interlace_start,
                                v.interlace_end,
                                v.uncertain_left,
                                v.uncertain_right
                            )
                        })
                        .collect(),
                ));
                iv.push_str(&format!("], \"used\": {}}}", le.used));
                iv
            })
            .collect(),
    ));
    j.push_str("  ],\n  \"analysis\": {\n");
    j.push_str(&format!(
        "    \"script\": \"{}\",\n",
        json_escape(&d.script_name)
    ));
    j.push_str("    \"mute_intervals\": [");
    j.push_str(&join_json(
        d.mute_intervals
            .iter()
            .map(|(s, e)| format!("{{\"start_frame\": {}, \"end_frame\": {}}}", s, e))
            .collect(),
    ));
    j.push_str("    ],\n    \"scene_changes\": [");
    j.push_str(&join_json(
        d.scene_changes
            .iter()
            .map(|(f, st)| format!("{{\"frame\": {}, \"still\": {}}}", f, st))
            .collect(),
    ));
    j.push_str("    ]\n  },\n  \"warnings\": [");
    j.push_str(&join_json(
        d.warnings.iter().map(|w| format!("\"{}\"", w)).collect(),
    ));
    j.push_str(&format!(
        "    ],\n  \"tool\": {{\"name\": \"susuki\", \"version\": \"{}\"}}\n}}\n",
        json_escape(&d.version)
    ));
    j
}

/// §9.3 chapter bytes (CP932 + CRLF)
pub fn build_chapter(d: &OutputData, keep: &[(i64, i64)]) -> Vec<u8> {
    let mut pts: Vec<(i64, &'static str)> = Vec::new();
    let in_keep = |f: i64| keep.iter().any(|(s, e)| f >= *s && f < *e);
    if keep.is_empty() {
        pts.push((0, "CM区間"));
    } else if keep.len() == 1 && keep[0].0 == 0 && keep[0].1 >= d.total_frames {
        pts.push((0, "本編"));
    } else {
        pts.push((0, if in_keep(0) { "本編" } else { "CM区間" }));
        for (s, e) in keep {
            if *s > 0 {
                pts.push((*s, "本編"));
            }
            if *e < d.total_frames {
                pts.push((*e, "CM区間"));
            }
        }
    }
    pts.sort_by_key(|p| p.0);
    let mut out = Vec::new();
    for (i, (f, title)) in pts.iter().enumerate() {
        let ms = d.cnv.tc.get_msec_from_frm(*f);
        let h = ms / 3600000;
        let m = (ms - h * 3600000) / 60000;
        let rest = ms - h * 3600000 - m * 60000;
        let s = rest / 1000;
        let t = rest - s * 1000;
        out.extend_from_slice(
            format!(
                "CHAPTER{:02}={:02}:{:02}:{:02}.{:03}\r\n",
                i + 1,
                h,
                m,
                s,
                t
            )
            .as_bytes(),
        );
        out.extend_from_slice(format!("CHAPTER{:02}NAME=", i + 1).as_bytes());
        out.extend_from_slice(&cp932(title));
        out.extend_from_slice(b"\r\n");
    }
    out
}

fn cp932(s: &str) -> Vec<u8> {
    let (cow, _, _) = encoding_rs::SHIFT_JIS.encode(s);
    cow.into_owned()
}

pub fn build_div(d: &OutputData) -> Vec<u8> {
    let mut out = Vec::new();
    for ms in &d.ds.div_file {
        out.extend_from_slice(format!("{}\n", frm_of(d.cnv, *ms)).as_bytes());
    }
    out
}

/// §9.5/§3.6: an existing output without --overwrite is a code-2 error
pub fn check_not_existing(path: &Path, overwrite: bool) -> Result<(), String> {
    if !overwrite && path.exists() {
        return Err(format!("output exists (use --overwrite): {:?}", path));
    }
    Ok(())
}

pub enum WriteError {
    /// §9.5 cancel during output: the temporary file was removed, nothing renamed
    Interrupted,
    Failed(String, u8),
}

/// §9.5: tmp+fsync+rename; refuse existing without --overwrite
pub fn write_atomic(
    path: &Path,
    bytes: &[u8],
    overwrite: bool,
    interrupted: &dyn Fn() -> bool,
) -> Result<(), WriteError> {
    check_not_existing(path, overwrite).map_err(|m| WriteError::Failed(m, 2))?;
    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out".to_string());
    let tmp = path.with_file_name(format!("{}.tmp-{}", file_name, std::process::id()));
    let r = (|| -> std::io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        Ok(())
    })();
    if let Err(e) = r {
        let _ = std::fs::remove_file(&tmp);
        return Err(WriteError::Failed(format!("tmp write: {}", e), 3));
    }
    if interrupted() {
        let _ = std::fs::remove_file(&tmp);
        return Err(WriteError::Interrupted);
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(WriteError::Failed(format!("rename: {}", e), 3));
    }
    Ok(())
}
