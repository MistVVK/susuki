//! Derived from logoframe (https://github.com/tobitti0/logoframe,
//! commit c93aec0c89838e9b4dd3a9a3d5496dfaaa41311d), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Design 6.3.1 step 1: REF-LOGO `logoframe_det.c` setup port
//! (LogoRead_Chk_BS11 / LogoRead_Chk / LogoSet_mem / LogoSet_edge /
//! LogoSet_sumparam / LogoSet_sumparam_16x16 / LogoSet_revparam),
//! fixed commit c93aec0c. Integer semantics (1/16 luminance units, ConvLogo)
//! preserved verbatim.

use crate::logo::conv_logo;

use susuki_logo_file::LogoFile;

pub const LOGO_MAX_DP: i64 = 1000;
pub const LOGO_AREANUM: usize = 7;

/// LOGO_THRESREC (logoset.h). Default values are the design 6.3.2 table,
/// verified equal to the reference DEF_LOGO_* constants.
#[derive(Debug, Clone)]
pub struct Thres {
    pub rate_th_logo: i16,
    pub num_fadein: i16,
    pub num_fadeout: i16,
    pub num_cutleft: i16,
    pub num_cutright: i16,
    pub num_onwidth: i16,
    pub num_onlevel: i16,
    pub num_offwidth: i16,
    pub thres_ymax: i16,
    pub thres_ymin: i16,
    pub thres_yedge: i16,
    pub thres_ydif: i16,
    pub thres_ysetdif: i16,
    pub thres_yoffedg: i16,
    pub num_areaset: i16,
    pub num_clrrate: i16,
    pub auto_fade: i16,
    /// 1=undecided, 2=BS11, -1=non-BS11 (reference auto_bs11)
    pub auto_bs11: i16,
}

impl Default for Thres {
    fn default() -> Self {
        // logoset.h DEF_* (== design 6.3.2 table)
        Thres {
            rate_th_logo: 50,
            num_fadein: 0,
            num_fadeout: 0,
            num_cutleft: 500,
            num_cutright: 150,
            num_onwidth: 25,
            num_onlevel: 21,
            num_offwidth: 0,
            thres_ymax: 235,
            thres_ymin: 16,
            thres_yedge: 255,
            thres_ydif: 40,
            thres_ysetdif: 222,
            thres_yoffedg: 255,
            num_areaset: 0,
            num_clrrate: 8,
            auto_fade: 8,
            auto_bs11: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AreaParam {
    pub total_area: i64,
    pub total_area1: i64,
    pub total_area2: i64,
    pub xmin: i64,
    pub xmax: i64,
    pub ymin: i64,
    pub ymax: i64,
}

/// LOGO_PARAMREC
#[derive(Debug, Clone)]
pub struct Param {
    pub pos_x: i16,
    pub pos_y: i16,
    pub height: i16,
    pub width: i16,
    pub dp_y: Vec<i16>,
    /// logo luminance in 1/16 units (design 6.3.1 preprocessing)
    pub lum16: Vec<i16>,
    pub dif_y_col: Vec<i8>,
    pub dif_y_row: Vec<i8>,
    pub area_y: Vec<i16>,
    pub most_logo_y: i16,
    pub thres_dp_y: i16,
    pub total_dif: i64,
    pub total_dif_c1: i64,
    pub total_dif_c2: i64,
    pub num_local: i64,
    pub scale_area2: i16,
    pub area: [AreaParam; LOGO_AREANUM],
}

impl Param {
    fn new(w: i16, h: i16) -> Self {
        let n = (w as usize) * (h as usize);
        Param {
            pos_x: 0,
            pos_y: 0,
            height: h,
            width: w,
            dp_y: vec![0; n],
            lum16: vec![0; n],
            dif_y_col: vec![0; n],
            dif_y_row: vec![0; n],
            area_y: vec![0; n],
            most_logo_y: 0,
            thres_dp_y: 0,
            total_dif: 0,
            total_dif_c1: 0,
            total_dif_c2: 0,
            num_local: 0,
            scale_area2: 0,
            area: [AreaParam::default(); LOGO_AREANUM],
        }
    }
}

/// REF LogoRead_Chk_BS11 (logoframe_det.c:217-388): "BS11" station logo
/// glyph shape probe from the dp_y bitmap; drives auto yedge 220.
pub fn chk_bs11(logo: &LogoFile) -> bool {
    let lw = logo.width;
    let lh = logo.height;
    // reference: dp_y bitmap only (LOGO_MAX_DP*10/100 threshold)
    let mut state_col = 0i32;
    let mut col_ank = 0i32;
    let mut ytop = -1i32;
    let mut flag_row;
    for x in 0..lw {
        let mut row_st = 0i32;
        let mut row_ed = 0i32;
        let mut state_row = 0i32;
        let mut ncnt = 0i32;
        let mut loc_dp = x as usize;
        for y in 0..lh {
            let dp_y = i64::from(logo.dp_y[loc_dp]);
            loc_dp += lw as usize;
            let flag_dp_y = if dp_y >= LOGO_MAX_DP * 10 / 100 { 1 } else { 0 };
            if state_row == 0 {
                if flag_dp_y > 0 {
                    ncnt += 1;
                    if ncnt >= 3 {
                        state_row = 1;
                        row_st = y - ncnt + 1;
                        ncnt = 0;
                    }
                } else {
                    ncnt = 0;
                }
            } else if state_row == 1 {
                if flag_dp_y > 0 {
                    ncnt = 0;
                } else {
                    ncnt += 1;
                    if ncnt >= 3 {
                        state_row = 2;
                        row_ed = y - ncnt;
                        ncnt = 0;
                    }
                }
            } else if state_row == 2 {
                if flag_dp_y > 0 {
                    ncnt += 1;
                    if ncnt >= 3 {
                        state_row = 9;
                    }
                } else {
                    ncnt = 0;
                }
            }
        }
        if state_row == 1 {
            state_row = 2;
            row_ed = lh - ncnt - 1;
        }
        if state_row == 0 {
            flag_row = 1;
        } else if state_row == 2 {
            if ytop < 0 {
                ytop = row_st;
            }
            if (row_ed - row_st - 11).abs() < 5 && (row_st - ytop).abs() < 5 {
                flag_row = 2;
            } else if (row_ed - row_st - 50).abs() < 5 && (row_st - ytop).abs() < 5 {
                flag_row = 3;
            } else {
                flag_row = 0;
            }
        } else {
            flag_row = 0;
        }
        if state_col == 0 {
            ytop = -1;
            if flag_row == 2 {
                state_col = 1;
            }
        } else if state_col == 1 {
            if flag_row == 3 || flag_row == 0 {
                state_col = 2;
                col_ank = x;
            } else if flag_row != 2 {
                state_col = 0;
            }
        } else if state_col == 2 {
            if flag_row == 1 || flag_row == 0 {
                if (x - col_ank - 14).abs() < 3 {
                    state_col = 3;
                } else {
                    state_col = 0;
                }
            } else if flag_row != 3 {
                state_col = 0;
            }
        } else if state_col == 3 {
            if flag_row == 2 || flag_row == 0 {
                state_col = 4;
            } else if flag_row != 1 {
                state_col = 9;
            }
        } else if state_col == 4 {
            if flag_row == 3 || flag_row == 0 {
                if (x - col_ank - 28).abs() < 3 {
                    state_col = 5;
                    col_ank = x;
                } else {
                    state_col = 9;
                }
            } else if flag_row != 2 {
                state_col = 9;
            }
        } else if state_col == 5 {
            if flag_row == 1 || flag_row == 0 {
                if (x - col_ank - 13).abs() < 3 {
                    state_col = 6;
                } else {
                    state_col = 9;
                }
            } else if flag_row != 3 {
                state_col = 9;
            }
        } else if state_col == 6 && flag_row != 1 {
            state_col = 9;
        }
    }
    state_col == 6
}

/// REF LogoRead_Chk (391-410)
pub fn read_chk(thres: &mut Thres, logo: &LogoFile) {
    if thres.auto_bs11 > 0 {
        if chk_bs11(logo) {
            thres.auto_bs11 = 2;
            thres.thres_yedge = 220; // DEF_LOGO_AUTO_YEDGE
        } else {
            thres.auto_bs11 = -1;
        }
    }
}

/// REF LogoSet_mem (412-534): internal arrays + dp threshold + most logo Y.
pub fn set_mem(logo: &LogoFile, thres: &Thres) -> Param {
    let w = logo.width as i16;
    let h = logo.height as i16;
    let mut p = Param::new(w, h);
    p.pos_x = logo.x as i16;
    p.pos_y = logo.y as i16;
    let ymax = i64::from(thres.thres_ymax);
    let ymin = i64::from(thres.thres_ymin);
    let total_y = (w as usize) * (h as usize);
    for i in 0..total_y {
        p.dp_y[i] = logo.dp_y[i];
        // (y*(ymax-ymin)+128)/256 + (ymin<<4), 1/16-unit luminance
        p.lum16[i] = ((i64::from(logo.lum[i]) * (ymax - ymin) + 128) / 256 + (ymin << 4)) as i16;
    }

    // DP threshold from histogram of mid-dp pixels (412-467 semantics)
    let mut histdp = vec![0i64; LOGO_MAX_DP as usize];
    let mut num_total = 0i64;
    let mut num_total_tmp = 0i64;
    for i in 0..total_y {
        let dp = i64::from(p.dp_y[i]);
        if dp > LOGO_MAX_DP * 7 / 100 && dp < LOGO_MAX_DP * 85 / 100 {
            histdp[dp as usize] += 1;
            num_total_tmp += 1;
            if dp > LOGO_MAX_DP * 15 / 100 {
                num_total += 1;
            }
        }
    }
    if num_total < 500 {
        num_total = num_total_tmp;
    }
    let mut num = 0i64;
    let mut loc_dp = LOGO_MAX_DP * 10 / 100;
    let mut ncount = LOGO_MAX_DP * 3 / 100;
    let mut i = LOGO_MAX_DP * 85 / 100;
    while ncount > 0 && i > LOGO_MAX_DP * 7 / 100 {
        num += histdp[i as usize];
        if num >= num_total / 2 && num_total >= 200 {
            ncount -= 1;
            loc_dp = i;
        }
        i -= 1;
    }
    p.thres_dp_y = loc_dp as i16;

    let mut hist = [0i64; 256];
    for j in 0..total_y {
        if i64::from(p.dp_y[j]) >= loc_dp {
            let y = (i64::from(p.lum16[j]) + 8) >> 4;
            let y = y.clamp(0, 255);
            hist[y as usize] += 1;
        }
    }
    let mut loc_ymax = 1i64;
    let mut val_ymax = 0i64;
    for k in 1i64..=254 {
        let val = hist[(k - 1) as usize] + hist[k as usize] + hist[(k + 1) as usize];
        if val_ymax < val {
            val_ymax = val;
            loc_ymax = k;
        }
    }
    p.most_logo_y = loc_ymax as i16;
    p
}

/// REF LogoSet_edge (536-672): interlace-aware column/row edge masks; may
/// iterate lowering dif_thres until both field edge counts are sufficient.
pub fn set_edge(p: &mut Param) {
    let lw = i64::from(p.width);
    let lh = i64::from(p.height);
    let interval = 2i64;
    let mut dif_thres: i64 = 30;
    if i64::from(p.thres_dp_y) < LOGO_MAX_DP * 13 / 100 {
        dif_thres = 20;
    }
    dif_thres += 2;
    loop {
        dif_thres -= 2;
        let mut total_dif = 0i64;
        let mut total_dif_c1 = 0i64;
        let mut total_dif_c2 = 0i64;
        let mut interlace = i64::from(p.pos_y) % 2;
        // column edges
        for i in 0..lh {
            for j in 0..lw {
                let idx = (i * lw + j) as usize;
                let fdif: i8;
                if j >= lw - interval {
                    fdif = 0;
                } else {
                    let idx2 = (i * lw + j + interval) as usize;
                    let data0 = i64::from(p.lum16[idx]);
                    let data1 = i64::from(p.lum16[idx2]);
                    let d1 = conv_logo(0, i64::from(p.dp_y[idx]), data0)
                        - conv_logo(0, i64::from(p.dp_y[idx2]), data1);
                    let d2 = conv_logo(256 << 4, i64::from(p.dp_y[idx]), data0)
                        - conv_logo(256 << 4, i64::from(p.dp_y[idx2]), data1);
                    if d1.abs() > (dif_thres << 4) || d2.abs() > (dif_thres << 4) {
                        fdif = 1;
                        total_dif += 1;
                        if interlace == 0 {
                            total_dif_c1 += 1;
                        } else {
                            total_dif_c2 += 1;
                        }
                    } else {
                        fdif = 0;
                    }
                }
                p.dif_y_col[idx] = fdif;
            }
            interlace = 1 - interlace;
        }
        // row edges
        interlace = i64::from(p.pos_y) % 2;
        for i in 0..lh {
            if i >= lh - interval {
                for j in 0..lw {
                    p.dif_y_row[(i * lw + j) as usize] = 0;
                }
            } else {
                for j in 0..lw {
                    let idx = (i * lw + j) as usize;
                    let idx2 = ((i + interval) * lw + j) as usize;
                    let data0 = i64::from(p.lum16[idx]);
                    let data1 = i64::from(p.lum16[idx2]);
                    let d1 = conv_logo(0, i64::from(p.dp_y[idx]), data0)
                        - conv_logo(0, i64::from(p.dp_y[idx2]), data1);
                    let d2 = conv_logo(256 << 4, i64::from(p.dp_y[idx]), data0)
                        - conv_logo(256 << 4, i64::from(p.dp_y[idx2]), data1);
                    let fdif: i8;
                    if d1.abs() > (dif_thres << 4) || d2.abs() > (dif_thres << 4) {
                        fdif = 1;
                        total_dif += 1;
                        if interlace == 0 {
                            total_dif_c1 += 1;
                        } else {
                            total_dif_c2 += 1;
                        }
                    } else {
                        fdif = 0;
                    }
                    p.dif_y_row[idx] = fdif;
                }
            }
            interlace = 1 - interlace;
        }
        p.total_dif = total_dif;
        p.total_dif_c1 = total_dif_c1;
        p.total_dif_c2 = total_dif_c2;
        // C: do{...}while((c1<100 || c2<100) && dif_thres>16)
        if !((total_dif_c1 < 100 || total_dif_c2 < 100) && dif_thres > 16) {
            break;
        }
    }
}

/// REF LogoSet_sumparam_16x16 (673-849). Returns num_local.
fn set_sumparam_16x16(p: &mut Param) -> i64 {
    let lw = i64::from(p.width);
    let lh = i64::from(p.height);
    let interval = 2i64;
    let mut num_area = 0usize;
    let mut wk = [AreaParam::default(); 24];

    let mut bty = interval;
    while bty < lh - interval - 4 {
        let mut st_y = bty;
        let mut ed_y = st_y + 15;
        if ed_y >= lh - interval {
            let tmp = ed_y - (lh - interval) + 1;
            st_y -= tmp;
            ed_y -= tmp;
            if st_y < interval {
                st_y = interval;
            }
        }
        let mut btx = interval;
        while btx < lw - interval - 4 {
            let mut st_x = btx;
            let mut ed_x = st_x + 15;
            if ed_x >= lw - interval {
                let tmp = ed_x - (lw - interval) + 1;
                st_x -= tmp;
                ed_x -= tmp;
                if st_x < interval {
                    st_x = interval;
                }
            }
            let mut cnt_d1 = 0i64;
            let mut cnt_d2 = 0i64;
            for y in st_y..=ed_y {
                for x in st_x..=ed_x {
                    match p.area_y[(y * lw + x) as usize] {
                        1 => cnt_d1 += 1,
                        2 => cnt_d2 += 1,
                        _ => {}
                    }
                }
            }
            let cnt_d = cnt_d1.min(cnt_d2);
            if cnt_d > 4 {
                let mut pos = 0usize;
                let mut tmpcnt = wk[pos].total_area;
                while tmpcnt >= cnt_d && pos < num_area {
                    pos += 1;
                    if pos < num_area {
                        tmpcnt = wk[pos].total_area;
                    }
                }
                if pos < num_area {
                    let mut i = num_area;
                    while i > pos {
                        if i < 24 {
                            wk[i] = wk[i - 1];
                        }
                        i -= 1;
                    }
                }
                if pos < 24 {
                    wk[pos] = AreaParam {
                        total_area: cnt_d,
                        total_area1: cnt_d1,
                        total_area2: cnt_d2,
                        xmin: st_x,
                        xmax: ed_x,
                        ymin: st_y,
                        ymax: ed_y,
                    };
                    if num_area < 24 {
                        num_area += 1;
                    }
                }
            }
            btx += 8;
        }
        bty += 8;
    }

    // delete overlapped
    let mut cnt_area = num_area as i64;
    let mut pos = num_area as i64 - 1;
    while pos > 0 && cnt_area >= LOGO_AREANUM as i64 {
        let st_x = wk[pos as usize].xmin;
        let st_y = wk[pos as usize].ymin;
        let mut i = pos - 1;
        while i >= 0 {
            if (wk[i as usize].xmin - st_x).abs() <= 8 && (wk[i as usize].ymin - st_y).abs() <= 8 {
                wk[pos as usize].total_area = 0;
                cnt_area -= 1;
                i = 0;
            }
            i -= 1;
        }
        pos -= 1;
    }

    // set into area[1..]
    let mut cnt_final = 0i64;
    let mut pos = 0usize;
    for i in 1..LOGO_AREANUM {
        let mut tmpcnt;
        if pos < num_area {
            tmpcnt = wk[pos].total_area;
            while tmpcnt == 0 && pos < num_area {
                pos += 1;
                if pos < num_area {
                    tmpcnt = wk[pos].total_area;
                }
            }
        }
        if pos < num_area {
            cnt_final = i as i64;
            p.area[i] = wk[pos];
        } else {
            p.area[i] = AreaParam::default();
        }
        pos += 1;
    }
    cnt_final
}

/// REF LogoSet_sumparam (850-1175).
pub fn set_sumparam(p: &mut Param) {
    let lw = i64::from(p.width);
    let lh = i64::from(p.height);
    let interval = 2i64;
    let total = (lw * lh) as usize;
    // C advances pt_val6 = pt_val0+1 even at j=lw-1, reading one element past
    // the buffer (undefined garbage). Determinize (N8) with zero padding.
    let mut newval = vec![0i16; total + 2];

    let ylogo = i64::from(p.most_logo_y);
    let (ybase, ystep) = if ylogo > 180 {
        (ylogo - 40, 1i64)
    } else {
        (ylogo + 40, -1i64)
    };

    let mut k = 0usize;
    for _i in 0..lh {
        for _j in 0..lw {
            newval[k] = conv_logo(ybase << 4, i64::from(p.dp_y[k]), i64::from(p.lum16[k])) as i16;
            k += 1;
        }
    }

    let mut hista = vec![0i16; 256];
    let mut area_xmin = lw;
    let mut area_xmax = 0i64;
    let mut area_ymin = lh;
    let mut area_ymax = 0i64;
    let mut total_area1 = 0i64;
    let mut _total_area2 = 0i64;
    let val_invalid: i16 = -100;

    let mut numline = 0i64;
    for i in 0..lh {
        let (v1base, v2base) = if i < interval {
            (numline, numline + lw * interval)
        } else if i >= lh - interval {
            (numline - lw * interval, numline)
        } else {
            (numline - lw * interval, numline + lw * interval)
        };
        let _ = (v1base, v2base);
        for j in 0..lw {
            let idx = (numline + j) as usize;
            let (v1, v2) = if i < interval {
                (
                    newval[(numline + j) as usize],
                    newval[((numline + j) + lw * interval) as usize],
                )
            } else if i >= lh - interval {
                (
                    newval[((numline + j) - lw * interval) as usize],
                    newval[idx],
                )
            } else {
                (
                    newval[((numline + j) - lw * interval) as usize],
                    newval[((numline + j) + lw * interval) as usize],
                )
            };
            let (v3, v4) = if j < interval {
                (val_invalid, newval[(idx as i64 + interval) as usize])
            } else if j >= lw - interval {
                (newval[(idx as i64 - interval) as usize], val_invalid)
            } else {
                (
                    newval[(idx as i64 - interval) as usize],
                    newval[(idx as i64 + interval) as usize],
                )
            };
            let (v5, v6) = if j < 1 {
                (val_invalid, newval[idx + 1])
            } else if j >= lw {
                (newval[idx - 1], val_invalid)
            } else {
                (newval[idx - 1], newval[idx + 1])
            };
            let v0 = newval[idx];
            let farea: i16;
            if (i64::from(v0 - v1).abs() < (2 << 4))
                || (i64::from(v0 - v2).abs() < (2 << 4))
                || (i64::from(v0 - v3).abs() < (2 << 4))
                || (i64::from(v0 - v4).abs() < (2 << 4))
                || (i64::from(v0 - v5).abs() < (2 << 4) && i64::from(v0 - v6).abs() < (2 << 4))
            {
                if i64::from(v0 - (ybase << 4) as i16).abs() < (1 << 3)
                    && i64::from(p.dp_y[idx]) <= LOGO_MAX_DP / 200
                {
                    farea = 1;
                    total_area1 += 1;
                } else if (i64::from(p.lum16[idx]) - (ylogo << 4)).abs() <= (5 << 4)
                    || (i64::from(p.lum16[idx]) >= (255 << 4) && ylogo >= 254)
                {
                    farea = 2;
                    _total_area2 += 1;
                    let val0ch = (i64::from(v0) + 8) >> 4;
                    let val0ch = val0ch.clamp(0, 255);
                    // C uses signed short and wraps on overflow; keep wrap
                    // semantics with wrapping_add (reference quirk preserved).
                    hista[val0ch as usize] = hista[val0ch as usize].wrapping_add(1);
                } else {
                    farea = 0;
                }
            } else {
                farea = 0;
            }
            p.area_y[idx] = farea;
            if farea == 0 || farea == 2 {
                if area_xmin > j {
                    area_xmin = j;
                }
                if area_xmax < j {
                    area_xmax = j;
                }
                if area_ymin > i {
                    area_ymin = i;
                }
                if area_ymax < i {
                    area_ymax = i;
                }
            }
        }
        numline += lw;
    }

    // limit area2 to same level
    let mut val_max = 0i64;
    let mut loc_max = 0i64;
    let mut i = ybase + ystep * 3;
    while i != ylogo + ystep {
        let val_tmp = i64::from(hista[(i - 1) as usize])
            + i64::from(hista[i as usize])
            + i64::from(hista[(i + 1) as usize]);
        if (val_tmp >= 80 && val_max <= val_tmp * 2)
            || (val_tmp <= 80 && val_max * 9 <= val_tmp * 10)
        {
            loc_max = i;
            if val_max <= val_tmp {
                if (i - ybase).abs() < 5 {
                    val_max = val_tmp / (6 - (i - ybase).abs());
                } else {
                    val_max = val_tmp;
                }
            }
        }
        i += ystep;
    }
    let mut idx = 0usize;
    for _i in 0..lh {
        for _j in 0..lw {
            if p.area_y[idx] == 2
                && (i64::from(newval[idx]) < ((loc_max - 1) << 4) - (1 << 3)
                    || (i64::from(newval[idx]) >= ((loc_max + 1) << 4) + (1 << 3) && loc_max < 254))
            {
                p.area_y[idx] = 0;
                _total_area2 -= 1;
            }
            idx += 1;
        }
    }
    let _ = total_area1;

    // clamp to 48x48 best window
    if area_xmin > 0 {
        area_xmin -= 1;
    }
    if area_xmax < lw - 1 {
        area_xmax += 1;
    }
    if area_ymin > 0 {
        area_ymin -= 1;
    }
    if area_ymax < lh - 1 {
        area_ymax += 1;
    }
    let area_ywid = area_ymax - area_ymin + 1;
    let area_xwid = area_xmax - area_xmin + 1;
    let mut cnt_dmax = -1i64;
    for i in 0..5 {
        if i == 2 || area_ywid >= 48 + 8 {
            let (st_y, ed_y) = if area_ywid > 48 {
                let s = (area_ywid - 48) * i / 4 + area_ymin;
                (s, s + 48 - 1)
            } else {
                (area_ymin, area_ymax)
            };
            for j in 0..5 {
                if j == 2 || area_xwid >= 48 + 8 {
                    let (st_x, ed_x) = if area_xwid > 48 {
                        let s = (area_xwid - 48) * j / 4 + area_xmin;
                        (s, s + 48 - 1)
                    } else {
                        (area_xmin, area_xmax)
                    };
                    let mut cnt_d1 = 0i64;
                    let mut cnt_d2 = 0i64;
                    for y in st_y..=ed_y {
                        for x in st_x..=ed_x {
                            match p.area_y[(y * lw + x) as usize] {
                                1 => cnt_d1 += 1,
                                2 => cnt_d2 += 1,
                                _ => {}
                            }
                        }
                    }
                    let cnt_d = cnt_d1.min(cnt_d2);
                    if cnt_d > cnt_dmax {
                        cnt_dmax = cnt_d;
                        p.area[0] = AreaParam {
                            total_area: cnt_d,
                            total_area1: cnt_d1,
                            total_area2: cnt_d2,
                            xmin: st_x,
                            xmax: ed_x,
                            ymin: st_y,
                            ymax: ed_y,
                        };
                    }
                }
            }
        }
    }

    // NOTE: C calls LogoSet_sumparam_16x16 BEFORE the (farea&0x3) select
    // tagging loop; num_local semantics = returned (num_area-1) from that fn.
    let num_local = set_sumparam_16x16(p);

    for i in 0..lh {
        for j in 0..lw {
            let idx = (i * lw + j) as usize;
            let farea = p.area_y[idx];
            if (farea & 0x3) != 0 {
                for k2 in 0..=num_local {
                    let a = p.area[k2 as usize];
                    if j >= a.xmin && j <= a.xmax && i >= a.ymin && i <= a.ymax {
                        p.area_y[idx] |= 1 << (k2 + 8);
                    }
                }
                if (p.area_y[idx] & !0xFFi16) == 0 {
                    p.area_y[idx] = 0;
                }
            }
        }
    }

    p.num_local = num_local;
    p.scale_area2 = loc_max as i16;
}

/// REF LogoSet_revparam (1176-1197): lower thres_ydif by the ysetdif-derived
/// contrast of the most logo level (min DEF_LOGO_AUTO_YDIFMIN=24).
pub fn set_revparam(thres: &mut Thres, p: &Param) {
    let data_src = i64::from(thres.thres_ysetdif) * 16;
    let data_base = i64::from(p.most_logo_y) * 16;
    if data_src > 0 {
        let data_dst = conv_logo(data_src, i64::from(p.thres_dp_y), data_base);
        let mut data_dif = (data_src - data_dst).abs();
        if data_dif < 24 {
            // DEF_LOGO_AUTO_YDIFMIN
            data_dif = 24;
        }
        if i64::from(thres.thres_ydif) > data_dif {
            thres.thres_ydif = data_dif as i16;
        }
    }
}

/// REF LogoRead (1202-1234): full setup pipeline over one logo file.
pub fn logo_setup(logo: &LogoFile, thres: &mut Thres) -> Param {
    read_chk(thres, logo);
    let mut p = set_mem(logo, thres);
    set_edge(&mut p);
    set_sumparam(&mut p);
    set_revparam(thres, &p);
    p
}
