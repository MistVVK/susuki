//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Phase-A of the JlsAutoReform port (design plan 5A): score/chap
//! estimation base and common helpers. Stateless like the reference
//! (all trace data is passed through arguments).

use super::cmdarg::WideMsec;
use super::cnvstr::Cnv;
use super::dataset::*;
use super::dataset_search::*;
use super::tables::ConfigVarType;

pub const SIZE_SETFORM_KEEP: usize = 6;

#[derive(Clone, Copy, Default)]
pub struct CalcDifInfo {
    pub sgn: i32,
    pub sec: i32,
    pub gap: i32,
}

#[derive(Clone, Copy, Default)]
pub struct CalcModInfo {
    pub mod15: i32,
    pub mod05: i32,
}

#[derive(Clone, Copy, Default)]
pub struct RangeFixMsec {
    pub st: i32,
    pub ed: i32,
    pub fix_st: bool,
    pub fix_ed: bool,
}

#[derive(Clone, Copy)]
pub struct RangeWideMsec {
    pub st: WideMsec,
    pub ed: WideMsec,
}

#[derive(Clone, Copy)]
pub struct TraceChap {
    pub num_da: usize,
    pub num_db: usize,
    pub nsc_da: [i32; SIZE_SETFORM_KEEP],
    pub nsc_db: [i32; SIZE_SETFORM_KEEP],
}

impl Default for TraceChap {
    fn default() -> Self {
        TraceChap {
            num_da: 0,
            num_db: 0,
            nsc_da: [0; SIZE_SETFORM_KEEP],
            nsc_db: [0; SIZE_SETFORM_KEEP],
        }
    }
}

/// setScore(RangeMsec)
pub fn set_score(ds: &mut Dataset, cnv: &Cnv, scope_st: i32, scope_ed: i32) {
    let _ = cnv;
    let num_scpos = ds.size_scp();
    let mut nsc_cur = 1i32;
    let mut nsc_last = -1i32;
    let mut over = false;
    if nsc_cur >= num_scpos - 1 {
        over = true;
    }
    while !over {
        let mut chap_cur = ds.scp_chap(nsc_cur);
        let msec_cur = ds.msec_scp(nsc_cur);
        if (msec_cur >= scope_st || scope_st < 0) && (msec_cur <= scope_ed || scope_ed < 0) {
            let score = get_score(ds, cnv, msec_cur, scope_st, scope_ed);
            let score_org = ds.scp_score(nsc_cur);
            if chap_cur < SCP_CHAP_DECIDE || score >= score_org {
                ds.set_scp_score(nsc_cur, score);
            }
            if chap_cur < SCP_CHAP_DECIDE {
                chap_cur = if score >= 30 {
                    SCP_CHAP_CPOSQ
                } else if score >= 10 {
                    SCP_CHAP_CPOSIT
                } else {
                    SCP_CHAP_NONE
                };
                ds.set_scp_chap(nsc_cur, chap_cur);
            }
            if nsc_last >= 0 && chap_cur > SCP_CHAP_NONE {
                let chap_last = ds.scp_chap(nsc_last);
                let msec_last = ds.msec_scp(nsc_last);
                let msec_diff = msec_cur - msec_last;
                if msec_diff <= ds.msec_val_lap1 {
                    if ds.scp_score(nsc_last) < score && chap_last < SCP_CHAP_DECIDE {
                        ds.set_scp_chap(nsc_last, SCP_CHAP_DUPE);
                    } else if chap_cur <= 2 {
                        ds.set_scp_chap(nsc_cur, SCP_CHAP_DUPE);
                    }
                }
            }
        }
        if chap_cur > SCP_CHAP_NONE {
            nsc_last = nsc_cur;
        }
        nsc_cur += 1;
        if nsc_cur >= num_scpos - 1 {
            over = true;
        }
    }
}

/// setChap(RangeFixMsec)
pub fn set_chap(ds: &mut Dataset, cnv: &Cnv, fixscope: RangeFixMsec) {
    let mut trace = TraceChap::default();
    let num_scpos = ds.size_scp();
    let mut nsc_cur = 1i32;
    let mut over = false;
    if nsc_cur >= num_scpos - 1 {
        over = true;
    }
    while !over {
        let chap_cur = ds.scp_chap(nsc_cur);
        let msec_cur = ds.msec_scp(nsc_cur);
        if (msec_cur >= fixscope.st || fixscope.st < 0)
            && (msec_cur <= fixscope.ed || fixscope.ed < 0)
            && chap_cur > SCP_CHAP_NONE
        {
            set_chap_update(ds, &mut trace, nsc_cur);
            set_chap_guess(ds, cnv, &mut trace, nsc_cur);
        }
        nsc_cur += 1;
        if nsc_cur >= num_scpos - 1 || (msec_cur > fixscope.ed && fixscope.ed > 0) {
            over = true;
        }
        if over {
            set_chap_update(ds, &mut trace, -1);
        }
    }
    set_chap_thin_overlap(ds, fixscope);
}

/// setChapUpdate
pub fn set_chap_update(ds: &mut Dataset, trace: &mut TraceChap, nsc_cur: i32) {
    let msec_cur = if nsc_cur >= 0 {
        ds.msec_scp(nsc_cur)
    } else {
        0
    };
    //--- 新候補位置の確定処理 ---
    if trace.num_db > 0 {
        let msec_chk = ds.msec_scp(trace.nsc_db[0]);
        let msec_dif = if nsc_cur >= 0 {
            msec_cur - msec_chk
        } else {
            180 * 1000 + ds.msec_val_lap2
        };
        if msec_dif >= 180 * 1000 + ds.msec_val_lap2 {
            let msec_tmp = if trace.num_da > 0 {
                ds.msec_scp(trace.nsc_da[trace.num_da - 1])
            } else {
                msec_chk - 1
            };
            if msec_chk > msec_tmp && trace.num_db > 1 {
                for j in 0..trace.num_db {
                    trace.nsc_da[j] = trace.nsc_db[j];
                }
                trace.num_da = trace.num_db;
                trace.num_db = 0;
                set_chap_fixed(ds, trace);
            } else {
                trace.num_db = 0;
            }
        }
    }
    //--- 継続候補の確定処理 ---
    if trace.num_da > 0 {
        let mut flag_loop = true;
        while flag_loop {
            flag_loop = false;
            let nsc_chk = trace.nsc_da[0];
            let msec_chk = ds.msec_scp(nsc_chk);
            let msec_dif = if nsc_cur >= 0 {
                msec_cur - msec_chk
            } else {
                180 * 1000 + ds.msec_val_lap2
            };
            if msec_dif >= 180 * 1000 + ds.msec_val_lap2 {
                set_chap_fixed(ds, trace);
                for j in 1..trace.num_da {
                    trace.nsc_da[j - 1] = trace.nsc_da[j];
                }
                trace.num_da -= 1;
                if trace.num_da > 0 {
                    flag_loop = true;
                }
            }
        }
    }
}

/// setChapGuess
pub fn set_chap_guess(ds: &mut Dataset, cnv: &Cnv, trace: &mut TraceChap, nsc_cur: i32) {
    if nsc_cur < 0 {
        return;
    }
    let score_cur = ds.scp_score(nsc_cur);
    let msec_cur = ds.msec_scp(nsc_cur);
    //--- スコア情報の整理（継続候補） ---
    let mut scoremax_prev = 0i32;
    let mut scoremax_mid = 0i32;
    let mut scoremax_post = 0i32;
    if trace.num_da > 0 {
        let (nsc_chks, nsc_chke) = if trace.num_db > 0 {
            (trace.nsc_db[0], trace.nsc_db[trace.num_db - 1])
        } else {
            let s = trace.nsc_da[trace.num_da - 1] + 1;
            (s, s)
        };
        for j in 0..trace.num_da {
            let nsc_tmp = trace.nsc_da[j];
            let msec_tmp = ds.msec_scp(nsc_tmp);
            let msec_dif = (msec_cur - msec_tmp).abs();
            let score_tmp = set_chap_get_distance_score(ds, nsc_tmp, msec_dif);
            if nsc_tmp < nsc_chks {
                if scoremax_prev < score_tmp {
                    scoremax_prev = score_tmp;
                }
            } else if nsc_tmp > nsc_chke && scoremax_mid < score_tmp {
                scoremax_mid = score_tmp;
            }
        }
        scoremax_post = set_chap_maxscore(ds, cnv, trace.nsc_da[trace.num_da - 1]);
    }
    //--- スコア情報の整理（新候補） ---
    let mut scoremax_db = 0i32;
    if trace.num_db > 0 {
        for j in 0..trace.num_db {
            let nsc_tmp = trace.nsc_db[j];
            let msec_tmp = ds.msec_scp(nsc_tmp);
            let msec_dif = (msec_cur - msec_tmp).abs();
            let score_tmp = set_chap_get_distance_score(ds, nsc_tmp, msec_dif);
            if scoremax_db < score_tmp {
                scoremax_db = score_tmp;
            }
        }
    }

    //--- 継続/新候補の 5/15秒単位距離 ---
    let mut chkmod_d = [0i32; 2];
    let mut difmod_d = [0i32; 2];
    for k in 0..2 {
        chkmod_d[k] = 0;
        let mut jmax = if k == 0 { trace.num_da } else { trace.num_db };
        let mut j = 0usize;
        while j < jmax {
            let nsc_chk = if k == 0 {
                trace.nsc_da[j]
            } else {
                trace.nsc_db[j]
            };
            let msec_dif = msec_cur - ds.msec_scp(nsc_chk);
            let mut calcmod = CalcModInfo::default();
            let modtype = calc_dif_mod0515(ds, &mut calcmod, msec_dif);
            //--- 前に15秒単位、直前は15秒単位ではない時の処理 ---
            if chkmod_d[k] == 2 && modtype == 0 {
                let score_tmp = ds.scp_score(nsc_chk);
                if score_cur <= score_tmp {
                    chkmod_d[k] = 0;
                } else if msec_dif <= 31 * 1000 {
                    if ds.scp_chap(nsc_chk) >= SCP_CHAP_CDET {
                        ds.set_scp_chap(nsc_chk, SCP_CHAP_NONE);
                    }
                    let arr = if k == 0 {
                        &mut trace.nsc_da
                    } else {
                        &mut trace.nsc_db
                    };
                    for m in j..jmax - 1 {
                        arr[m] = arr[m + 1];
                    }
                    if k == 0 {
                        trace.num_da -= 1;
                    } else {
                        trace.num_db -= 1;
                    }
                    j = j.saturating_sub(1);
                    jmax -= 1;
                    // (jmax==0 exits loop naturally)
                    if jmax == 0 {
                        break;
                    }
                }
            }
            //--- 15/5秒単位最適箇所取得 ---
            if modtype >= 2 {
                if chkmod_d[k] == 2 {
                    if difmod_d[k] > calcmod.mod15 {
                        difmod_d[k] = calcmod.mod15;
                    }
                } else if chkmod_d[k] < 2 {
                    chkmod_d[k] = 2;
                    difmod_d[k] = calcmod.mod15;
                }
            } else if modtype > 0 {
                if chkmod_d[k] == 1 {
                    if difmod_d[k] > calcmod.mod05 {
                        difmod_d[k] = calcmod.mod05;
                    }
                } else if chkmod_d[k] < 1 {
                    chkmod_d[k] = 1;
                    difmod_d[k] = calcmod.mod05;
                }
            }
            j += 1;
        }
    }
    //--- 候補追加 ---
    if chkmod_d[0] > 0 {
        if trace.num_db > 0
            && (scoremax_prev >= scoremax_db && score_cur >= scoremax_db
                || scoremax_db < scoremax_mid)
        {
            trace.num_db = 0;
        }
        if chkmod_d[1] > 0 && trace.num_db > 0 && difmod_d[0] > difmod_d[1] {
            chkmod_d[0] = 0;
        } else {
            chkmod_d[1] = 0;
            if trace.num_da >= SIZE_SETFORM_KEEP {
                set_chap_fixed(ds, trace);
                for j in 1..SIZE_SETFORM_KEEP {
                    trace.nsc_da[j - 1] = trace.nsc_da[j];
                }
                trace.num_da = SIZE_SETFORM_KEEP - 1;
            }
            trace.nsc_da[trace.num_da] = nsc_cur;
            trace.num_da += 1;
            if chkmod_d[0] > 1 && trace.num_da > 1 && trace.num_db == 0 {
                set_chap_fixed(ds, trace);
            }
        }
    }
    if chkmod_d[1] > 0 {
        if trace.num_db < SIZE_SETFORM_KEEP {
            trace.nsc_db[trace.num_db] = nsc_cur;
            trace.num_db += 1;
        }
        if chkmod_d[1] > 1 {
            let mut flag_change = true;
            if trace.num_da > 0 {
                let nsc_k0 = trace.nsc_da[trace.num_da - 1];
                let score_k0 = ds.scp_score(nsc_k0);
                let mut score_k1 = 0i32;
                for j in 0..trace.num_db {
                    score_k1 += ds.scp_score(trace.nsc_db[j]);
                }
                if score_k0 >= score_k1 {
                    let msec_dif = msec_cur - ds.msec_scp(nsc_k0);
                    if msec_dif < 60 * 1000 {
                        flag_change = false;
                    }
                }
            }
            if flag_change {
                for j in 0..trace.num_db {
                    trace.nsc_da[j] = trace.nsc_db[j];
                }
                trace.num_da = trace.num_db;
                trace.num_db = 0;
                set_chap_fixed(ds, trace);
            }
        }
    }
    if chkmod_d[0] == 0 && chkmod_d[1] == 0 {
        if trace.num_da == 0 {
            trace.nsc_da[0] = nsc_cur;
            trace.num_da += 1;
        } else if (scoremax_prev < score_cur || scoremax_post < score_cur)
            && (trace.num_db == 0 || scoremax_db < score_cur)
        {
            trace.nsc_db[0] = nsc_cur;
            trace.num_db = 1;
        }
    }
}

/// setChapMaxscore
pub fn set_chap_maxscore(ds: &Dataset, cnv: &Cnv, nsc_target: i32) -> i32 {
    let _ = cnv;
    let num_scpos = ds.size_scp();
    let mut max_score = 0i32;
    let mut i = nsc_target + 1;
    let mut over = false;
    if i >= num_scpos - 1 || i < 0 {
        over = true;
    }
    while !over {
        let msec_target = ds.msec_scp(nsc_target);
        let msec_i = ds.msec_scp(i);
        let msec_dif = msec_i - msec_target;
        let mut calcmod = CalcModInfo::default();
        let modtype = calc_dif_mod0515(ds, &mut calcmod, msec_dif);
        if modtype > 0 {
            let score = set_chap_get_distance_score(ds, i, msec_dif);
            if max_score < score {
                max_score = score;
            }
        }
        if calcmod.mod15 < 0 || i >= num_scpos - 1 {
            over = true;
        }
        i += 1;
    }
    max_score
}

/// setChapGetDistanceScore
pub fn set_chap_get_distance_score(ds: &Dataset, nsc: i32, msec_dif: i32) -> i32 {
    let mut score = ds.scp_score(nsc);
    if msec_dif > 122 * 1000 {
        score /= 2;
    } else if msec_dif > 62 * 1000 {
        score = score * 3 / 4;
    }
    score
}

/// setChapFixed
pub fn set_chap_fixed(ds: &mut Dataset, trace: &mut TraceChap) -> bool {
    let flag_strict = set_chap_fixed_limit(ds, trace);
    if !flag_strict {
        let mut nsc_wait = -1i32;
        if trace.num_db > 0 {
            nsc_wait = trace.nsc_db[0];
        }
        for j in 0..trace.num_da {
            let nsc_chap = trace.nsc_da[j];
            if nsc_chap < nsc_wait || nsc_wait < 0 {
                let chap_dst = ds.scp_chap(nsc_chap);
                if chap_dst < SCP_CHAP_CDET {
                    ds.set_scp_chap(nsc_chap, SCP_CHAP_CDET);
                }
            }
        }
        return true;
    }
    false
}

/// setChapFixedLimit
pub fn set_chap_fixed_limit(ds: &Dataset, trace: &TraceChap) -> bool {
    if trace.num_da <= 1 {
        return true;
    }
    let mut flag_remove = false;
    let mut sum_s15 = 0i32;
    let mut sum_s05 = 0i32;
    for i in 0..trace.num_da - 1 {
        let nsc_from = trace.nsc_da[i];
        let nsc_to = trace.nsc_da[i + 1];
        let msec_from = ds.msec_scp(nsc_from);
        let msec_to = ds.msec_scp(nsc_to);
        let sec_dif = (msec_to - msec_from + 500) / 1000;
        let sec_dif05 = ((sec_dif + 2) / 5) * 5;
        let msec_new = msec_from + sec_dif05 * 1000;
        let msecdif = (msec_to - msec_new).abs();
        let flag_still = ds.scp_still(nsc_from) || ds.scp_still(nsc_to);
        let flag_both_still = ds.scp_still(nsc_from) && ds.scp_still(nsc_to);
        let mut flag_nomute = false;
        let mut flag_long_nomute = false;
        if nsc_to - nsc_from == 1 {
            flag_nomute = true;
            flag_long_nomute = true;
        } else {
            let mut flag_sub = true;
            for k in (nsc_from + 1)..nsc_to {
                if !ds.is_smute_same_area(nsc_from, k) && !ds.is_smute_same_area(nsc_to, k) {
                    flag_sub = false;
                } else {
                    let msec_k = ds.msec_scp(k);
                    if (msec_k - msec_from).abs() > ds.msec_val_lap2
                        || (msec_to - msec_k).abs() > ds.msec_val_lap2
                    {
                        flag_sub = false;
                    }
                }
            }
            if flag_sub {
                flag_long_nomute = true;
            }
        }
        let cancel_cntsc = ds.get_config(ConfigVarType::cancelCntSc) != 0;
        if cancel_cntsc {
            flag_nomute = true;
        }
        if sec_dif05 <= 30 && sec_dif05 > 0 && !flag_still && msecdif <= ds.msec_val_exact {
            sum_s05 += 1;
            if flag_nomute {
                sum_s05 += 1;
            }
        }
        if sec_dif05 % 15 == 0 && sec_dif05 > 0 {
            if sec_dif05 % 30 == 0
                && sec_dif05 >= 60
                && (flag_long_nomute || msecdif <= ds.msec_val_exact && !flag_still)
            {
                sum_s15 += 6;
            }
            if !flag_still || flag_nomute {
                sum_s15 += 1;
            }
            if msecdif <= ds.msec_val_near3 || flag_nomute {
                sum_s15 += 1;
            }
            if !flag_both_still && msecdif <= ds.msec_val_near1 {
                sum_s15 += 1;
            }
        }
    }
    if sum_s05 < 2 && sum_s15 < 6 {
        flag_remove = true;
    }
    flag_remove
}

/// setChapThinOverlap
pub fn set_chap_thin_overlap(ds: &mut Dataset, fixscope: RangeFixMsec) {
    let mut nsc_fix_st = -1i32;
    let mut nsc_fix_ed = -1i32;
    if fixscope.fix_st {
        nsc_fix_st = ds.nsc_from_msec_all(fixscope.st);
    }
    if fixscope.fix_ed {
        nsc_fix_ed = ds.nsc_from_msec_all(fixscope.ed);
    }
    let mut nsc_cur = 0i32;
    let mut msec_cur = 0i32;
    let mut over = false;
    while !over {
        let nsc_last = nsc_cur;
        let msec_last = msec_cur;
        nsc_cur = ds.nsc_next_scp_chap(nsc_last, SCP_CHAP_CDET);
        msec_cur = ds.msec_scp(nsc_cur);
        if nsc_cur < 0 || msec_cur > fixscope.ed && fixscope.ed >= 0 {
            over = true;
        } else if nsc_last <= 0 {
            // skip
        } else if (msec_cur >= fixscope.st || fixscope.st < 0)
            && (msec_last <= fixscope.ed || fixscope.ed < 0)
        {
            let msecdif_cur = (msec_cur - msec_last).abs();
            if msecdif_cur < ds.msec_val_lap2 {
                let score_last = ds.scp_score(nsc_last);
                let score_cur = ds.scp_score(nsc_cur);
                let flag_fix_last = nsc_last == nsc_fix_st || nsc_last == nsc_fix_ed;
                let flag_fix_cur = nsc_cur == nsc_fix_st || nsc_cur == nsc_fix_ed;
                let flag_score_last = score_last >= score_cur;
                if (flag_fix_last || flag_score_last) && !flag_fix_cur {
                    ds.set_scp_chap(nsc_cur, SCP_CHAP_CPOSQ);
                }
                if (flag_fix_cur || !flag_score_last) && !flag_fix_last {
                    ds.set_scp_chap(nsc_last, SCP_CHAP_CPOSQ);
                }
            }
        }
    }
}

/// getScore(msec, RangeMsec)
pub fn get_score(ds: &Dataset, cnv: &Cnv, msec_target: i32, scope_st: i32, scope_ed: i32) -> i32 {
    let score_all;
    let mut score_m3 = 0i32;
    let mut score_m2 = 0i32;
    let mut score_m1 = 0i32;
    let mut score_mt = 0i32;
    let mut score_sec5 = 0i32;
    let mut nsc_cur = 0i32;
    for i in -16..=16i32 {
        let sec_width: i32 = if i < -6 {
            i * 15 + 60
        } else if i <= 6 {
            i * 5
        } else {
            i * 15 - 60
        };
        let msec_cmp = msec_target + sec_width * 1000;
        if (scope_st <= msec_cmp + ds.msec_val_lap2 || scope_st < 0)
            && (scope_ed >= msec_cmp - ds.msec_val_lap2 || scope_ed < 0)
            && sec_width != 0
        {
            let mut nsc_c = nsc_cur;
            let msec_dist_min = get_score_dist(ds, &mut nsc_c, msec_cmp);
            nsc_cur = nsc_c;
            let (smute_det, still_det) =
                if (scope_st < 0 && sec_width < 0) || (scope_ed < 0 && sec_width > 0) {
                    (false, false)
                } else {
                    (
                        ds.is_smute_from_msec(msec_cmp),
                        ds.is_still_from_msec(msec_cmp),
                    )
                };
            let score_tmp = get_score_target(ds, msec_dist_min, sec_width, smute_det, still_det);
            let _ = cnv;
            if sec_width.abs() > 120 {
                score_m3 += score_tmp;
            } else if sec_width.abs() > 60 {
                score_m2 += score_tmp;
            } else if sec_width.abs() % 15 == 0 {
                score_m1 += score_tmp;
                if sec_width.abs() <= 30 {
                    score_mt += score_tmp;
                }
            } else {
                score_sec5 += score_tmp;
            }
        }
    }
    if score_mt >= 50 && score_m1 <= score_mt + 50 / 2 && score_m2 > score_m1 - score_mt {
        score_m2 = score_m1 - score_mt;
    }
    if score_m3 > score_m2 / 2 && score_m2 < score_m1 {
        score_m3 = score_m2 / 2;
    }
    let score_ma = score_m1 + score_m2 + score_m3;
    if score_ma + score_sec5 >= 50 {
        if score_ma >= 50 {
            score_all = score_ma;
        } else {
            score_all = 50;
        }
    } else {
        score_all = score_ma + score_sec5;
    }
    score_all
}

/// getScoreDist
pub fn get_score_dist(ds: &Dataset, nsc_cur: &mut i32, msec_cmp: i32) -> i32 {
    let num_scpos = ds.size_scp();
    let mut nsc_cand = -1i32;
    let mut msec_dist_min = -1i32;
    let msec_val_valid = ds.msec_val_near3;
    let mut over = false;
    if *nsc_cur >= num_scpos - 1 {
        over = true;
    }
    while !over {
        let msec_c = ds.msec_scp(*nsc_cur);
        let msec_dist_cur = (msec_c - msec_cmp).abs();
        if msec_dist_cur <= msec_val_valid && (nsc_cand < 0 || msec_dist_cur < msec_dist_min) {
            nsc_cand = *nsc_cur;
            msec_dist_min = msec_dist_cur;
        }
        if msec_c > msec_cmp + msec_val_valid {
            over = true;
        } else {
            *nsc_cur += 1;
            if *nsc_cur >= num_scpos - 1 {
                over = true;
            }
        }
    }
    msec_dist_min
}

/// getScoreTarget
pub fn get_score_target(
    ds: &Dataset,
    msec_dist: i32,
    sec_width: i32,
    smute_det: bool,
    still_det: bool,
) -> i32 {
    let mut score = 0i32;
    let mut flag_d15 = 0i32;
    let abs_sec = sec_width.abs();
    if abs_sec % 15 == 0 {
        flag_d15 = 1;
    }
    let mut ndist = 0i32;
    if msec_dist < 0 {
        ndist = -1;
    } else if msec_dist <= ds.msec_val_exact {
        ndist = 3;
    } else if msec_dist <= ds.msec_val_near1 {
        ndist = 2;
    } else if msec_dist <= ds.msec_val_near2 {
        ndist = 1;
    }
    if abs_sec == 0 {
        score = 0;
    } else if abs_sec <= 60 {
        if flag_d15 > 0 {
            if ndist >= 0 || smute_det {
                score = 21;
            }
            if ndist > 0 {
                score += ndist * 3;
                if ndist >= 3 {
                    score += ndist;
                }
            }
        } else if ndist > 0 {
            if still_det {
                ndist = 1;
            }
            score = 7;
            score += ndist * 3;
        }
    } else if ndist >= 0 || smute_det {
        if abs_sec <= 120 {
            score = 15;
        } else {
            score = 3;
        }
        score += ndist * 3;
    }
    score
}

/// checkMuteMany
pub fn check_mute_many(ds: &Dataset, rnsc_st: i32, rnsc_ed: i32) -> i32 {
    let mut type_mute = 0i32;
    if rnsc_st < rnsc_ed && rnsc_st >= 0 && rnsc_ed >= 0 {
        let mut count_smute = 0i32;
        if (rnsc_ed - rnsc_st).abs() > 1 {
            let lpcnt = (rnsc_ed - rnsc_st).abs() - 1;
            let mut nsc_tmp = rnsc_st;
            for _ in 0..lpcnt {
                nsc_tmp += 1;
                if !ds.is_smute_same_area(nsc_tmp, rnsc_st)
                    && !ds.is_smute_same_area(nsc_tmp, rnsc_ed)
                    && !ds.is_smute_same_area(nsc_tmp, nsc_tmp - 1)
                {
                    count_smute += 1;
                }
            }
            if ds.get_config(ConfigVarType::cancelCntSc) != 0 {
                count_smute = 0;
            }
        }
        let (msec_st, msec_ed) = ds.range_msec_from_range_nsc(rnsc_st, rnsc_ed);
        let msec_dif = msec_ed - msec_st;
        if (count_smute > 3 && msec_dif < 14500)
            || (count_smute > 4 && msec_dif < 29500)
            || (count_smute > 6 && msec_dif < 59500)
            || count_smute > 8
        {
            type_mute = 2;
        } else if (count_smute > 1 && msec_dif < 14500)
            || (count_smute > 2 && msec_dif < 29500)
            || (count_smute > 4 && msec_dif < 59500)
            || count_smute > 6
        {
            type_mute = 1;
        }
        let type_nosc = ds.get_config_action(CFG_ACT_MUTE_NO_SC);
        if type_mute > 0 && type_nosc == 1 && (ds.scp_still(rnsc_st) || ds.scp_still(rnsc_ed)) {
            type_mute += 4;
        }
    }
    type_mute
}

const CFG_ACT_MUTE_NO_SC: i32 = 5; // ConfigActType::MuteNoSc

/// addCommonComponentOne
pub fn add_common_component_one(
    ds: &mut Dataset,
    cnv: &Cnv,
    nsc_target: i32,
    nsc_side: i32,
    enable_short: bool,
) {
    let _ = cnv;
    let dir_next = nsc_target < nsc_side;
    if (nsc_target - nsc_side).abs() <= 1 {
        return;
    }
    let msec_mgn = ds.get_config(ConfigVarType::msecMgnCmDivide);
    let msec_target = ds.msec_scp(nsc_target);
    let msec_side = ds.msec_scp(nsc_side);
    let mut logomode = true;
    {
        let nsc_tmp = if dir_next { nsc_side } else { nsc_target };
        if !is_scp_ar_type_logo(ds.scp_arstat(nsc_tmp)) {
            logomode = false;
        }
    }
    let mut nsc_cur = nsc_target;
    let mut cont = true;
    while cont {
        nsc_cur = ds.nsc_dir_scp_chap(nsc_cur, dir_next, SCP_CHAP_NONE);
        if nsc_cur < 0 || nsc_cur == nsc_side {
            cont = false;
        } else {
            let msec_cur = ds.msec_scp(nsc_cur);
            let mut calc_target = CalcDifInfo::default();
            let mut calc_side = CalcDifInfo::default();
            let mut type_target = calc_dif_select(ds, cnv, &mut calc_target, msec_target, msec_cur);
            let type_side = calc_dif_select(ds, cnv, &mut calc_side, msec_side, msec_cur);
            if calc_target.sec < 30 && !enable_short {
                type_target = 0;
            } else if calc_target.sec >= 122 {
                cont = false;
            }
            if type_target > 0
                && (type_side > 0 || calc_side.sec > 30)
                && calc_target.gap <= msec_mgn
            {
                let arstat_cur = if logomode {
                    SCP_AR_L_OTHER
                } else {
                    SCP_AR_N_OTHER
                };
                ds.set_scp_chap(nsc_cur, SCP_CHAP_DINT);
                ds.set_scp_arstat(nsc_cur, arstat_cur);
                cont = false;
            }
        }
    }
}

/// calcDifSelect
pub fn calc_dif_select(
    ds: &Dataset,
    cnv: &Cnv,
    calcdif: &mut CalcDifInfo,
    msec_src: i32,
    msec_dst: i32,
) -> i32 {
    let msec_dif = (msec_dst - msec_src).abs();
    let sec_dif = cnv.get_sec_from_msec(msec_dif);
    let gap_dif = (sec_dif * 1000 - msec_dif).abs();
    let mut type_r = 0i32;
    if gap_dif <= ds.msec_val_near2
        && (sec_dif == 10
            || sec_dif == 15
            || sec_dif == 30
            || sec_dif == 45
            || sec_dif == 60
            || sec_dif == 90
            || sec_dif == 120)
    {
        type_r = 2;
    } else if gap_dif <= ds.msec_val_near1 && (3..=25).contains(&sec_dif) {
        type_r = 1;
    }
    calcdif.sgn = if msec_src > msec_dst {
        -1
    } else if msec_src < msec_dst {
        1
    } else {
        0
    };
    calcdif.sec = sec_dif;
    calcdif.gap = gap_dif;
    type_r
}

/// calcDifMod0515
pub fn calc_dif_mod0515(ds: &Dataset, calcmod: &mut CalcModInfo, msec_dif: i32) -> i32 {
    let mut ret = 0i32;
    let mut calc_mod15 = -1i32;
    let mut calc_mod05 = -1i32;
    if msec_dif > ds.msec_val_lap2 && msec_dif <= 180 * 1000 + ds.msec_val_lap2 {
        calc_mod15 = msec_dif % (15 * 1000);
        if calc_mod15 > (15 * 1000) / 2 {
            calc_mod15 = (15 * 1000) - calc_mod15;
        }
        if calc_mod15 <= ds.msec_val_near3 {
            ret += 2;
        }
    }
    if msec_dif > ds.msec_val_lap2 && msec_dif <= 30 * 1000 + ds.msec_val_lap2 {
        calc_mod05 = msec_dif % (5 * 1000);
        if calc_mod05 > (5 * 1000) / 2 {
            calc_mod05 = (5 * 1000) - calc_mod05;
        }
        if calc_mod05 <= ds.msec_val_near1 {
            ret += 1;
        }
    }
    calcmod.mod15 = calc_mod15;
    calcmod.mod05 = calc_mod05;
    ret
}

/// isCmLengthMsec
pub fn is_cm_length_msec(cnv: &Cnv, msec_target: i32) -> bool {
    let sec_target = cnv.get_sec_from_msec(msec_target);
    sec_target % 15 == 0
}

/// isLengthLogoLevel
pub fn is_length_logo_level(
    ds: &Dataset,
    cnv: &Cnv,
    msec_target: i32,
    en_long: bool,
    longmod: bool,
    mid5s: bool,
) -> bool {
    if msec_target < ds.msec_val_lap2 {
        return false;
    }
    let mut match_len = false;
    let lvmid = if mid5s { 5 } else { 6 }; // USE_MIDL / USE_MIDH
    let lvlogo = ds.level_use_logo();
    if lvlogo == 8 {
        // USE_MAX
        match_len = true;
    } else {
        let mut calcmod = CalcModInfo::default();
        let typemod = calc_dif_mod0515(ds, &mut calcmod, msec_target);
        let mut calcdif = CalcDifInfo::default();
        calc_dif_select(ds, cnv, &mut calcdif, 0, msec_target);
        if lvlogo >= 7 {
            // USE_HIGH
            if calcdif.gap <= ds.msec_val_near1 {
                match_len = true;
            }
        }
        if lvlogo >= lvmid && typemod >= 1 {
            match_len = true;
        }
        if typemod >= 2 {
            match_len = true;
        }
        if en_long && calcdif.sec % 30 == 0 && calcdif.sec > 180 && calcdif.sec <= 600 {
            match_len = true;
        }
        if longmod {
            if lvlogo >= lvmid && calcdif.sec % 5 == 0 && calcdif.gap <= ds.msec_val_near1 {
                match_len = true;
            }
            if calcdif.sec % 15 == 0 && calcdif.gap <= ds.msec_val_near1 {
                match_len = true;
            }
        }
    }
    match_len
}
