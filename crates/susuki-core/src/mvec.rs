//! Derived from chapter_exe (https://github.com/rigaya/chapter_exe,
//! commit 32880d45f088e574285a101e6a49b032bb04f6ea), Copyright (C) rigaya,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! REF-CHAPTER `mvec.cpp` port (design 6.2.2).
//! Normative: chapter_exe fixed commit 32880d45 src/mvec.cpp. Integer arithmetic
//! matches the reference exactly (SSE ops translated to scalar: _mm_sad_epu8 =
//! sum |a-b| over 16 bytes; max/min reductions equivalent).
//! Design 6.2.4 allows later internal optimizations as long as final results
//! stay within the 11.1 tolerance; this first cut is the strict port.
//!
//! 6.2.2 constants verified against source (spec-notes N5):
//! `FIELD_PICTURE` is a compile-time constant = 2 (mvec.cpp:22), so the mvec
//! `threshold` argument `(100-0)*(100/FIELD_PICTURE)` = 5000 for BOTH scan modes
//! (progressive `pict_struct=1` does not change it). Card-1 #5 retraction confirmed.

const MAX_LINEOBJ: usize = 20;
const MAX_SEARCH_EXTENT: i32 = 32;
const RATE_SCENE_CHANGE: i32 = 100;
const RATE_SCENE_CHGLOW: i32 = 50;
const RATE_SCENE_CHGBLK: i32 = 20;
const RATE_SCENE_CHGCBK: i32 = 8;
const RATE_SCENE_JDGBLK: i32 = 850;
const RATE_SCENE_STAY: i32 = 100;
const RATE_SCENE_STYLOW: i32 = 70;
const RATE_SCENE_STYBLK: i32 = 20;
const RATE_SCENE_STYCBK: i32 = 8;
const THRES_STILLDATA: i32 = 8;
const FIELD_PICTURE: i32 = 2;

/// Per-frame-pair features kept for the whole program (design 8.3).
#[derive(Debug, Clone, Copy)]
pub struct PairFeat {
    /// motion of the busier field side (+1 added by reference)
    pub mvec1: i32,
    /// motion of the calmer field side (+1)
    pub mvec2: i32,
    pub flag_sc: i32,
    pub rate_sc: i32,
}

/// Distinct per-pair mvec outputs; `pair_images` carry the reduced-luma frames
/// already prepared by design 6.0/6.1. `lx`,`ly` are the reduced dimensions
/// (mvec `lx`,`ly` arguments); strides use lx2 = lx*pict_struct (6.0 rule 4).
/// threshold = (100-0)*(100/FIELD_PICTURE) = 5000 (6.2.2).
pub fn mvec_pair(
    cur: &[u8],
    bef: &[u8],
    lx: usize,
    ly: usize,
    threshold: i32,
    pict_struct: i32,
    nframe: i32,
) -> PairFeat {
    mvec_pair_opt(cur, bef, lx, ly, threshold, pict_struct, nframe, None)
}

/// Block-origin table in the exact iteration order of the reference loops
/// (§6.2.2 field passes). Used to feed the §6.5 GPU path or a cached table.
pub fn block_offsets(lx: usize, ly: usize, pict_struct: i32) -> Vec<u32> {
    let lxi = lx as i32;
    let lyi = ly as i32;
    let mut out = Vec::new();
    for i in 0..pict_struct {
        let mut y = i + 16;
        while y < lyi - 16 {
            let row0 = (y as usize) * lx;
            let mut x = 16;
            let mut p = row0 + 16usize;
            while x < lxi - 16 {
                out.push(p as u32);
                x += 16;
                p += 16;
            }
            y += 16;
        }
    }
    out
}

/// mvec_pair with optional precomputed per-block avgdist table (§6.5 GPU
/// path feeds this; None = identical CPU computation as the reference).
/// 原典 mvec_pair の引数構成 + テーブル引数のため 8 個になる
#[allow(clippy::too_many_arguments)]
pub fn mvec_pair_opt(
    cur: &[u8],
    bef: &[u8],
    lx: usize,
    ly: usize,
    threshold: i32,
    pict_struct: i32,
    nframe: i32,
    pre: Option<&[crate::gpu::BlockStat]>,
) -> PairFeat {
    let lxi = lx as i32;
    let lyi = ly as i32;
    let lx2 = lxi * pict_struct;
    let block_height = 16 / pict_struct;

    let thr_blank = threshold / 100;
    let thr_noobj = threshold / 10;
    let thr_mergin = threshold / 8;

    // lane totals across the interlace fields (reference outputs add 1)
    let mut calc_total_lane_i0: i32 = 0;
    let mut calc_total_lane_i1: i32 = 0;
    let mut b_sc_all = 0i32;
    let mut rate_sc_all = 0i32;
    let mut kb = 0usize;

    for i in 0..pict_struct {
        let mut calc_total = 0i32;
        let mut areacnt_blankor = 0i32;
        let mut areacnt_blankand = 0i32;
        let mut areacnt_blank1 = 0i32;
        let mut areacnt_blank2 = 0i32;
        let mut areacnt_noobj1 = 0i32;
        let mut areacnt_noobj2 = 0i32;
        let mut cnt_total = 0i32;
        let mut cnt_sc = 0i32;
        let mut cnt_detobj = 0i32;
        let mut cnt_center_detobj = 0i32;
        let mut _cnt_blank = 0i32;
        let mut cnt_undet = 0i32;
        let mut cnt_undetobj = 0i32;
        let mut cnt_sc_low1 = 0i32;
        let mut cnt_sc_low2 = 0i32;
        let mut cnt_sc_center_low1 = 0i32;
        let mut cnt_sc_center_low2 = 0i32;
        let mut lineobj = [[0i32; MAX_LINEOBJ]; 4];

        let mut y = i + 16;
        while y < lyi - 16 {
            let row0 = (y as usize) * lx;
            let mut x: i32 = 16;
            let mut p1 = row0 + 16usize;
            let mut p2 = row0 + 16usize;
            while x < lxi - 16 {
                let mut center_area = 0;
                let mut invalid_th_fine = 0;
                let mut lowtype = 0;

                let mut val_calc: i32 = 0;

                if y >= lyi * 3 / 16
                    && y < lyi - (lyi * 3 / 16)
                    && x >= lxi * 3 / 16
                    && x < lxi - (lxi * 3 / 16)
                {
                    center_area = 1;
                }

                let (ddist1, avg1, ddist2, avg2) = match pre {
                    Some(t) => {
                        let st = t[kb];
                        kb += 1;
                        (st.ddist1, st.avg1, st.ddist2, st.avg2)
                    }
                    None => {
                        let (d1, a1) = avgdist(cur, p1, lx2 as usize, block_height);
                        let (d2, a2) = avgdist(bef, p2, lx2 as usize, block_height);
                        (d1, a1, d2, a2)
                    }
                };

                if ddist1 <= threshold && ddist2 > thr_blank && ddist1 * 2 <= ddist2 {
                    lowtype = 1;
                } else if ddist2 <= threshold && ddist1 > thr_blank && ddist2 * 2 <= ddist1 {
                    lowtype = 2;
                }
                if ddist1 <= thr_blank || ddist2 <= thr_blank {
                    areacnt_blankor += 1;
                    if ddist1 <= thr_blank {
                        areacnt_blank1 += 1;
                    }
                    if ddist2 <= thr_blank {
                        areacnt_blank2 += 1;
                    }
                    if ddist1 <= thr_blank && ddist2 <= thr_blank && (avg1 - avg2).abs() <= 5 {
                        areacnt_blankand += 1;
                    }
                }
                if ddist1 <= thr_noobj {
                    areacnt_noobj1 += 1;
                }
                if ddist2 <= thr_noobj {
                    areacnt_noobj2 += 1;
                }

                // (pc,pp) block origins + base dist
                let (pc, pp, ddist, base_off_c, base_off_p) = if lowtype == 1 {
                    (bef, cur, ddist2, p2, p1)
                } else {
                    (cur, bef, ddist1, p1, p2)
                };

                let mut th_fine = ddist * 3 / 5;
                if th_fine > threshold {
                    th_fine = threshold;
                }
                if th_fine <= thr_mergin {
                    th_fine = threshold;
                    invalid_th_fine = 1;
                }

                let n_sc = search_change(
                    &mut val_calc,
                    pc,
                    pp,
                    base_off_c,
                    base_off_p as isize,
                    lxi,
                    lyi,
                    x,
                    y,
                    th_fine,
                    threshold,
                    pict_struct,
                    lx2 as usize,
                    block_height,
                );

                if n_sc == 2 {
                    cnt_sc += 1;
                    if lowtype == 1 {
                        cnt_sc_low1 += 1;
                        if center_area == 1 {
                            cnt_sc_center_low1 += 1;
                        }
                    } else if lowtype == 2 {
                        cnt_sc_low2 += 1;
                        if center_area == 1 {
                            cnt_sc_center_low2 += 1;
                        }
                    }
                } else if ddist1 <= thr_blank && ddist2 <= thr_blank {
                    _cnt_blank += 1;
                } else if invalid_th_fine > 0 || n_sc > 0 {
                    cnt_undet += 1;
                    if ddist1 > thr_noobj || ddist2 > thr_noobj {
                        cnt_undetobj += 1;
                    }
                } else {
                    cnt_detobj += 1;
                    if center_area == 1 {
                        cnt_center_detobj += 1;
                    }
                    if center_area == 0 {
                        if y < lyi * 3 / 16 {
                            let idtmp = y / 16;
                            if (idtmp as usize) < MAX_LINEOBJ {
                                lineobj[0][idtmp as usize] += 1;
                            }
                        }
                        if y >= lyi - (lyi * 3 / 16) {
                            let idtmp = (lyi - y - 1) / 16;
                            if (idtmp as usize) < MAX_LINEOBJ {
                                lineobj[1][idtmp as usize] += 1;
                            }
                        }
                        if x < lxi * 3 / 16 {
                            let idtmp = x / 16;
                            if (idtmp as usize) < MAX_LINEOBJ {
                                lineobj[2][idtmp as usize] += 1;
                            }
                        }
                        if x >= lxi - (lxi * 3 / 16) {
                            let idtmp = (lxi - x - 1) / 16;
                            if (idtmp as usize) < MAX_LINEOBJ {
                                lineobj[3][idtmp as usize] += 1;
                            }
                        }
                    }
                }

                calc_total += val_calc;
                p1 += 16;
                p2 += 16;
                cnt_total += 1;
                x += 16;
            }
            y += 16;
        }

        let mut rate_sc = cnt_sc * 1000 / cnt_total;

        // fixed-line detection (reference: cnt_lineobj)
        let mut cnt_lineobj = 0i32;
        {
            let mut linecount_y = 0i32;
            for (k1, row) in lineobj.iter().enumerate().take(4) {
                let cnt_lineobj_max = if k1 < 2 { lxi / 16 } else { lyi / 16 };
                let mut cnttmp_max = 0i32;
                for cnttmp in row.iter().copied().take(MAX_LINEOBJ) {
                    if cnttmp > cnt_lineobj_max / 2 && cnttmp > cnttmp_max {
                        cnttmp_max = cnttmp;
                    }
                }
                if cnttmp_max > 1 {
                    cnt_lineobj += cnttmp_max;
                    if k1 < 2 {
                        linecount_y += 1;
                    } else {
                        cnt_lineobj -= linecount_y;
                    }
                }
            }
            if cnt_lineobj > 0 {
                cnt_detobj -= cnt_lineobj;
                cnt_total -= cnt_lineobj;
            }
        }

        if rate_sc < RATE_SCENE_CHANGE {
            let calc_jdgblk = cnt_total * RATE_SCENE_JDGBLK / 1000;
            let cnt_stayobj = cnt_detobj + cnt_undetobj;
            let mut flag_blankchk = false;

            if (areacnt_blankor >= calc_jdgblk)
                && (cnt_sc_low1 * 3 / 2 >= cnt_stayobj || cnt_sc_low2 * 3 / 2 >= cnt_stayobj)
            {
                flag_blankchk = true;
            }
            if (areacnt_blank1 >= calc_jdgblk || areacnt_blank2 >= calc_jdgblk)
                && (areacnt_blank1 - areacnt_blank2).abs() >= cnt_total / 3
            {
                flag_blankchk = true;
            }
            if (areacnt_noobj1 >= calc_jdgblk || areacnt_noobj2 >= calc_jdgblk)
                && (areacnt_noobj1 - areacnt_noobj2).abs() >= cnt_total / 2
            {
                flag_blankchk = true;
            }
            if flag_blankchk {
                let calc_blank_thres = cnt_total * RATE_SCENE_CHGBLK / 1000;
                if (cnt_sc_low1 - cnt_sc_low2 * 4 >= calc_blank_thres)
                    || (cnt_sc_low2 - cnt_sc_low1 * 4 >= calc_blank_thres)
                {
                    rate_sc = RATE_SCENE_CHANGE;
                }
                let calc_blank_thres_c = cnt_total * RATE_SCENE_CHGCBK / 1000;
                if (cnt_sc_center_low1 - cnt_sc_center_low2 * 4 >= calc_blank_thres_c)
                    || (cnt_sc_center_low2 - cnt_sc_center_low1 * 4 >= calc_blank_thres_c)
                {
                    rate_sc = RATE_SCENE_CHANGE;
                }
            }

            if cnt_sc - cnt_detobj / 2 >= cnt_total * RATE_SCENE_CHGLOW / 1000
                && cnt_detobj <= cnt_total * RATE_SCENE_CHGLOW / 1000
                && rate_sc < RATE_SCENE_CHANGE
                && (cnt_undet > cnt_total / 2
                    || (areacnt_blankor >= calc_jdgblk
                        && (cnt_sc - cnt_detobj + cnt_undet >= calc_jdgblk)))
            {
                rate_sc = RATE_SCENE_CHANGE;
            }
        }

        if cnt_detobj >= cnt_total * RATE_SCENE_STAY / 1000 {
            if cnt_center_detobj >= cnt_total * RATE_SCENE_STYCBK / 1000 {
                rate_sc /= 10;
            }
        } else if (RATE_SCENE_CHANGE..=RATE_SCENE_CHANGE * 3).contains(&rate_sc) {
            let calc_blank_dif = (areacnt_blank1 - areacnt_blank2).abs();
            let calc_blank_base = areacnt_blankand + calc_blank_dif;
            let calc_blank_rev = (calc_blank_base - cnt_total / 2) * 2 * RATE_SCENE_STAY / 1000;
            if areacnt_blankand >= cnt_total / 2
                && cnt_detobj >= cnt_total * RATE_SCENE_STYBLK / 1000
                && cnt_sc < calc_blank_dif + (cnt_total - calc_blank_base) / 2
                && ((cnt_detobj + calc_blank_rev >= cnt_total * RATE_SCENE_STAY / 1000)
                    || calc_blank_base > cnt_total * RATE_SCENE_JDGBLK / 1000)
                || cnt_detobj >= cnt_total * RATE_SCENE_STYLOW / 1000
                    && cnt_sc - cnt_detobj + cnt_total * RATE_SCENE_STYLOW / 1000
                        < cnt_total * RATE_SCENE_CHANGE / 1000
            {
                rate_sc = RATE_SCENE_CHANGE - 1;
            }
        }

        if (RATE_SCENE_CHANGE..=RATE_SCENE_CHANGE * 3).contains(&rate_sc)
            && cnt_sc + cnt_undetobj - cnt_detobj * 10 >= cnt_total / 2
        {
            rate_sc = RATE_SCENE_CHANGE * 3;
        }

        let b_sc = if rate_sc >= RATE_SCENE_CHANGE { 1 } else { 0 };

        if i == 0 {
            calc_total_lane_i0 = calc_total + 1;
            b_sc_all = b_sc;
            rate_sc_all = rate_sc;
        } else {
            calc_total_lane_i1 = calc_total + 1;
            if rate_sc_all < rate_sc {
                b_sc_all = b_sc;
                rate_sc_all = rate_sc;
            }
        }
    }

    let (mvec1, mvec2) = if pict_struct != FIELD_PICTURE {
        (calc_total_lane_i0, calc_total_lane_i0)
    } else if calc_total_lane_i0 >= calc_total_lane_i1 {
        (calc_total_lane_i0, calc_total_lane_i1)
    } else {
        (calc_total_lane_i1, calc_total_lane_i0)
    };

    let _ = nframe; // reference uses it only for debug printf
    PairFeat {
        mvec1,
        mvec2,
        flag_sc: b_sc_all,
        rate_sc: rate_sc_all,
    }
}

/// threshold constant used by the caller (6.2.2): (100-0)*(100/FIELD_PICTURE)
pub fn mvec_threshold() -> i32 {
    100 * (100 / FIELD_PICTURE)
}

/// REF mvec.cpp:455-501 search_change.
/// `co`/`bo` are block origins (byte indices) in cur/bef frames.
#[allow(clippy::too_many_arguments)]
fn search_change(
    val: &mut i32,
    pc: &[u8],
    pp: &[u8],
    co: usize,
    bo: isize,
    lxi: i32,
    lyi: i32,
    x: i32,
    y: i32,
    thres_fine: i32,
    thres_sc: i32,
    pict_struct: i32,
    lx2: usize,
    block_height: i32,
) -> i32 {
    let mut method = 0;
    let mut n_sc = 0;
    let mut vx = 0i32;
    let mut vy = 0i32;

    let mut min = dist(pc, co, pp, bo, lx2, i32::MAX, block_height);
    if min <= thres_fine {
        method = 2;
    }
    min = tree_search(
        pc,
        pp,
        co,
        bo,
        lxi,
        lyi,
        &mut vx,
        &mut vy,
        x,
        y,
        min,
        pict_struct,
        method,
        lx2,
        block_height,
    );
    if thres_sc < min {
        // C: full_search(pc, &pp[vy*lx+vx], ..., x+vx, y+vy, ...)
        let (sx, sy) = (vx, vy);
        let bo2 = bo + (sy as i64 * lxi as i64 + sx as i64) as isize;
        min = full_search(
            pc,
            pp,
            co,
            bo2,
            lxi,
            lyi,
            &mut vx,
            &mut vy,
            x + sx,
            y + sy,
            min,
            pict_struct,
            (sx.abs().max(sy.abs())) * 2,
            lx2,
            block_height,
        );
        if thres_sc < min {
            let mut vxe = 0i32;
            let mut vye = 0i32;
            min = tree_search(
                pc,
                pp,
                co,
                bo,
                lxi,
                lyi,
                &mut vxe,
                &mut vye,
                x,
                y,
                min,
                pict_struct,
                3,
                lx2,
                block_height,
            );
            if thres_sc < min {
                vx = MAX_SEARCH_EXTENT * 10;
                vy = MAX_SEARCH_EXTENT * 10;
                n_sc = 2;
            }
        }
    }
    if n_sc == 0 && min > thres_fine {
        n_sc = 1;
    }
    *val = vx.abs() + vy.abs();
    n_sc
}

/// REF mvec.cpp:504-633 tree_search. `co` block origin in cur, `bo` base
/// offset in bef (C `&bef_pix[...]`); dy/dx search offsets are relative to
/// the block origin, exactly like the C pointer arithmetic.
#[allow(clippy::too_many_arguments)]
fn tree_search(
    cur: &[u8],
    bef: &[u8],
    co: usize,
    bo: isize,
    lxi: i32,
    lyi: i32,
    vx: &mut i32,
    vy: &mut i32,
    search_block_x: i32,
    search_block_y: i32,
    mut min: i32,
    pict_struct: i32,
    method: i32,
    lx2: usize,
    block_height: i32,
) -> i32 {
    let mut ddx = 0i32;
    let mut ddy = 0i32;
    let speedup = pict_struct - 1;

    let ylow = 0 - search_block_y;
    let yhigh = lyi - search_block_y - 16;
    let xlow = 0 - search_block_x;
    let xhigh = lxi - search_block_x - 16;

    if method == 2 {
        return min;
    }

    // ys mirrors the C variable retaining the last executed `dy*lx`
    let mut ys: isize = 0;

    let (loopmax, inter0): (i32, i32) = match method {
        0 => (3 - speedup, 0),
        3 => (1, 0),
        _ => (5 - speedup, MAX_SEARCH_EXTENT),
    };
    let mut inter = inter0;

    for it in 0..loopmax {
        let locx;
        let locy;
        let nrep;
        let step;
        let dthres;
        if method == 0 {
            match it {
                0 => {
                    locx = MAX_SEARCH_EXTENT - 8;
                    locy = MAX_SEARCH_EXTENT - 8;
                    nrep = MAX_SEARCH_EXTENT / 8 * 2 - 1;
                    step = 8;
                    dthres = THRES_STILLDATA << 4;
                }
                1 => {
                    locx = ddx - 6;
                    locy = ddy - 6;
                    nrep = 7;
                    step = 2;
                    dthres = THRES_STILLDATA << 2;
                }
                _ => {
                    locx = ddx - 1;
                    locy = ddy - 1;
                    nrep = 3;
                    step = 1;
                    dthres = 1;
                }
            }
        } else if method == 3 {
            locx = ddx - MAX_SEARCH_EXTENT * 2;
            locy = ddy - MAX_SEARCH_EXTENT * 2;
            nrep = 5;
            step = MAX_SEARCH_EXTENT;
            dthres = THRES_STILLDATA << 4;
        } else {
            inter /= 2;
            locx = ddx - inter;
            locy = ddy - inter;
            nrep = 3;
            step = inter;
            dthres = THRES_STILLDATA << (loopmax - it - 1);
        }

        let mut dy = locy;
        for yy in 0..nrep {
            if dy < ylow || dy > yhigh {
            } else {
                ys = (dy as i64 * lxi as i64) as isize;
                let mut dx = locx;
                for xx in 0..nrep {
                    if dx < xlow || dx > xhigh {
                    } else if xx == (nrep - 1) / 2 && yy == (nrep - 1) / 2 {
                        // center: skip
                    } else {
                        let d = dist(cur, co, bef, bo + ys + dx as isize, lx2, min, block_height);
                        if d <= min
                            && ((d + dthres <= min)
                                || (dx.abs() + dy.abs() <= ddx.abs() + ddy.abs()))
                        {
                            min = d;
                            ddx = dx;
                            ddy = dy;
                        }
                    }
                    dx += step;
                }
            }
            dy += step;
        }
    }

    if pict_struct == FIELD_PICTURE {
        for k in 0..3i32 {
            let dx = ddx - 1 + 2 * k;
            if search_block_x + dx < 0 || search_block_x + dx + 16 > lxi {
                continue;
            }
            let d = dist(cur, co, bef, bo + ys + dx as isize, lx2, min, block_height);
            if d < min {
                min = d;
                ddx = dx;
            }
        }
    }

    *vx += ddx;
    *vy += ddy;
    min
}

/// REF mvec.cpp:635-692 full_search. The C center-distance test uses
/// `abs(dx)+abs(dy) <= abs(ddx)-abs(ddy)` verbatim (reference quirk preserved).
#[allow(clippy::too_many_arguments)]
fn full_search(
    cur: &[u8],
    bef: &[u8],
    co: usize,
    bo: isize,
    lxi: i32,
    lyi: i32,
    vx: &mut i32,
    vy: &mut i32,
    search_block_x: i32,
    search_block_y: i32,
    mut min: i32,
    pict_struct: i32,
    mut search_extent: i32,
    lx2: usize,
    block_height: i32,
) -> i32 {
    let mut ddx = 0i32;
    let mut ddy = 0i32;

    if search_extent > MAX_SEARCH_EXTENT {
        search_extent = MAX_SEARCH_EXTENT;
    }

    let ylow = 0 - if search_block_y - search_extent < 0 {
        search_block_y
    } else {
        search_extent
    };
    let yhigh = if search_block_y + search_extent + 16 > lyi {
        lyi - search_block_y - 16
    } else {
        search_extent
    };
    let xlow = 0 - if search_block_x - search_extent < 0 {
        search_block_x
    } else {
        search_extent
    };
    let xhigh = if search_block_x + search_extent + 16 > lxi {
        lxi - search_block_x - 16
    } else {
        search_extent
    };

    let dthres = THRES_STILLDATA;
    let mut dy = ylow;
    while dy <= yhigh {
        let row = (dy as i64 * lxi as i64) as isize;
        let mut dx = xlow;
        while dx <= xhigh {
            let d = dist(cur, co, bef, bo + row + dx as isize, lx2, min, block_height);
            if d <= min && ((d + dthres <= min) || (dx.abs() + dy.abs() <= ddx.abs() - ddy.abs())) {
                min = d;
                ddx = dx;
                ddy = dy;
            }
            dx += 1;
        }
        dy += pict_struct;
    }

    *vx += ddx;
    *vy += ddy;
    min
}

/// REF mvec.cpp:694-763 dist (SSE translated to scalar; early break kept).
/// Origins are signed to mirror C pointer arithmetic; for correctly sized
/// images every touched index is in bounds (debug_asserted).
fn dist(
    cur: &[u8],
    co: usize,
    bef: &[u8],
    bo: isize,
    lx2: usize,
    distlim: i32,
    block_height: i32,
) -> i32 {
    let bh = block_height as usize;
    if bh == 8 {
        let mut s: i32 = 0;
        for i in 0..8usize {
            let a = row16(cur, co as isize + (i * lx2) as isize);
            let b = row16(bef, bo + (i * lx2) as isize);
            s += sad16(&a, &b) as i32;
        }
        return s;
    }
    let mut s = 0i32;
    for i in 0..bh {
        let a = row16(cur, co as isize + (i * lx2) as isize);
        let b = row16(bef, bo + (i * lx2) as isize);
        s += sad16(&a, &b) as i32;
        if s > distlim {
            break;
        }
    }
    s
}

/// One 16-byte row for the block math. For correctly sized reference images
/// (w16/h16 multiples, bounds-checked search windows) every access is in
/// bounds; out-of-range reads never occur in the reference and are zero
/// filled here defensively (debug builds assert).
#[inline]
fn row16(img: &[u8], off: isize) -> [u8; 16] {
    debug_assert!(off >= 0 && (off as usize) + 16 <= img.len());
    let mut r = [0u8; 16];
    if off >= 0 {
        if let Some(s) = img.get(off as usize..off as usize + 16) {
            r.copy_from_slice(s);
        }
    }
    r
}

fn sad16(a: &[u8], b: &[u8]) -> u32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs())
        .sum()
}

/// REF mvec.cpp:842-873 avgdist. Returns (sum-of-abs-dev-from-mean, mean).
fn avgdist(src: &[u8], off: usize, lx2: usize, block_height: i32) -> (i32, i32) {
    let bh = block_height as usize;
    let mut sum: i64 = 0;
    for j in 0..bh {
        let row = &src[off + j * lx2..off + j * lx2 + 16];
        sum += row.iter().map(|&v| v as i64).sum::<i64>();
    }
    let d_avg: i32 = (((sum + (bh as i64 * 16 / 2)) / (bh as i64 * 16)) & 0xFF) as i32;
    let mut dev = 0i64;
    for j in 0..bh {
        let row = &src[off + j * lx2..off + j * lx2 + 16];
        dev += row
            .iter()
            .map(|&v| (v as i32 - d_avg).abs() as i64)
            .sum::<i64>();
    }
    dev &= u32::MAX as i64;
    (dev as i32, d_avg)
}
