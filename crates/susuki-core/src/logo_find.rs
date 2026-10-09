//! Derived from logoframe (https://github.com/tobitti0/logoframe,
//! commit c93aec0c89838e9b4dd3a9a3d5496dfaaa41311d), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Design 6.3.1 step 3: LogoFind chain port from REF-LOGO logoframe_det.c
//! (fixed commit c93aec0c, lines 1236-1281, 2797-3810).
//! FrameStore mirrors LOGO_FRAMEREC; search semantics (rough edges, fine
//! location, fade detect, interlace adjust) are ported verbatim including the
//! reference's boundary-garbage reads, which we determinize as zero-padded
//! arrays (spec-notes N8).

use crate::logo_set::Thres;

pub const LOGO_FIND_MAX: usize = 256;
/// padding beyond num_frames: fade_detect may reference frm+step+3 etc. at the
/// last boundary; the C reads heap bytes there. We zero-pad (N8) so results are
/// deterministic; pad sized to the max observed index overrun ((step+3)*2+2 <= 70).
pub const FIND_PAD: usize = 128;

/// LOGO_OUTSUBREC
#[derive(Debug, Clone, Copy, Default)]
pub struct OutSub {
    pub frm_rise: i64,
    pub frm_fall: i64,
    pub frm_rise_l: i64,
    pub frm_rise_r: i64,
    pub frm_fall_l: i64,
    pub frm_fall_r: i64,
    pub fade_rise: i8,
    pub fade_fall: i8,
    pub intl_rise: i8,
    pub intl_fall: i8,
}

/// LOGO_FRAMEREC (LogoFrameInit 1236-1281). Arrays padded by FIND_PAD.
#[derive(Debug, Clone)]
pub struct FrameStore {
    pub num_frames: i64,
    pub rate_logoon: Vec<i8>,
    pub rate_fade: Vec<i8>,
    pub flag_nosample: Vec<i8>,
    pub flag_nofadesm: Vec<i8>,
    pub rate_logooni1: Vec<i8>,
    pub rate_logooni2: Vec<i8>,
    pub rate_fadei1: Vec<i8>,
    pub rate_fadei2: Vec<i8>,
    pub rank_unclear: Vec<i8>,
    pub flag_logoon: Vec<i8>,
    pub frm_leftstart: i64,
    pub frm_rightend: i64,
    pub workres: [OutSub; LOGO_FIND_MAX],
    pub num_find: i64,
    pub res: [OutSub; LOGO_FIND_MAX],
}

impl FrameStore {
    pub fn new(num_frames: i64) -> Self {
        let n = (num_frames + FIND_PAD as i64) as usize;
        FrameStore {
            num_frames,
            rate_logoon: vec![0; n],
            rate_fade: vec![0; n],
            flag_nosample: vec![0; n],
            flag_nofadesm: vec![0; n],
            rate_logooni1: vec![0; n],
            rate_logooni2: vec![0; n],
            rate_fadei1: vec![0; n],
            rate_fadei2: vec![0; n],
            rank_unclear: vec![0; n],
            flag_logoon: vec![0; n],
            frm_leftstart: 0,
            frm_rightend: 0,
            workres: [OutSub::default(); LOGO_FIND_MAX],
            num_find: 0,
            res: [OutSub::default(); LOGO_FIND_MAX],
        }
    }

    /// Append a no-logo frame while preserving the boundary padding used by
    /// LogoFind. Decoded images are not retained, only per-frame features.
    pub fn push_frame(&mut self) -> usize {
        let index = self.num_frames as usize;
        self.num_frames += 1;
        let len = self.num_frames as usize + FIND_PAD;
        self.rate_logoon.resize(len, 0);
        self.rate_fade.resize(len, 0);
        self.flag_nosample.resize(len, 0);
        self.flag_nofadesm.resize(len, 0);
        self.rate_logooni1.resize(len, 0);
        self.rate_logooni2.resize(len, 0);
        self.rate_fadei1.resize(len, 0);
        self.rate_fadei2.resize(len, 0);
        self.rank_unclear.resize(len, 0);
        self.flag_logoon.resize(len, 0);
        index
    }
}

/// LogoFind_rough (2797-2928)
pub fn find_rough(f: &mut FrameStore, t: &Thres) {
    let num_onwidth_on = i64::from(t.num_onwidth);
    let num_onwidth_off = if i64::from(t.num_offwidth) == 0 {
        i64::from(t.num_onwidth)
    } else {
        i64::from(t.num_offwidth)
    };

    let (rank_onlevel, n_next_onlevel_on, n_next_onlevel_off) = if i64::from(t.num_onlevel) >= 10 {
        (
            i64::from(t.num_onlevel) / 10 + 1,
            (i64::from(t.num_onlevel) % 10) * num_onwidth_on / 10,
            (i64::from(t.num_onlevel) % 10) * num_onwidth_off / 10,
        )
    } else {
        (i64::from(t.num_onlevel) + 1, 0, 0)
    };

    for i in 0..f.num_frames as usize {
        f.flag_logoon[i] = if i64::from(f.rate_logoon[i]) >= i64::from(t.rate_th_logo) {
            1
        } else {
            0
        };
    }

    let mut invalid_left = 1i64;
    let mut invalid_right = 0i64;
    let mut frm_leftrise = 0i64;
    let mut frm_leftstart = 0i64;
    let mut frm_rightend = f.num_frames - 1;
    let mut num_find = 0i64;
    let mut stat_logoon = -1i64;
    for i in 0..f.num_frames {
        let flag_logoon = i64::from(f.flag_logoon[i as usize]);
        if invalid_left == 1 && i >= i64::from(t.num_cutleft) {
            invalid_left = 0;
            if stat_logoon == 1 {
                f.res[num_find as usize].frm_rise = frm_leftrise;
            } else if stat_logoon != 0 {
                stat_logoon = 0;
            }
        }
        if stat_logoon != flag_logoon
            && invalid_right == 0
            && i64::from(f.rank_unclear[i as usize]) <= rank_onlevel
            && num_find < LOGO_FIND_MAX as i64
        {
            let mut n_cnt_onlevel = 0i64;
            let mut ncount = 1i64;
            let mut tmp_logoon = flag_logoon;
            let mut j = i + 1;

            let (num_onwidth_sel, n_next_onlevel_sel) = if flag_logoon > 0 {
                (num_onwidth_on, n_next_onlevel_on)
            } else {
                (num_onwidth_off, n_next_onlevel_off)
            };
            while j < f.num_frames && tmp_logoon == flag_logoon && ncount < num_onwidth_sel {
                tmp_logoon = i64::from(f.flag_logoon[j as usize]);
                if i64::from(f.rank_unclear[j as usize]) == rank_onlevel {
                    n_cnt_onlevel += 1;
                }
                if i64::from(f.rank_unclear[j as usize]) > rank_onlevel
                    || n_cnt_onlevel > n_next_onlevel_sel
                {
                    tmp_logoon = -1;
                }
                ncount += 1;
                j += 1;
            }
            if tmp_logoon == flag_logoon {
                stat_logoon = flag_logoon;
                if flag_logoon == 1 {
                    if invalid_left == 1 {
                        frm_leftrise = i;
                    } else if i >= f.num_frames - i64::from(t.num_cutright) {
                        frm_rightend = i - 1;
                        invalid_right = 1;
                    } else {
                        f.res[num_find as usize].frm_rise = i;
                    }
                } else {
                    if invalid_left == 1 {
                        frm_leftstart = i;
                    } else {
                        f.res[num_find as usize].frm_fall = i - 1;
                        num_find += 1;
                    }
                }
            }
        }
    }
    if stat_logoon == 1 && invalid_right == 0 {
        f.res[num_find as usize].frm_fall = f.num_frames - 1;
        num_find += 1;
    }
    f.num_find = num_find;
    f.frm_leftstart = frm_leftstart;
    f.frm_rightend = frm_rightend;
}

/// LogoFind_fine_getloc (2930-3114)
fn fine_getloc(f: &FrameStore, loc_start: i64, loc_end: i64) -> (i64, i64, i64) {
    let loc_step = if loc_start <= loc_end { 1i64 } else { -1 };
    let mut val = 0i64;
    let mut val_max = 0i64;
    let mut ncnt_nosmp = 0i64;
    let mut ncnt0 = [0i64; 3];
    let mut ncnt1 = [30i64; 3];
    let mut nhold1 = [0i64; 2];
    let mut ncand = 0;
    let mut nend = 0;
    let nfinish = 0;
    let mut loc_short = loc_start;
    let mut loc_long = loc_start;
    let mut loc_cand = loc_start;
    let mut frm = loc_start;
    while frm != loc_end && nfinish == 0 {
        let nunclear = i64::from(f.rank_unclear[frm as usize]);
        if nunclear == 9 {
            if ncand == 0 {
                ncnt_nosmp += 1;
                loc_long = frm;
                if val + ncnt_nosmp >= val_max {
                    loc_cand = frm;
                    val = val_max;
                }
            } else {
                val -= 1;
            }
        } else if f.flag_logoon[frm as usize] == 0 {
            val -= 1;
            ncnt_nosmp = 0;
            ncnt1 = [0; 3];
            if nunclear == 0 {
                ncnt0[0] += 1;
            } else {
                ncnt0[0] = 0;
            }
            if nunclear <= 1 {
                ncnt0[1] += 1;
            } else {
                ncnt0[1] = 0;
            }
            if nunclear <= 2 {
                ncnt0[2] += 1;
            } else {
                ncnt0[2] = 0;
            }
            if ncnt0[0] >= 2 || ncnt0[1] >= 5 {
                nend = 1;
            }
            if ncnt0[1] >= 2 || ncnt0[2] >= 8 {
                ncand = 1;
            }
            if ncnt0[2] == 0 && nend == 0 && ncand == 0 {
                loc_long = frm;
            }
        } else {
            val += 1;
            ncnt_nosmp = 0;
            ncnt0 = [0; 3];
            if nunclear == 0 {
                val += 1;
                if nhold1[0] > 0 {
                    nhold1[0] -= 1;
                }
                if ncnt1[0] == 0 || nhold1[0] == 0 {
                    ncnt1[0] += 1;
                    nhold1[0] = 0;
                }
            } else if ncnt1[0] != 0 {
                if nhold1[0] == 0 && ncnt1[0] >= 2 {
                    nhold1[0] = 2;
                } else if nhold1[0] <= 2 && ncnt1[0] >= 3 {
                    nhold1[0] += 2;
                } else {
                    ncnt1[0] = 0;
                }
            }
            if nunclear <= 1 {
                if nhold1[1] > 0 {
                    nhold1[1] -= 1;
                }
                if ncnt1[1] == 0 || nhold1[1] == 0 {
                    ncnt1[1] += 1;
                    nhold1[1] = 0;
                }
            } else if ncnt1[1] != 0 {
                if nhold1[1] == 0 && ncnt1[1] >= 2 {
                    nhold1[1] = 2;
                } else if nhold1[1] <= 2 && ncnt1[1] >= 3 {
                    nhold1[1] += 2;
                } else {
                    ncnt1[1] = 0;
                }
            }
            if nunclear <= 2 {
                ncnt1[2] += 1;
            } else {
                ncnt1[2] = 0;
            }
            if (ncnt1[0] >= 2 && nhold1[0] == 0)
                || (ncnt1[1] >= 5 && nhold1[1] == 0)
                || ncnt1[2] >= 12
            {
                ncand = 0;
            }
            if (ncnt1[0] >= 2 && nhold1[0] == 0) || (ncnt1[1] >= 5 && nhold1[1] == 0) {
                nend = 0;
            }
            if (ncnt1[0] >= 5 && nhold1[0] == 0)
                || (ncnt1[1] >= 15 && nhold1[1] == 0)
                || ncnt1[2] >= 30
            {
                loc_short = frm;
                loc_long = frm;
                loc_cand = frm;
                nend = 0;
                ncand = 0;
                val = 0;
                val_max = 0;
            } else if val >= val_max && nend == 0 {
                loc_long = frm;
                loc_cand = frm;
                val_max = val;
            } else if nend == 0 {
                loc_long = frm;
            }
        }
        frm += loc_step;
    }
    (loc_cand, loc_long, loc_short)
}

/// LogoFind_fine (3116-3155)
pub fn find_fine(f: &mut FrameStore) {
    for i in 0..f.num_find as usize {
        let loc_ed = f.res[i].frm_rise;
        let loc_st = if i == 0 {
            f.frm_leftstart
        } else {
            f.res[i - 1].frm_fall + 1
        };
        let (loc_cand, loc_long, loc_short) = fine_getloc(f, loc_ed, loc_st);
        f.res[i].frm_rise = loc_cand;
        f.res[i].frm_rise_l = loc_long;
        f.res[i].frm_rise_r = loc_short;

        let loc_ed = f.res[i].frm_fall + 1;
        let loc_st = f.res[i].frm_rise + 1;
        let (loc_cand, loc_long2, loc_short2) = fine_getloc(f, loc_st, loc_ed);
        f.res[i].frm_fall = loc_cand;
        f.res[i].frm_fall_l = loc_short2;
        f.res[i].frm_fall_r = loc_long2;
    }
}

/// LogoFind_fade_getloc (3177-3319). Returns (frame, val_result).
fn fade_getloc(
    f: &FrameStore,
    loc_start: i64,
    loc_end: i64,
    param_fade: i64,
    mut npre: i64,
    flag_getmax: i64,
) -> (i64, i64) {
    let (difchk, npost_thres) = if param_fade > 10 {
        (2i64, param_fade)
    } else {
        (10i64, 7i64)
    };
    let loc_step = if loc_start <= loc_end { 1i64 } else { -1 };
    let mut npost = 0i64;
    let mut frm = loc_start - loc_step;
    let mut val_d1 = 0i64;
    let mut val_d2 = 0i64;
    let mut val_d3 = 0i64;
    let mut ncount = 0i64;
    let mut flag_1st = 1i64;
    while (ncount < 3 || npost < npost_thres) && frm != loc_end {
        if npre > 0 {
            npre -= 1;
            ncount = 0;
        } else {
            npost += 1;
        }
        frm += loc_step;
        let val_i1 = i64::from(f.rate_fadei1[frm as usize]);
        let val_i2 = i64::from(f.rate_fadei2[frm as usize]);
        let bm = i64::from(f.flag_nofadesm[frm as usize]);
        let (flag_nofadesm, val);
        if bm & 0x2 != 0 {
            flag_nofadesm = bm & 0x1;
            val = val_i1;
        } else if bm & 0x1 != 0 {
            flag_nofadesm = 0;
            val = val_i2;
        } else {
            flag_nofadesm = 0;
            val = if flag_getmax == 0 {
                val_i1.min(val_i2)
            } else {
                val_i1.max(val_i2)
            };
        }
        if flag_nofadesm != 0 {
            // reference: skip
        } else if flag_getmax == 0 {
            if flag_1st != 0 {
                val_d1 = val;
                flag_1st = 0;
            } else if val + difchk < val_d1 {
                ncount = 0;
            } else if val + difchk < val_d2 && ncount > 0 {
                ncount = 1;
            } else if val + difchk < val_d3 && ncount > 1 {
                ncount = 2;
            } else {
                ncount += 1;
            }
            val_d3 = val_d2;
            val_d2 = val_d1;
            if val < val_d1 {
                val_d1 = val;
            }
        } else {
            if flag_1st != 0 {
                val_d1 = val;
                flag_1st = 0;
            } else if val > val_d1 + difchk {
                ncount = 0;
            } else if val > val_d2 + difchk && ncount > 0 {
                ncount = 1;
            } else if val > val_d3 + difchk && ncount > 1 {
                ncount = 2;
            } else {
                ncount += 1;
            }
            val_d3 = val_d2;
            val_d2 = val_d1;
            if val > val_d1 {
                val_d1 = val;
            }
        }
    }
    (frm, val_d1)
}

/// boundary-safe read mirroring C's unguarded heap reads at frame edges
/// (reference reads adjacent garbage there; N8 determinizes as clamp/zero).
#[inline]
fn fr(arr: &[i8], idx: i64) -> i64 {
    if idx < 0 {
        0
    } else {
        i64::from(arr[std::cmp::min(idx as usize, arr.len() - 1)])
    }
}

/// LogoFind_fade_detect (3321-3550). Returns (loc| -1, num_fade, num_intl).
fn fade_detect(
    f: &FrameStore,
    param_fade: i64,
    loc_left: i64,
    loc_right: i64,
    val_min: i64,
    val_max: i64,
    flag_rise: i64,
) -> (i64, i64, i64) {
    let interlace = 2i64;
    let mut loc_avgdif_hold = -1i64;
    let mut val_avgdif_hold = 0i64;
    let mut num_fade_hold = 0i64;
    let mut num_intl_hold = 0i64;
    for j in 0..2i64 {
        let mut step = if j == 0 { 1i64 } else { param_fade + 1 };
        if step <= 0 {
            step = 1;
        }
        let loc_start = loc_left;
        let loc_end = loc_right - (step + 2);
        let mut loc_sumdif_hold = -1i64;
        let mut val_sumdif_hold = 0i64;
        let mut intl_sumdif_hold = 0i64;
        let mut frm = loc_start;
        while frm <= loc_end {
            let mut dif_it = 0i64;
            for n in 0..(step + 3) {
                if fr(&f.flag_nofadesm, frm + n) == 0 {
                    dif_it += fr(&f.rate_fadei1, frm + n) - fr(&f.rate_fadei2, frm + n);
                }
            }
            for it in 0..interlace {
                let mut ncnt_nosample = 0i64;
                let mut ncnt_nearsample = 0i64;
                let mut val_sumdif = 0i64;
                for k in 0..(step + 3) * interlace {
                    let knum = (it + k) / interlace;
                    let ks = k / interlace;
                    let sel_it = if (((it + k) % 2 == 0) as i64
                        ^ ((dif_it <= 0) as i64)
                        ^ ((flag_rise == 0) as i64))
                        == 0
                    {
                        1i64
                    } else {
                        2i64
                    };

                    let (flag_nofadesm, val_cur) = if sel_it == 1 {
                        (
                            fr(&f.flag_nofadesm, frm + knum) & 0x1,
                            fr(&f.rate_fadei1, frm + knum),
                        )
                    } else {
                        (
                            fr(&f.flag_nofadesm, frm + knum) & 0x2,
                            fr(&f.rate_fadei2, frm + knum),
                        )
                    };
                    let val_exp = if flag_rise != 0 {
                        if ks == 0 {
                            val_min
                        } else if ks == step + 2 {
                            val_max
                        } else {
                            ((ks - 1) * (val_max - val_min) + step / 2) / step + val_min
                        }
                    } else if ks == 0 {
                        val_max
                    } else if ks == step + 2 {
                        val_min
                    } else {
                        ((step + 1 - ks) * (val_max - val_min) + step / 2) / step + val_min
                    };
                    if flag_nofadesm != 0 {
                        ncnt_nosample += 1;
                    } else {
                        let val_tmp = (val_cur - val_exp).abs();
                        val_sumdif += val_tmp;
                        if val_tmp * 2 > val_max - val_min {
                            ncnt_nosample += 1;
                        }
                        if step > 10 {
                            if val_cur < (val_exp - val_min) / 2 + val_min && ks < step {
                                ncnt_nosample += 1;
                                val_sumdif -= val_tmp;
                            } else if val_tmp * 4 <= val_max - val_min {
                                ncnt_nearsample += 1;
                                if (flag_rise != 0 && (ks == 0 || ks == 1))
                                    || (flag_rise == 0 && (ks == step + 1 || ks == step + 2))
                                {
                                    val_sumdif += val_tmp;
                                }
                            }
                            let mut val_ref1 = -1i64;
                            let mut val_ref2 = 0i64;
                            if ks == 2 {
                                if sel_it == 1 {
                                    val_ref1 = fr(&f.rate_fadei1, frm + knum - 1);
                                    val_ref2 = fr(&f.rate_fadei1, frm + knum - 2);
                                } else {
                                    val_ref1 = fr(&f.rate_fadei2, frm + knum - 1);
                                    val_ref2 = fr(&f.rate_fadei2, frm + knum - 2);
                                }
                            } else if ks == step {
                                if sel_it == 1 {
                                    val_ref1 = fr(&f.rate_fadei1, frm + knum + 1);
                                    val_ref2 = fr(&f.rate_fadei1, frm + knum + 2);
                                } else {
                                    val_ref1 = fr(&f.rate_fadei2, frm + knum + 1);
                                    val_ref2 = fr(&f.rate_fadei2, frm + knum + 2);
                                }
                            }
                            if val_ref1 >= 0 {
                                if (flag_rise != 0 && ks == 2) || (flag_rise == 0 && ks == step) {
                                    if val_cur <= val_ref1 {
                                        val_sumdif += step / 2;
                                    }
                                    if val_ref2 < val_ref1 && val_ref1 < val_cur {
                                        val_sumdif += step;
                                    }
                                    if val_ref2 < val_ref1 {
                                        val_sumdif += step / 4;
                                    }
                                } else if ((flag_rise == 0 && ks == 2)
                                    || (flag_rise != 0 && ks == step))
                                    && val_cur >= val_ref1
                                {
                                    val_sumdif += step / 2;
                                }
                            }
                        }
                    }
                }
                if ncnt_nosample > 0 && ncnt_nosample < step * interlace {
                    val_sumdif +=
                        (val_sumdif * ncnt_nosample) / ((step + 3) * interlace - ncnt_nosample);
                }
                if (val_sumdif < val_sumdif_hold || loc_sumdif_hold < 0)
                    && ncnt_nosample < step * interlace / 2
                    && (step > 1 || ncnt_nosample == 0)
                    && (step <= 10 || ncnt_nearsample > (step + 3) * interlace * 3 / 5)
                {
                    val_sumdif_hold = val_sumdif;
                    if flag_rise != 0 {
                        loc_sumdif_hold = frm + 2;
                    } else {
                        loc_sumdif_hold = frm + step + it;
                    }
                    if it == 1 {
                        intl_sumdif_hold = if dif_it < 0 { 2 } else { 1 };
                    } else {
                        intl_sumdif_hold = 0;
                    }
                }
            }
            frm += 1;
        }
        if loc_sumdif_hold >= 0 {
            let val_avgdif = (val_sumdif_hold * 100 + (step + 3) / 2) / (step + 3);
            if val_avgdif < val_avgdif_hold
                || loc_avgdif_hold < 0
                || (step > 10 && loc_sumdif_hold >= 0)
            {
                val_avgdif_hold = val_avgdif;
                loc_avgdif_hold = loc_sumdif_hold;
                num_fade_hold = step - 1;
                num_intl_hold = intl_sumdif_hold;
            }
        }
    }
    if val_max - val_min < 20 || num_fade_hold == 0 {
        num_fade_hold = 0;
        num_intl_hold = 0;
        loc_avgdif_hold = -1;
    }
    (loc_avgdif_hold, num_fade_hold, num_intl_hold)
}

/// LogoFind_fade (3552-3687)
pub fn find_fade(f: &mut FrameStore, t: &Thres) {
    let (num_fadein, num_fadeout) = if i64::from(t.auto_fade) > 0 {
        (i64::from(t.auto_fade), i64::from(t.auto_fade))
    } else {
        (i64::from(t.num_fadein), i64::from(t.num_fadeout))
    };
    let mut total_fade = 0i64;
    for i in 0..f.num_find as usize {
        // rise
        let loc_target = f.res[i].frm_rise;
        let loc_border_min = if i == 0 {
            f.frm_leftstart
        } else {
            f.res[i - 1].frm_fall
        };
        let loc_border_max = f.res[i].frm_fall;
        let mut npre = 5i64;
        let mut loc_start = loc_target + npre;
        if loc_start > loc_border_max {
            loc_start = loc_border_max;
            npre = loc_border_max - loc_target;
        }
        let (loc_left, val_min) = fade_getloc(f, loc_start, loc_border_min, num_fadein, npre, 0);
        let mut npre = 5i64;
        let mut loc_start = loc_target - npre;
        if loc_start < loc_border_min {
            loc_start = loc_border_min;
            npre = loc_target - loc_border_min;
        }
        let (loc_right, val_max) = fade_getloc(f, loc_start, loc_border_max, num_fadein, npre, 1);
        let (loc_newrise, num_fade_rise, num_intl_rise) =
            fade_detect(f, num_fadein, loc_left, loc_right, val_min, val_max, 1);
        if loc_newrise >= 0 {
            total_fade += 1;
        }
        f.workres[i].frm_rise = loc_newrise;
        f.workres[i].fade_rise = num_fade_rise as i8;
        f.workres[i].intl_rise = num_intl_rise as i8;

        // fall
        let loc_target = f.res[i].frm_fall;
        let loc_border_min = f.res[i].frm_rise;
        let loc_border_max = if i as i64 == f.num_find - 1 {
            f.frm_rightend
        } else {
            f.res[i + 1].frm_rise
        };
        let mut npre = 5i64;
        let mut loc_start = loc_target + npre;
        if loc_start > loc_border_max {
            loc_start = loc_border_max;
            npre = loc_border_max - loc_target;
        }
        let (loc_left, val_max) = fade_getloc(f, loc_start, loc_border_min, num_fadeout, npre, 1);
        let mut npre = 5i64;
        let mut loc_start = loc_target - npre;
        if loc_start < loc_border_min {
            loc_start = loc_border_min;
            npre = loc_target - loc_border_min;
        }
        let (loc_right, val_min) = fade_getloc(f, loc_start, loc_border_max, num_fadeout, npre, 0);
        let (loc_newfall, num_fade_fall, num_intl_fall) =
            fade_detect(f, num_fadeout, loc_left, loc_right, val_min, val_max, 0);
        if loc_newfall >= 0 {
            total_fade += 1;
        }
        f.workres[i].frm_fall = loc_newfall;
        f.workres[i].fade_fall = num_fade_fall as i8;
        f.workres[i].intl_fall = num_intl_fall as i8;
    }

    if (total_fade >= f.num_find && total_fade > 1) || i64::from(t.auto_fade) == 0 {
        for i in 0..f.num_find as usize {
            if f.workres[i].frm_rise >= 0 {
                if f.res[i].frm_rise_l > f.workres[i].frm_rise {
                    f.res[i].frm_rise_l = f.workres[i].frm_rise;
                }
                f.res[i].frm_rise = f.workres[i].frm_rise;
                f.res[i].fade_rise = f.workres[i].fade_rise;
                f.res[i].intl_rise = f.workres[i].intl_rise;
            }
            if f.workres[i].frm_fall >= 0 {
                if f.res[i].frm_fall_r < f.workres[i].frm_fall {
                    f.res[i].frm_fall_r = f.workres[i].frm_fall;
                }
                f.res[i].frm_fall = f.workres[i].frm_fall;
                f.res[i].fade_fall = f.workres[i].fade_fall;
                f.res[i].intl_fall = f.workres[i].intl_fall;
            }
        }
    }
}

/// LogoFind_interlace (3689-3794)
pub fn find_interlace(f: &mut FrameStore) {
    for i in 0..f.num_find as usize {
        // rise
        let loc_target = f.res[i].frm_rise;
        let mut loc_max = 0i64;
        let mut val_max = 0i64;
        let mut flag_intl = 0i64;
        if f.res[i].fade_rise == 0 {
            if loc_target >= f.frm_leftstart + 2 && loc_target < f.frm_rightend {
                for j in 0..=3i64 {
                    let loc = loc_target + j - 2;
                    let val_dif = (i64::from(f.rate_logooni1[loc as usize])
                        - i64::from(f.rate_logooni2[loc as usize]))
                    .abs();
                    if val_dif > val_max {
                        val_max = val_dif;
                        if (1..=2).contains(&j) {
                            loc_max = loc;
                        }
                    }
                }
                if loc_max > 0
                    && val_max > 20
                    && ((i64::from(f.rate_logooni1[loc_max as usize]) <= 50
                        && i64::from(f.rate_logooni2[loc_max as usize]) >= 50)
                        || (i64::from(f.rate_logooni1[loc_max as usize]) >= 50
                            && i64::from(f.rate_logooni2[loc_max as usize]) <= 50))
                {
                    let a1 = i64::from(f.rate_logooni1[(loc_max + 1) as usize]);
                    let a0 = i64::from(f.rate_logooni1[(loc_max - 1) as usize]);
                    let b1 = i64::from(f.rate_logooni2[(loc_max + 1) as usize]);
                    let b0 = i64::from(f.rate_logooni2[(loc_max - 1) as usize]);
                    let val_qi1 = ((a1 - a0) + 2) / 4;
                    let val_qi2 = ((b1 - b0) + 2) / 4;
                    let val_di1 = i64::from(f.rate_logooni1[loc_max as usize]) - a0;
                    let val_di2 = i64::from(f.rate_logooni2[loc_max as usize]) - b0;
                    if val_di1 > val_qi1 * 2
                        && val_di2 < val_qi2 * 2
                        && val_di1 - val_qi1 * 3 > val_di2 - val_qi2
                    {
                        flag_intl = 1;
                    } else if val_di2 > val_qi2 * 2
                        && val_di1 < val_qi1 * 2
                        && val_di2 - val_qi2 * 3 > val_di1 - val_qi1
                    {
                        flag_intl = 2;
                    }
                }
            }
            if flag_intl > 0 {
                if f.res[i].frm_rise_l == f.res[i].frm_rise {
                    f.res[i].frm_rise_l = loc_max;
                }
                if f.res[i].frm_rise_r == f.res[i].frm_rise {
                    f.res[i].frm_rise_r = loc_max;
                }
                f.res[i].frm_rise = loc_max;
            }
            f.res[i].intl_rise = flag_intl as i8;
        }

        // fall
        let loc_target = f.res[i].frm_fall;
        let mut loc_max = 0i64;
        let mut val_max = 0i64;
        let mut flag_intl = 0i64;
        if f.res[i].fade_fall == 0 {
            if loc_target > f.frm_leftstart && loc_target <= f.frm_rightend - 2 {
                for j in 0..=3i64 {
                    let loc = loc_target + j - 1;
                    let val_dif = (i64::from(f.rate_logooni1[loc as usize])
                        - i64::from(f.rate_logooni2[loc as usize]))
                    .abs();
                    if val_dif > val_max {
                        val_max = val_dif;
                        if (1..=2).contains(&j) {
                            loc_max = loc;
                        }
                    }
                }
                if loc_max > 0
                    && val_max > 20
                    && ((i64::from(f.rate_logooni1[loc_max as usize]) <= 50
                        && i64::from(f.rate_logooni2[loc_max as usize]) >= 50)
                        || (i64::from(f.rate_logooni1[loc_max as usize]) >= 50
                            && i64::from(f.rate_logooni2[loc_max as usize]) <= 50))
                {
                    let a1 = i64::from(f.rate_logooni1[(loc_max - 1) as usize]);
                    let a0 = i64::from(f.rate_logooni1[(loc_max + 1) as usize]);
                    let b1 = i64::from(f.rate_logooni2[(loc_max - 1) as usize]);
                    let b0 = i64::from(f.rate_logooni2[(loc_max + 1) as usize]);
                    let val_qi1 = ((a1 - a0) + 2) / 4;
                    let val_qi2 = ((b1 - b0) + 2) / 4;
                    let val_di1 = i64::from(f.rate_logooni1[loc_max as usize]) - a0;
                    let val_di2 = i64::from(f.rate_logooni2[loc_max as usize]) - b0;
                    if val_di1 > val_qi1 * 2
                        && val_di2 < val_qi2 * 2
                        && val_di1 - val_qi1 * 3 > val_di2 - val_qi2
                    {
                        flag_intl = 1;
                    } else if val_di2 > val_qi2 * 2
                        && val_di1 < val_qi1 * 2
                        && val_di2 - val_qi2 * 3 > val_di1 - val_qi1
                    {
                        flag_intl = 2;
                    }
                }
            }
            if flag_intl > 0 {
                if f.res[i].frm_fall_l == f.res[i].frm_fall {
                    f.res[i].frm_fall_l = loc_max;
                }
                if f.res[i].frm_fall_r == f.res[i].frm_fall {
                    f.res[i].frm_fall_r = loc_max;
                }
                f.res[i].frm_fall = loc_max;
            }
            f.res[i].intl_fall = flag_intl as i8;
        }
    }
}

/// LogoFind (3795-3807)
pub fn logo_find(f: &mut FrameStore, t: &Thres) {
    find_rough(f, t);
    find_fine(f);
    find_fade(f, t);
    find_interlace(f);
}
