//! CLI logo detection over the engine's shared native-resolution luma.

use std::path::PathBuf;

use susuki_core::logo_calc::logo_calc_frame;
use susuki_core::logo_find::{logo_find, FrameStore};
use susuki_core::logo_mul::{merge_logos, ResultOut};
use susuki_core::logo_set::{logo_setup, Param, Thres};
use susuki_decode::luma::Luma;
use susuki_logo_file::{parse_logo, LogoFile};

use crate::cli::LogoConfig;
use crate::output::{LogoEntry, LogoIntervals};

struct Candidate {
    path: PathBuf,
    file: LogoFile,
    param: Param,
    thres: Thres,
    store: FrameStore,
}

pub struct LogoRunner {
    candidates: Vec<Candidate>,
    thres: Thres,
    warnings: Vec<&'static str>,
}

pub struct LogoResults {
    pub merged: ResultOut,
    pub entries: Vec<LogoEntry>,
    pub warnings: Vec<&'static str>,
}

impl LogoRunner {
    pub fn new(config: &LogoConfig) -> Result<Self, (String, u8)> {
        let mut thres = Thres::default();
        for (name, value) in &config.params {
            let n = i16::try_from(*value)
                .map_err(|_| (format!("logo parameter {name} is outside s16 range"), 2u8))?;
            match name.as_str() {
                "fadein" => {
                    thres.num_fadein = n;
                    thres.auto_fade = 0;
                }
                "fadeout" => {
                    thres.num_fadeout = n;
                    thres.auto_fade = 0;
                }
                "mrgleft" => thres.num_cutleft = n,
                "mrgright" => thres.num_cutright = n,
                "onwidth" => thres.num_onwidth = n,
                "offwidth" => thres.num_offwidth = n,
                "onlevel" => thres.num_onlevel = n,
                "yedge" => {
                    thres.thres_yedge = n;
                    thres.auto_bs11 = 0;
                }
                "yoffedg" => thres.thres_yoffedg = n,
                "ymax" => thres.thres_ymax = n,
                "ymin" => thres.thres_ymin = n,
                "ydif" => thres.thres_ydif = n,
                "ysetdif" => thres.thres_ysetdif = n,
                "areaset" => thres.num_areaset = n,
                "clrrate" => thres.num_clrrate = n,
                _ => return Err((format!("unknown logo parameter {name}"), 2)),
            }
        }
        let mut candidates = Vec::new();
        for path in &config.paths {
            if path
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("lgd2"))
            {
                return Err(("LogoFormatUnsupported".into(), 2));
            }
            let bytes = std::fs::read(path).map_err(|e| {
                (
                    format!("cannot read logo file {}: {e}", path.display()),
                    3u8,
                )
            })?;
            let file = parse_logo(&bytes).map_err(|e| (e.0.to_string(), 2u8))?;
            let mut local = thres.clone();
            let param = logo_setup(&file, &mut local);
            candidates.push(Candidate {
                path: path.clone(),
                file,
                param,
                thres: local,
                store: FrameStore::new(0),
            });
        }
        Ok(Self {
            candidates,
            thres,
            warnings: Vec::new(),
        })
    }

    fn warn(&mut self, code: &'static str) {
        if !self.warnings.contains(&code) {
            eprintln!("WARN\t{code}\tlogo skipped for incompatible frame geometry");
            self.warnings.push(code);
        }
    }

    pub fn push_frame(&mut self, luma: &Luma) {
        let mut mismatch = false;
        let mut outside = false;
        for candidate in &mut self.candidates {
            let index = candidate.store.push_frame();
            let f = &candidate.file;
            if f.image_width.is_some_and(|w| w as usize != luma.width)
                || f.image_height.is_some_and(|h| h as usize != luma.height)
            {
                mismatch = true;
                continue;
            }
            if (f.x + f.width) as usize > luma.width || (f.y + f.height) as usize > luma.height {
                outside = true;
                continue;
            }
            logo_calc_frame(
                &candidate.param,
                &candidate.thres,
                &mut candidate.store,
                &luma.data,
                luma.width,
                index,
            );
        }
        if mismatch {
            self.warn("logo_resolution_mismatch");
        }
        if outside {
            self.warn("logo_out_of_frame");
        }
    }

    pub fn finish(mut self) -> LogoResults {
        for c in &mut self.candidates {
            logo_find(&mut c.store, &c.thres);
        }
        let mut stores: Vec<FrameStore> = self
            .candidates
            .iter_mut()
            .map(|c| std::mem::replace(&mut c.store, FrameStore::new(0)))
            .collect();
        let (merged, used, _) = merge_logos(&mut stores, &self.thres);
        let entries = self
            .candidates
            .into_iter()
            .zip(stores)
            .zip(used)
            .map(|((c, store), used)| LogoEntry {
                path: c.path.to_string_lossy().into_owned(),
                used,
                intervals: store.res[..store.num_find as usize]
                    .iter()
                    .map(|r| LogoIntervals {
                        start: r.frm_rise,
                        // REF LogoFind fall is inclusive; JSON uses half-open ranges.
                        end: r.frm_fall + 1,
                        fade_in: i64::from(r.fade_rise),
                        fade_out: i64::from(r.fade_fall),
                        interlace_start: intl_name(r.intl_rise),
                        interlace_end: intl_name(r.intl_fall),
                        uncertain_left: r.frm_rise - r.frm_rise_l,
                        uncertain_right: r.frm_rise_r - r.frm_rise,
                    })
                    .collect(),
            })
            .collect();
        LogoResults {
            merged,
            entries,
            warnings: self.warnings,
        }
    }
}

fn intl_name(v: i8) -> &'static str {
    match v {
        1 => "T",
        2 => "B",
        _ => "ALL",
    }
}
