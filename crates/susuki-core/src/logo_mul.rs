//! Derived from logoframe (https://github.com/tobitti0/logoframe,
//! commit c93aec0c89838e9b4dd3a9a3d5496dfaaa41311d), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Design 6.3.3: multi-logo result merge.
//! Port of REF-LOGO LogoResultInit/Add_Copy/Add_set/Add_check/Add
//! (logoframe_det.c 3935-4073) and LogoGetTotalFrameWithClear (3830-3877)
//! plus the MultLogoFind(-oasel 0) selection semantics
//! (logoframe_mul.c 1377-1467, fixed commit c93aec0c).

use crate::logo_find::{FrameStore, OutSub, LOGO_FIND_MAX};
use crate::logo_set::Thres;

/// LOGO_RESULTOUTREC
#[derive(Debug, Clone)]
pub struct ResultOut {
    pub num_find: i64,
    pub res: [OutSub; LOGO_FIND_MAX],
}

impl ResultOut {
    pub fn new() -> Self {
        ResultOut {
            num_find: 0,
            res: [OutSub::default(); LOGO_FIND_MAX],
        }
    }
}

impl Default for ResultOut {
    fn default() -> Self {
        Self::new()
    }
}

fn add_copy(pdst: &mut OutSub, ptarget: &OutSub, kind: i64) {
    if kind == 0 || kind == 1 {
        pdst.frm_rise = ptarget.frm_rise;
        pdst.frm_rise_l = ptarget.frm_rise_l;
        pdst.frm_rise_r = ptarget.frm_rise_r;
        pdst.fade_rise = ptarget.fade_rise;
        pdst.intl_rise = ptarget.intl_rise;
    }
    if kind == 0 || kind == 2 {
        pdst.frm_fall = ptarget.frm_fall;
        pdst.frm_fall_l = ptarget.frm_fall_l;
        pdst.frm_fall_r = ptarget.frm_fall_r;
        pdst.fade_fall = ptarget.fade_fall;
        pdst.intl_fall = ptarget.intl_fall;
    }
}

/// LogoResultAdd_set (3970-4028). `lres` = one logo's intervals, `lnum` its count.
fn add_set(prs: &mut ResultOut, lres: &[OutSub], lnum: i64) {
    const SPC: i64 = 30; // treat as continuous logo within the frame
    let mut i = 0i64;
    while i < lnum {
        let ptarget = lres[i as usize];
        // search insert point
        let mut k = 0i64;
        while k < prs.num_find && ptarget.frm_rise_l > prs.res[k as usize].frm_fall_r + SPC {
            k += 1;
        }
        // divide if same logodata overlapped
        if i > 0
            && k < prs.num_find
            && lres[(i - 1) as usize].frm_fall_r == prs.res[k as usize].frm_fall_r
        {
            k += 1;
        }
        if prs.num_find >= LOGO_FIND_MAX as i64 {
            // already data full
        } else if k == prs.num_find {
            add_copy(&mut prs.res[k as usize], &ptarget, 0);
            prs.num_find += 1;
        } else if ptarget.frm_fall_r + SPC < prs.res[k as usize].frm_rise_l {
            let mut j = prs.num_find - 1;
            while j >= k {
                let copy = prs.res[j as usize];
                prs.res[(j + 1) as usize] = copy;
                j -= 1;
            }
            add_copy(&mut prs.res[k as usize], &ptarget, 0);
            prs.num_find += 1;
        } else {
            if prs.res[k as usize].frm_rise > ptarget.frm_rise {
                add_copy(&mut prs.res[k as usize], &ptarget, 1);
            }
            let mut m = k;
            while m + 1 < prs.num_find
                && prs.res[(m + 1) as usize].frm_rise_l < ptarget.frm_fall_r + SPC
            {
                m += 1;
            }
            if k < m {
                let fall = prs.res[m as usize];
                add_copy(&mut prs.res[k as usize], &fall, 2);
                let mut j = 1i64;
                while j < prs.num_find - m {
                    let mv = prs.res[(m + j) as usize];
                    prs.res[(k + j) as usize] = mv;
                    j += 1;
                }
                prs.num_find -= m - k;
            }
            if prs.res[k as usize].frm_fall < ptarget.frm_fall {
                add_copy(&mut prs.res[k as usize], &ptarget, 2);
            }
        }
        i += 1;
    }
}

/// LogoResultAdd_check (4037-4073). mgn=3 overlap margin.
fn add_check(prs: &ResultOut, lres: &[OutSub], lnum: i64) -> bool {
    const MGN: i64 = 3;

    if lnum == 0 {
        false
    } else if prs.num_find == 0 {
        true
    } else {
        let mut v = true;
        for i in 0..lnum {
            let ptarget = lres[i as usize];
            let mut nst = -1i64;
            let mut ned = -1i64;
            for j in 0..prs.num_find {
                let pdst = prs.res[j as usize];
                if ptarget.frm_rise_r + MGN >= pdst.frm_fall_l {
                    nst = j;
                }
                if ptarget.frm_fall_l > pdst.frm_rise_r + MGN {
                    ned = j;
                }
            }
            if nst != ned {
                v = false;
            }
        }
        v
    }
}

/// LogoResultAdd (4083-4095). Returns whether the logo was added.
pub fn result_add(prs: &mut ResultOut, lres: &[OutSub], lnum: i64, autosel: bool) -> bool {
    let flag = if autosel {
        add_check(prs, lres, lnum)
    } else {
        true
    };
    if flag {
        add_set(prs, lres, lnum);
    }
    flag
}

/// LogoGetTotalFrame (3810-3825) on this store's results.
fn total_frame(store: &FrameStore) -> i64 {
    let mut sum = 0i64;
    for i in 0..store.num_find {
        let t = store.res[i as usize].frm_fall - store.res[i as usize].frm_rise;
        if t > 0 {
            sum += t;
        }
    }
    sum
}

/// LogoGetTotalFrameWithClear (3830-3877). Clears the store's frame count when
/// the logo is judged too unclear (num_clrrate semantics).
pub fn total_frame_with_clear(store: &mut FrameStore, num_clrrate: i16) -> i64 {
    let mut unclear = if num_clrrate == 0 { 0i64 } else { 1 };
    let mut i = 0i64;
    while unclear > 0 && i < store.num_find {
        let loc_st = store.res[i as usize].frm_rise_r;
        let loc_ed = store.res[i as usize].frm_fall_l;
        let mut j = loc_st;
        while unclear > 0 && j <= loc_ed {
            let rank = i64::from(store.rank_unclear[j as usize]);
            let logoon = store.flag_logoon[j as usize];
            if (0..=1).contains(&rank) && logoon > 0 {
                unclear = 0;
            }
            j += 1;
        }
        i += 1;
    }
    let mut frame_sum = total_frame(store);
    if unclear > 0 && frame_sum * i64::from(num_clrrate) <= store.num_frames {
        store.num_frames = 0; // invalidate logo
        frame_sum = 0;
    }
    frame_sum
}

/// MultLogoFind + MultLogoFind_TotalResult merge stage for -oasel 0 with
/// all provided logos in use (v1 has no -oanum/-oasel options; design 6.3.3).
/// Returns (merged result, used flags per input order, totals per input).
pub fn merge_logos(stores: &mut [FrameStore], t: &Thres) -> (ResultOut, Vec<bool>, Vec<i64>) {
    let n = stores.len();
    let mut totals = vec![0i64; n];
    let mut valid = vec![false; n];
    for i in 0..n {
        totals[i] = total_frame_with_clear(&mut stores[i], t.num_clrrate);
        if totals[i] > 0 {
            valid[i] = true;
        }
    }
    let res: Vec<(Vec<OutSub>, i64)> = stores
        .iter()
        .map(|s| (s.res.to_vec(), s.num_find))
        .collect();
    let _ = n;
    merge_from_totals(&res, totals.clone(), valid.clone())
}

/// Pure merge on precomputed intervals/totals (MultLogoFind_TotalResult body).
pub fn merge_from_totals(
    res: &[(Vec<OutSub>, i64)],
    totals_in: Vec<i64>,
    valid_in: Vec<bool>,
) -> (ResultOut, Vec<bool>, Vec<i64>) {
    let n = res.len();
    let totals = totals_in;
    let mut valid = valid_in;
    // oanum = n (v1: all logos in scope)
    let _ = &totals;
    // priority_list sorted by total_frame descending, ties keep logo order
    let mut prio: Vec<usize> = (0..n).collect();
    let mut i = n as i64 - 1;
    while i > 0 {
        let mut j = i - 1;
        while j >= 0 {
            if totals[prio[i as usize]] > totals[prio[j as usize]] {
                prio.swap(i as usize, j as usize);
            }
            j -= 1;
        }
        i -= 1;
    }

    let mut result = ResultOut::new();
    for (rank, &isel) in prio.iter().enumerate() {
        if !valid[isel] {
            continue;
        }
        // -oasel 0: highest-priority logo adds unconditionally (i==0),
        // the rest only when LogoResultAdd_check passes (autosel=1)
        let autosel = rank > 0;
        let (lres, lnum) = &res[isel];
        let added = result_add(&mut result, lres, *lnum, autosel);
        if !added {
            valid[isel] = false;
        }
    }
    (result, valid, totals)
}
