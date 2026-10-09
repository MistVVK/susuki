//! Derived from chapter_exe (https://github.com/rigaya/chapter_exe,
//! commit 32880d45f088e574285a101e6a49b032bb04f6ea), Copyright (C) rigaya,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Design 6.2.1-6.2.3: mute detection + proc_scene_change port
//! (REF-CHAPTER chapter_exe.cpp fixed commit 32880d45).
//!
//! Streaming adaptation per design 8.3: frame-pair features
//! (`crate::mvec::PairFeat`) are computed when frames arrive and kept for the
//! whole program; a segment is processed retrospectively from that array,
//! exactly reproducing the reference control flow (reference reads pixels at
//! segment time, which yields the same numbers).

use crate::mvec::PairFeat;

/// §6.2.1: one mute interval (start frame, consecutive muted frame count).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MuteInterval {
    pub start_fr: i32,
    pub len: i32,
}

/// Fixed-16 kHz batch comparator for the existing streaming harness, not the
/// native-rate pipeline entry (which uses `MuteDetector`).
/// frame f window = [floor(f*16000*frate_d/frate_n), floor((f+1)*16000*frate_d/frate_n)).
/// A frame with no samples is NOT mute (2026-10-02 rule; does not extend runs).
/// Loop bounds mirror the reference: i in 0..(n - setseri - 1); a still-open
/// run at loop exit is dropped, as in chapter_exe.
pub fn detect_mute_intervals(
    samples: &[i16],
    total_frames: i32,
    frate_n: i64,
    frate_d: i64,
    threshold: i32,
    setseri: i32,
) -> Vec<MuteInterval> {
    let mut out = Vec::new();
    let a = 16000i64 * frate_d; // sample numerator per frame
    let b = frate_n; // denominator
    let mut seri = 0i32;
    let last_i = total_frames - setseri - 1;
    let mut i = 0i32;
    while i < last_i {
        let s0 = ((i as i64 * a) / b) as usize;
        let s1 = (((i as i64 + 1) * a) / b) as usize;
        let nomute = if s1 <= s0 || s1 > samples.len() {
            true // no samples in window -> non-mute (never extend)
        } else {
            let end = s1.min(samples.len());
            samples[s0..end]
                .iter()
                .any(|&v| i32::from(v.unsigned_abs()) > threshold)
        };
        if nomute {
            if seri >= setseri {
                out.push(MuteInterval {
                    start_fr: i - seri,
                    len: seri,
                });
            }
            seri = 0;
        } else {
            seri += 1;
        }
        i += 1;
    }
    out
}

/// One SC candidate as internal §6.2.3 record (frame numbers; ms conversion
/// happens at the JLS entry per 5.2/6.2.3).
#[derive(Debug, Clone)]
pub struct ScRecord {
    /// chapter position (mute interval start frame, reference write_chapter arg)
    pub chapter_frame: i32,
    /// mute interval length in frames (title "Nフレーム")
    pub seri: i32,
    /// d_maxrev_pos (SC position) -> JLS "msec"
    pub pos_sc: i32,
    /// d_maxpre_pos (position before change) -> JLS "msbk"
    pub pos_bk: i32,
    /// mute interval start/end frames -> JLS msmute_s / msmute_e
    pub mute_start_fr: i32,
    pub mute_end_fr: i32,
    /// true when the title carries the "＿" (still) mark
    pub still: bool,
    /// reference mark string (""/＿/○/★../＠) recorded for traceability
    pub mark: &'static str,
    /// scene-change judgement value (debug titles)
    pub rate_sc: i32,
}

const DEF_SCMAX: usize = 100;
const SPACE_SC1: i32 = 5;
const SPACE_SC2: i32 = 10;
const SPACE_SC3: i32 = 30;
const THRES_RATE1: i32 = 300;
const THRES_RATE2: i32 = 7;

/// Persistent state across mute intervals (reference lastmute_scpos /
/// lastmute_marker / idx).
#[derive(Debug)]
pub struct SceneState {
    last_scpos: i32,
    last_marker: i32,
    idx: i32,
    pub records: Vec<ScRecord>,
}

impl SceneState {
    pub fn new() -> Self {
        SceneState {
            last_scpos: -1,
            last_marker: -1,
            idx: 1,
            records: Vec::new(),
        }
    }

    /// REF chapter_exe appends `# SCPos:n-1 n-1` even without mute records.
    /// readScpos resets mute attributes after each SCPos, so the tail has none.
    pub fn append_tail(&mut self, total_frames: i32) {
        if total_frames <= 0 {
            return;
        }
        self.records.push(ScRecord {
            chapter_frame: total_frames - 1,
            seri: -1,
            pos_sc: total_frames - 1,
            pos_bk: total_frames - 1,
            mute_start_fr: -1,
            mute_end_fr: -1,
            still: false,
            mark: "",
            rate_sc: 0,
        });
    }

    /// REF chapter_exe.cpp proc_scene_change (lines 349-772), consuming
    /// precomputed pair features. `feats[x]` = pair (x-1, x); x==0 is the
    /// reference's self-comparison quirk (identical frames).
    // 原典 proc_scene_change の引数構成を維持している
    #[allow(clippy::too_many_arguments)]
    pub fn process_segment(
        &mut self,
        feats: &[Option<PairFeat>],
        n: i32,
        start_fr: i32,
        seri: i32,
        setseri: i32,
        breakmute: i32,
        extendmute: i32,
    ) -> usize {
        let mut ncount_sc = 0usize;

        //--- candidate arrays ---
        let mut msel = 0usize;
        let mut d_max_en = vec![0i32; DEF_SCMAX];
        let mut d_max_flagsc = vec![0i32; DEF_SCMAX];
        let mut d_max_pos = vec![0i32; DEF_SCMAX];
        let mut d_max_mvec = vec![0i32; DEF_SCMAX];
        let mut d_maxp_mvec = vec![0i32; DEF_SCMAX];
        let mut d_maxn_mvec = vec![0i32; DEF_SCMAX];
        let mut d_max_mvec2 = vec![0i32; DEF_SCMAX];
        let mut d_maxp_mvec2 = vec![0i32; DEF_SCMAX];
        let mut d_maxn_mvec2 = vec![0i32; DEF_SCMAX];
        let mut d_max_scrate = vec![0i32; DEF_SCMAX];

        //--- position settings ---
        let mut range_start_fr = start_fr - extendmute - 1;
        let mut valid_start_fr = start_fr - extendmute;
        let mut range_end_fr = start_fr + seri + extendmute + 1;
        let mut valid_end_fr = start_fr + seri + extendmute;
        if range_start_fr < 0 {
            range_start_fr = 0;
        }
        if valid_start_fr < 0 {
            valid_start_fr = 0;
        }
        if range_end_fr >= n {
            range_end_fr = n - 1;
        }
        if valid_end_fr >= n {
            valid_end_fr = n - 1;
        }

        //--- scan state ---
        let mut local_end_fr = start_fr + breakmute;
        let mut local_cntsc = 0i32;
        let mut pos_lastchange = -SPACE_SC1;
        if self.last_scpos > 0 {
            pos_lastchange = self.last_scpos;
        }
        let mut skip_update;
        let mut flag_sc_hold = 0i32;
        let mut last_cmvec = 0i32;
        let mut last_cmvec2 = 0i32;
        let mut last_rate = 0i32;
        let mut keep_schk = 0i32;
        let mut keep_msel = 0usize;

        for x in range_start_fr..=range_end_fr {
            // features for this pair position (reference: mvec(cur,bef,...,x))
            let f = pair_feat(feats, x);
            let (cmvec, cmvec2, mut flag_sc, rate_sc) = (f.mvec1, f.mvec2, f.flag_sc, f.rate_sc);

            if d_max_en[msel] > 0 && x == d_max_pos[msel] + 1 {
                d_maxn_mvec[msel] = cmvec;
                d_maxn_mvec2[msel] = cmvec2;
            }

            if msel < DEF_SCMAX - 1 {
                if flag_sc_hold > 0 && local_cntsc < 1 {
                    local_cntsc += 1;
                    msel += 1;
                    flag_sc_hold = 0;
                }
                if x >= local_end_fr {
                    if local_cntsc == 0 && d_max_en[msel] > 0 || flag_sc_hold > 0 {
                        msel += 1;
                    } else {
                        d_max_en[msel] = 0;
                    }
                    local_end_fr += breakmute;
                    local_cntsc = 0;
                    flag_sc_hold = 0;
                }
            }

            if !(valid_start_fr <= x && x <= valid_end_fr) {
                // out of valid range
            } else {
                if flag_sc > 0 && d_max_flagsc[msel] > 0 {
                    if (x - d_max_pos[msel]).abs() <= SPACE_SC2
                        && (d_max_scrate[msel] >= THRES_RATE1
                            || rate_sc < THRES_RATE1
                            || last_rate * 2 > rate_sc)
                    {
                        flag_sc = 0;
                    }
                    if (x - d_max_pos[msel]).abs() <= SPACE_SC3
                        && (d_max_scrate[msel] > rate_sc || last_rate * 2 > rate_sc)
                    {
                        flag_sc = 0;
                    }
                }

                skip_update = 0;
                if (x - pos_lastchange).abs() <= SPACE_SC1 && x - pos_lastchange != 0 {
                    skip_update = 1;
                    if keep_schk > 0
                        && flag_sc > 0
                        && rate_sc >= THRES_RATE1
                        && d_max_scrate[keep_msel] * 2 <= rate_sc
                        && last_rate * 3 <= rate_sc
                    {
                        skip_update = 0;
                        if msel != keep_msel {
                            msel = keep_msel;
                            if local_cntsc > 0 {
                                local_cntsc -= 1;
                            }
                        }
                    }
                } else {
                    keep_schk = 0;
                }

                if skip_update == 0 {
                    let mut flag_rateup = 0;
                    if (d_max_scrate[msel] < rate_sc && rate_sc >= THRES_RATE2)
                        || (d_max_scrate[msel] == rate_sc && d_max_mvec[msel] < cmvec)
                        || (d_max_scrate[msel] < THRES_RATE2
                            && rate_sc < THRES_RATE2
                            && d_max_mvec[msel] < cmvec)
                    {
                        flag_rateup = 1;
                    }
                    if (d_max_flagsc[msel] == 0 && flag_rateup > 0)
                        || d_max_en[msel] == 0
                        || flag_sc > 0
                    {
                        d_max_en[msel] = 1;
                        d_max_pos[msel] = x;
                        d_max_mvec[msel] = cmvec;
                        d_maxp_mvec[msel] = last_cmvec;
                        d_maxn_mvec[msel] = 0;
                        d_max_mvec2[msel] = cmvec2;
                        d_maxp_mvec2[msel] = last_cmvec2;
                        d_maxn_mvec2[msel] = 0;
                        d_max_scrate[msel] = rate_sc;
                        if flag_sc > 0 {
                            flag_sc_hold = 1;
                            d_max_flagsc[msel] = 1;
                            pos_lastchange = x;
                            if rate_sc < THRES_RATE1 {
                                keep_schk = 1;
                                keep_msel = msel;
                            }
                        }
                    }
                }
            }

            last_cmvec = cmvec;
            last_cmvec2 = cmvec2;
            last_rate = rate_sc;
        }

        if flag_sc_hold == 0
            && msel > 0
            && (local_cntsc > 0 || (local_end_fr - breakmute + setseri > range_end_fr))
        {
            d_max_en[msel] = 0;
        }

        //--- post processing: pre/rev positions, cleanup, lastmute state ---
        let mut d_maxpre_pos = vec![0i32; DEF_SCMAX];
        let mut d_maxrev_pos = vec![0i32; DEF_SCMAX];
        let mut msel_max = 0usize;
        for k in 0..=msel {
            if d_max_en[k] > 0 {
                d_maxpre_pos[k] = d_max_pos[k] - 1;
                if d_max_mvec[k] < d_maxp_mvec[k] * 2
                    && d_maxp_mvec[k] > d_maxp_mvec2[k] * 2
                    && d_max_mvec[k] - d_max_mvec2[k] > d_max_mvec[k] / 16
                {
                    d_maxpre_pos[k] = d_max_pos[k] - 2;
                }
                d_maxrev_pos[k] = d_max_pos[k];
                if d_maxrev_pos[k] - d_maxpre_pos[k] < 2
                    && d_max_mvec[k] > d_max_mvec2[k] * 2
                    && d_max_mvec[k] < d_maxn_mvec[k] * 2
                    && d_maxn_mvec[k] - d_maxn_mvec2[k] > d_maxn_mvec[k] / 16
                    && (d_maxn_mvec[k] > d_maxn_mvec2[k] * 2 || d_max_mvec[k] < d_maxn_mvec2[k] * 2)
                {
                    d_maxrev_pos[k] = d_max_pos[k] + 1;
                }
                if d_maxpre_pos[k] < 0 {
                    d_maxpre_pos[k] = 0;
                }
                if d_maxrev_pos[k] < 0 {
                    d_maxrev_pos[k] = 0;
                }
            }
        }
        if msel >= 1 {
            let mut d_tmpmax = -1i32;
            let mut rate_tmpmax = -1i32;
            for k in 0..=msel {
                if d_max_en[k] > 0
                    && (d_max_scrate[k] > rate_tmpmax
                        || (d_max_scrate[k] == rate_tmpmax && d_max_mvec[k] > d_tmpmax))
                {
                    msel_max = k;
                    rate_tmpmax = d_max_scrate[k];
                    d_tmpmax = d_max_mvec[k];
                }
            }
            for k in 0..=msel {
                if d_max_en[k] > 0 && d_max_scrate[k] < THRES_RATE2 && k != msel_max {
                    d_max_en[k] = 0;
                }
            }
        }
        {
            let mut tmp_scpos = -1i32;
            let mut tmp_flagsc = 0i32;
            for k in 0..=msel {
                if d_max_en[k] > 0 && tmp_scpos < d_max_pos[k] {
                    tmp_scpos = d_max_pos[k];
                    tmp_flagsc = d_max_flagsc[k];
                }
            }
            self.last_scpos = if tmp_flagsc > 0 { tmp_scpos } else { -1 };
        }

        //--- marks and records ---
        let mut flag_force_sc = 0i32;
        if msel > 0 {
            let mut msel_s = -1i32;
            let mut msel_e = -1i32;
            for (k, &v) in d_max_en.iter().enumerate().take(msel + 1) {
                if v > 0 {
                    msel_e = k as i32;
                    if msel_s < 0 {
                        msel_s = k as i32;
                    }
                }
            }
            if msel_s >= 0 && d_max_pos[msel_e as usize] - d_max_pos[msel_s as usize] > breakmute {
                flag_force_sc = 1;
            }
        }

        let mut msel_mark = msel_max;
        let mut msel_mknext = msel_max;
        let mut mark_type = 0i32;
        let mut difmin = 30i32;
        let last_frame = if self.last_marker < 0 {
            -10000
        } else {
            self.last_marker
        };
        for k in 0..=msel {
            if d_max_en[k] == 0 {
                continue;
            }
            let mut dif = (d_max_pos[k] - last_frame - 30 * 15).abs();
            if dif < difmin {
                difmin = dif;
                msel_mark = k;
                mark_type = 15;
            }
            dif = (d_max_pos[k] - last_frame - 30 * 30).abs();
            if dif < difmin {
                difmin = dif;
                msel_mark = k;
                mark_type = 30;
            }
            dif = (d_max_pos[k] - last_frame - 30 * 45).abs();
            if dif < difmin {
                difmin = dif;
                msel_mark = k;
                mark_type = 45;
            }
            dif = (d_max_pos[k] - last_frame - 30 * 60).abs();
            if dif < difmin {
                difmin = dif;
                msel_mark = k;
                mark_type = 60;
            }
            if k == msel_mark {
                msel_mknext = k;
            }
            if seri > breakmute
                && k > msel_mark
                && (d_max_pos[k] - d_max_pos[msel_mark]).abs() > breakmute
            {
                msel_mknext = k;
            }
        }

        let idx_snapshot = self.idx;
        let marks: Vec<&'static str> = (0..=msel)
            .map(|k| {
                let mark: &'static str = "";
                if d_max_scrate[k] < THRES_RATE2 {
                    return "＿";
                }
                if (flag_force_sc > 0 && (k != msel_mark || mark_type == 0))
                    || (d_max_scrate[k] < THRES_RATE2 && k != msel_max)
                {
                    return "○";
                }
                if k == msel_mark {
                    if idx_snapshot > 1 {
                        match mark_type {
                            15 => return "★",
                            30 => return "★★",
                            45 => return "★★★",
                            60 => return "★★★★",
                            _ => return mark,
                        }
                    }
                    return mark;
                }
                "＠"
            })
            .collect();

        for k in 0..=msel {
            if d_max_en[k] == 0 {
                continue;
            }
            let mark = marks[k];
            ncount_sc += 1;
            self.records.push(ScRecord {
                chapter_frame: start_fr,
                seri,
                pos_sc: d_maxrev_pos[k],
                pos_bk: d_maxpre_pos[k],
                mute_start_fr: start_fr,
                mute_end_fr: start_fr + seri,
                still: mark == "＿",
                mark,
                rate_sc: d_max_scrate[k],
            });
        }
        self.last_marker = d_max_pos[msel_mknext];
        self.idx += 1;
        ncount_sc
    }
}

impl Default for SceneState {
    fn default() -> Self {
        Self::new()
    }
}

fn pair_feat(feats: &[Option<PairFeat>], x: i32) -> PairFeat {
    if x <= 0 {
        // reference compares frame 0 with itself at the very first position
        return PairFeat {
            mvec1: 1,
            mvec2: 1,
            flag_sc: 0,
            rate_sc: 0,
        };
    }
    feats.get(x as usize).and_then(|o| *o).unwrap_or({
        // features must cover the processed range; fall back to "no change"
        PairFeat {
            mvec1: 1,
            mvec2: 1,
            flag_sc: 0,
            rate_sc: 0,
        }
    })
}

/// Native-rate, all-channel mute windows. Only the current window's maximum
/// and run state survive each push; channel/format changes need no reset.
/// Rate segments carry exact elapsed time so a crossing window is not lost.
pub struct MuteDetector {
    threshold: i32,
    setseri: i32,
    fps_n: i128,
    fps_d: i128,
    rate: u32,
    /// Reduced elapsed seconds before the current rate segment.
    elapsed_n: i128,
    elapsed_d: i128,
    /// Native sample positions consumed within the current rate segment.
    position: i128,
    /// Last checked floor boundary in the current rate segment.
    window_end: i128,
    peak: u16,
    seen: bool,
    /// frames already evaluated
    done: i32,
    seri: i32,
    out: Vec<MuteInterval>,
}

impl MuteDetector {
    pub fn new(frate_n: i64, frate_d: i64, threshold: i32, setseri: i32) -> Self {
        MuteDetector {
            threshold,
            setseri,
            fps_n: i128::from(frate_n),
            fps_d: i128::from(frate_d),
            rate: 0,
            elapsed_n: 0,
            elapsed_d: 1,
            position: 0,
            window_end: 0,
            peak: 0,
            seen: false,
            done: 0,
            seri: 0,
            out: Vec::new(),
        }
    }

    pub fn push(&mut self, peaks: &[u16], rate: u32) -> Result<(), &'static str> {
        const ERROR: &str = "audio time mapping overflow or invalid rate";
        if rate == 0 || self.fps_n <= 0 || self.fps_d <= 0 {
            return Err(ERROR);
        }
        if peaks.is_empty() {
            return Ok(());
        }
        if self.rate != 0 && self.rate != rate {
            let old_rate = i128::from(self.rate);
            let common = gcd(self.elapsed_d, old_rate);
            let left = old_rate / common;
            let right = self.elapsed_d / common;
            self.elapsed_n = self
                .elapsed_n
                .checked_mul(left)
                .and_then(|v| {
                    self.position
                        .checked_mul(right)
                        .and_then(|p| v.checked_add(p))
                })
                .ok_or(ERROR)?;
            self.elapsed_d = self.elapsed_d.checked_mul(left).ok_or(ERROR)?;
            let common = gcd(self.elapsed_n, self.elapsed_d);
            self.elapsed_n /= common;
            self.elapsed_d /= common;
            self.position = 0;
        }
        self.rate = rate;
        let mut cursor = 0;
        while cursor < peaks.len() {
            // floor((frame_end - elapsed) * rate), in native samples.
            let boundary = (i128::from(self.done) + 1)
                .checked_mul(self.fps_d)
                .and_then(|v| v.checked_mul(self.elapsed_d))
                .ok_or(ERROR)?;
            let elapsed = self.elapsed_n.checked_mul(self.fps_n).ok_or(ERROR)?;
            let numerator = boundary
                .checked_sub(elapsed)
                .and_then(|v| v.checked_mul(i128::from(rate)))
                .ok_or(ERROR)?;
            let denominator = self.fps_n.checked_mul(self.elapsed_d).ok_or(ERROR)?;
            let end = numerator.div_euclid(denominator).max(0);
            self.window_end = end;
            let take = (end - self.position)
                .max(0)
                .min((peaks.len() - cursor) as i128) as usize;
            if take > 0 {
                self.peak = self
                    .peak
                    .max(*peaks[cursor..cursor + take].iter().max().unwrap());
                self.seen = true;
                self.position = self.position.checked_add(take as i128).ok_or(ERROR)?;
                cursor += take;
            }
            // At a chunk end, the next rate segment may still contribute to
            // this floor window. Resolve its boundary before committing it.
            if self.position >= end && cursor < peaks.len() {
                self.close_window(!self.seen || i32::from(self.peak) > self.threshold);
                self.done = self.done.checked_add(1).ok_or(ERROR)?;
                self.peak = 0;
                self.seen = false;
            }
        }
        Ok(())
    }

    fn close_window(&mut self, nomute: bool) {
        if nomute {
            if self.seri >= self.setseri {
                self.out.push(MuteInterval {
                    start_fr: self.done - self.seri,
                    len: self.seri,
                });
            }
            self.seri = 0;
        } else {
            self.seri += 1;
        }
    }

    /// close evaluation at the batch loop bound `total_frames - setseri - 1`;
    /// frames beyond the pushed samples are nomute, and a still-open run at
    /// the exit is dropped, exactly like `detect_mute_intervals`.
    pub fn finish(mut self, total_frames: i32) -> Vec<MuteInterval> {
        let last_i = (total_frames - self.setseri - 1).max(0);
        // No next segment remains: a pending fixed-rate floor boundary is
        // complete even when its physical time precedes the frame end.
        if self.done < last_i && self.rate > 0 && self.position >= self.window_end {
            self.close_window(!self.seen || i32::from(self.peak) > self.threshold);
            self.done += 1;
        }
        if self.done < last_i {
            // An incomplete tail window is non-mute even if its prefix is quiet.
            self.close_window(true);
        }
        // Audio can decode ahead of video; preserve the reference scan bound.
        self.out.retain(|iv| iv.start_fr + iv.len < last_i);
        self.out
    }
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
