//! Derived from logoframe (https://github.com/tobitti0/logoframe,
//! commit c93aec0c89838e9b4dd3a9a3d5496dfaaa41311d), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Design 6.3.1 step 2-3 per-frame calculation: REF-LOGO
//! `logoframe_det.c` LogoCalc chain port (fixed commit c93aec0c).
//! LogoCalc_getdif_pix / _areasum / _vccalc / _vcresult / _getdif /
//! _summary / _areasummary_getloc / _areasummary_one / _areasummary /
//! _savedata. Integer semantics (long/i16, truncating division) preserved;
//! the reference's recursion argument quirk is kept verbatim.

use crate::logo::{conv_inv_logo, conv_logo};
use crate::logo_set::{Param, LOGO_AREANUM, LOGO_MAX_DP};

pub const LOGO_FADE_STEP: i64 = 31; // LOGO_FADE_STEP
pub const LOGO_FADE_OFST: i64 = 2; // LOGO_FADE_OFST
pub const LOGO_FADE_MAXLEVEL: usize = 36; // STEP + OFST*2 + 1
pub const THRES_DPYSMIN: i64 = 100; // const in getdif_pix

#[derive(Debug, Clone, Copy)]
pub struct AreaCalc {
    pub hista_areaoff: [i64; 256],
    pub hista_areaon: [i64; 256],
    pub hista_areacal: [i64; 256],
    pub num_hista_off: i64,
    pub num_hista_on: i64,
    pub rate_areavalid: i16,
    pub rate_arealogo: i16,
    pub diff_arealogo: i16,
    pub diff1_arealogo: i16,
    pub diff2_arealogo: i16,
    pub vari_arealogo_off: i16,
    pub vari_arealogo_cal: i16,
    pub vari_arealogo_on: i16,
}

impl Default for AreaCalc {
    fn default() -> Self {
        AreaCalc {
            hista_areaoff: [0; 256],
            hista_areaon: [0; 256],
            hista_areacal: [0; 256],
            num_hista_off: 0,
            num_hista_on: 0,
            rate_areavalid: 0,
            rate_arealogo: 0,
            diff_arealogo: 0,
            diff1_arealogo: 0,
            diff2_arealogo: 0,
            vari_arealogo_off: 0,
            vari_arealogo_cal: 0,
            vari_arealogo_on: 0,
        }
    }
}

/// LOGO_CALCREC
#[derive(Debug, Clone)]
pub struct Calc {
    pub hist_y: [i64; LOGO_FADE_MAXLEVEL],
    pub fade_calcstep: i64,
    pub cnt_logooff: i64,
    pub cnt_logoon: i64,
    pub cnt_logomv: i64,
    pub cnt_logovc: i64,
    pub cntf_logooff: i64,
    pub cntf_logoon: i64,
    pub cntf_logost: i64,
    pub cntf_logovc: i64,
    pub cntopp_logoon: i64,
    pub sum_areaoff: i64,
    pub sum_areaon: i64,
    pub sum_areadif: i64,
    pub sum_areanum: i64,
    pub cnt_offedg: i64,
    pub cntf_offedg: i64,
    pub cnts_logooff: i64,
    pub cnts_logoon: i64,
    pub total_dif: i64,
    pub vc_cntd_logooff: i64,
    pub vc_cntd_logoon: i64,
    pub vc_cntf_logooff: i64,
    pub vc_cntf_logoon: i64,
    pub vc_cntd_voff: i64,
    pub vc_cntd_von: i64,
    pub vc_cntf_voff: i64,
    pub vc_cntf_von: i64,
    pub rate_logoon: i16,
    pub rate_fade: i16,
    pub flag_nosample: i16,
    pub flag_nofadesm: i16,
    pub rank_unclear: i16,
    pub rate_arealogo: i16,
    pub area: [AreaCalc; LOGO_AREANUM],
    pub rate_areaavg_t: i16,
    pub sum_areanum_t: i64,
}

/// LogoCalc_clear (1283-1349): memset + fade histogram step setting.
pub fn clear(num_fadein: i16, num_fadeout: i16) -> Calc {
    let mut num_fade = i64::from(if num_fadein >= num_fadeout {
        num_fadein
    } else {
        num_fadeout
    });
    if num_fade == 0 || num_fade >= LOGO_FADE_STEP {
        num_fade = LOGO_FADE_STEP - 1;
    } else if num_fade < 5 {
        num_fade *= 2;
    }
    Calc {
        hist_y: [0; LOGO_FADE_MAXLEVEL],
        fade_calcstep: num_fade + 1,
        cnt_logooff: 0,
        cnt_logoon: 0,
        cnt_logomv: 0,
        cnt_logovc: 0,
        cntf_logooff: 0,
        cntf_logoon: 0,
        cntf_logost: 0,
        cntf_logovc: 0,
        cntopp_logoon: 0,
        sum_areaoff: 0,
        sum_areaon: 0,
        sum_areadif: 0,
        sum_areanum: 0,
        cnt_offedg: 0,
        cntf_offedg: 0,
        cnts_logooff: 0,
        cnts_logoon: 0,
        total_dif: 0,
        vc_cntd_logooff: 0,
        vc_cntd_logoon: 0,
        vc_cntf_logooff: 0,
        vc_cntf_logoon: 0,
        vc_cntd_voff: 0,
        vc_cntd_von: 0,
        vc_cntf_voff: 0,
        vc_cntf_von: 0,
        rate_logoon: 0,
        rate_fade: 0,
        flag_nosample: 0,
        flag_nofadesm: 0,
        rank_unclear: 0,
        rate_arealogo: 0,
        area: [AreaCalc::default(); LOGO_AREANUM],
        rate_areaavg_t: 0,
        sum_areanum_t: 0,
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Vc {
    pub cntd_last_on: i64,
    pub cntd_last_off: i64,
    pub cntf_last_on: i64,
    pub cntf_last_off: i64,
}

#[allow(clippy::too_many_arguments)]
/// 末尾再帰（flag_chkopp 時の exe_type=1 経路）を呼ぶかを示す
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PixTail {
    pub call: bool,
}

/// §6.5: getdif_pix の意思決定を Calc 書き込みイベントへ分離したもの。
/// CPU 実装と GPU コンピュートが同一イベント列を生成し、適用のみ CPU で
/// 共通化する（加算・ヒストグラミ追加のみなので元の逐次結果と一致する）。
pub const PIX_CNTS_OFF: u32 = 1 << 0;
pub const PIX_CNTS_ON: u32 = 1 << 1;
pub const PIX_CNT_OFF: u32 = 1 << 2;
pub const PIX_CNT_ON: u32 = 1 << 3;
pub const PIX_CNT_MV: u32 = 1 << 4;
pub const PIX_CNTF_OFF: u32 = 1 << 5;
pub const PIX_CNTF_ON: u32 = 1 << 6;
pub const PIX_CNTF_ST: u32 = 1 << 7;
pub const PIX_CNTOPP_ON: u32 = 1 << 8;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct PixEval {
    pub mask: u32,
    /// hist_y index; -1 = none
    pub hist: i32,
    /// cnt_offedg / cntf_offedg の増分量 (0/1)
    pub offedg: i64,
    pub sum_valid: bool,
    pub sum_off: i64,
    pub sum_on: i64,
    pub sum_dif: i64,
}

/// PixEval を Calc へ適用（getdif_pix_core の元の書き込み順と同一意味）
pub fn apply_pix_eval(c: &mut Calc, e: &PixEval) {
    if e.hist >= 0 {
        c.hist_y[e.hist as usize] += 1;
    }
    if e.mask & PIX_CNTS_OFF != 0 {
        c.cnts_logooff += 1;
    }
    if e.mask & PIX_CNTS_ON != 0 {
        c.cnts_logoon += 1;
    }
    if e.mask & PIX_CNT_OFF != 0 {
        c.cnt_logooff += 1;
        c.cnt_offedg += e.offedg;
    }
    if e.mask & PIX_CNT_ON != 0 {
        c.cnt_logoon += 1;
    }
    if e.mask & PIX_CNT_MV != 0 {
        c.cnt_logomv += 1;
    }
    if e.mask & PIX_CNTF_OFF != 0 {
        c.cntf_logooff += 1;
        c.cntf_offedg += e.offedg;
    }
    if e.mask & PIX_CNTF_ON != 0 {
        c.cntf_logoon += 1;
    }
    if e.mask & PIX_CNTF_ST != 0 {
        c.cntf_logost += 1;
    }
    if e.mask & PIX_CNTOPP_ON != 0 {
        c.cntopp_logoon += 1;
    }
    if e.sum_valid {
        c.sum_areaoff += e.sum_off;
        c.sum_areaon += e.sum_on;
        c.sum_areadif += e.sum_dif;
        c.sum_areanum += 1;
    }
}

/// LogoCalc_getdif_areasum の判定をエリア別イベントへ分離（§6.5 GPU 用）。
/// mode: 0=none 1=off-hist 2=on/cal-hist; bits=area i に相当するビット (1<<(i+8))
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct AreaEval {
    pub mode: u32,
    pub bits: u32,
    pub data0: u32,
    pub cal: u32,
}

pub fn eval_areasum(
    data0: u8,
    dp_y0: i64,
    y0: i64,
    area_y0: i64,
    thres_ymax: i64,
    most_logo_y: i64,
) -> AreaEval {
    if (area_y0 & 0x3) == 1 {
        AreaEval {
            mode: 1,
            bits: (area_y0 >> 8) as u32,
            data0: data0 as u32,
            cal: 0,
        }
    } else if (area_y0 & 0x3) == 2 {
        let d0_s = i64::from(data0) << 4;
        let mut d0_rev = conv_inv_logo(d0_s, dp_y0, y0);
        d0_rev = d0_rev.clamp(1 << 4, 254 << 4);
        let mut d0r = (d0_rev + 8) / 16;
        if i64::from(data0) > thres_ymax && i64::from(data0) > most_logo_y {
            d0r = i64::from(data0);
        }
        AreaEval {
            mode: 2,
            bits: (area_y0 >> 8) as u32,
            data0: data0 as u32,
            cal: d0r as u32,
        }
    } else {
        AreaEval::default()
    }
}

pub fn apply_area_eval(c: &mut Calc, e: &AreaEval) {
    if e.mode == 0 {
        return;
    }
    for i in 0..LOGO_AREANUM {
        if (i64::from(e.bits) & (1 << i)) == 0 {
            continue;
        }
        if e.mode == 1 {
            c.area[i].hista_areaoff[e.data0 as usize] += 1;
            c.area[i].num_hista_off += 1;
        } else {
            c.area[i].hista_areaon[e.data0 as usize] += 1;
            c.area[i].num_hista_on += 1;
            c.area[i].hista_areacal[e.cal as usize] += 1;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn getdif_pix_core(
    c: &mut Calc,
    data0: u8,
    data1: u8,
    dp_y0: i64,
    y0: i64,
    dp_y1: i64,
    y1: i64,
    thres_ymax: i64,
    thres_yedge: i64,
    thres_ydif: i64,
    thres_yoffedg: i64,
    most_logo_y: i64,
    exe_type: i64,
) -> PixEval2 {
    let fade_calcstep = c.fade_calcstep;
    // C: d0_s short <<0..4; all i64 here (no overflow at 8bit input scale)
    let d0_src = i64::from(data0) << 4;
    let d0_rev = conv_inv_logo(d0_src, dp_y0, y0);
    let d1_rev = conv_logo(d0_rev, dp_y1, y1);
    let d0_rev = d0_rev.clamp(0, 255 << 4);
    let d1_rev = d1_rev.clamp(0, 255 << 4);
    let mut d1 = i64::from(data1);
    let mut d1s = i64::from(data0);
    let mut d1r = (d1_rev + 8) >> 4;
    if d1 > thres_ymax {
        d1 = thres_ymax;
    }
    if d1s > thres_ymax {
        d1s = thres_ymax;
    }
    if d1r > thres_ymax {
        d1r = thres_ymax;
    }

    let oppside = if (d1 < d1s && d1s < d1r) || (d1 > d1s && d1s > d1r) {
        1
    } else {
        0
    };
    let val_offedg = if i64::from(data0) > thres_yoffedg || i64::from(data1) > thres_yoffedg {
        1
    } else {
        0
    };
    let d1s_fine = d1s << 4;
    let mut d1r_fine = d1_rev;
    if d1r_fine > thres_ymax << 4 {
        d1r_fine = thres_ymax << 4;
    }
    let d1dif_fine = d1r_fine - d1s_fine;

    let d0calc_adif = (d0_src - d0_rev).abs();
    let d1calc_adif = (d0_rev - d1_rev).abs();
    let d0calc_much = i64::from(d0calc_adif * 2 >= d1calc_adif && d0calc_adif >= (1 << 4));
    let d1calc_much = i64::from(d1calc_adif * 2 >= d0calc_adif && d1calc_adif >= (1 << 4));
    let most_logo_adif_y0 = (most_logo_y - ((y0 + 8) >> 4)).abs();
    let most_logo_adif_y1 = (most_logo_y - ((y1 + 8) >> 4)).abs();

    let d1dif = d1r - d1s;
    let mut e = PixEval::default();
    let mut flag_chkopp = 0i64;
    pix_branches(
        data0,
        data1,
        dp_y0,
        dp_y1,
        thres_ymax,
        thres_yedge,
        thres_ydif,
        thres_yoffedg,
        most_logo_y,
        exe_type,
        fade_calcstep,
        d0_src,
        d0_rev,
        d1_rev,
        d1,
        d1s,
        d1r,
        oppside,
        val_offedg,
        d1dif_fine,
        d0calc_adif,
        d1calc_adif,
        d0calc_much,
        d1calc_much,
        most_logo_adif_y0,
        most_logo_adif_y1,
        d1dif,
        &mut e,
        &mut flag_chkopp,
    );
    apply_pix_eval(c, &e);
    PixEval2 {
        e,
        tail: PixTail {
            call: exe_type == 0 && flag_chkopp > 0,
        },
    }
}

/// §6.5 GPU 用: pix 判定イベントのみ（Calc 適用なし）を返す純関数。
/// 判定ツリーは getdif_pix_core と同一（fade_calcstep は引数で渡す）。
#[allow(clippy::too_many_arguments)]
pub fn getdif_pix_eval(
    data0: u8,
    data1: u8,
    dp_y0: i64,
    y0: i64,
    dp_y1: i64,
    y1: i64,
    thres_ymax: i64,
    thres_yedge: i64,
    thres_ydif: i64,
    thres_yoffedg: i64,
    most_logo_y: i64,
    exe_type: i64,
    fade_calcstep: i64,
) -> PixEval2 {
    let d0_src = i64::from(data0) << 4;
    let d0_rev0 = conv_inv_logo(d0_src, dp_y0, y0);
    let d1_rev0 = conv_logo(d0_rev0, dp_y1, y1);
    let d0_rev = d0_rev0.clamp(0, 255 << 4);
    let d1_rev = d1_rev0.clamp(0, 255 << 4);
    let mut d1 = i64::from(data1);
    let mut d1s = i64::from(data0);
    let mut d1r = (d1_rev + 8) >> 4;
    if d1 > thres_ymax {
        d1 = thres_ymax;
    }
    if d1s > thres_ymax {
        d1s = thres_ymax;
    }
    if d1r > thres_ymax {
        d1r = thres_ymax;
    }
    let oppside = i64::from((d1 < d1s && d1s < d1r) || (d1 > d1s && d1s > d1r));
    let val_offedg =
        i64::from(i64::from(data0) > thres_yoffedg || i64::from(data1) > thres_yoffedg);
    let d1s_fine = d1s << 4;
    let mut d1r_fine = d1_rev;
    if d1r_fine > thres_ymax << 4 {
        d1r_fine = thres_ymax << 4;
    }
    let d1dif_fine = d1r_fine - d1s_fine;
    let d0calc_adif = (d0_src - d0_rev).abs();
    let d1calc_adif = (d0_rev - d1_rev).abs();
    let d0calc_much = i64::from(d0calc_adif * 2 >= d1calc_adif && d0calc_adif >= (1 << 4));
    let d1calc_much = i64::from(d1calc_adif * 2 >= d0calc_adif && d1calc_adif >= (1 << 4));
    let most_logo_adif_y0 = (most_logo_y - ((y0 + 8) >> 4)).abs();
    let most_logo_adif_y1 = (most_logo_y - ((y1 + 8) >> 4)).abs();
    let d1dif = d1r - d1s;
    let mut e = PixEval::default();
    let mut flag_chkopp = 0i64;
    pix_decide(
        data0,
        data1,
        dp_y0,
        y0,
        dp_y1,
        y1,
        thres_ymax,
        thres_yedge,
        thres_ydif,
        thres_yoffedg,
        most_logo_y,
        exe_type,
        fade_calcstep,
        d0_src,
        d0_rev,
        d1_rev,
        d1,
        d1s,
        d1r,
        oppside,
        val_offedg,
        d1s_fine,
        d1r_fine,
        d1dif_fine,
        d0calc_adif,
        d1calc_adif,
        d0calc_much,
        d1calc_much,
        most_logo_adif_y0,
        most_logo_adif_y1,
        d1dif,
        &mut e,
        &mut flag_chkopp,
    );
    PixEval2 {
        e,
        tail: PixTail {
            call: exe_type == 0 && flag_chkopp > 0,
        },
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct PixEval2 {
    pub e: PixEval,
    pub tail: PixTail,
}

#[allow(clippy::too_many_arguments)]
fn pix_decide(
    data0: u8,
    data1: u8,
    dp_y0: i64,
    _y0: i64,
    dp_y1: i64,
    _y1: i64,
    thres_ymax: i64,
    thres_yedge: i64,
    thres_ydif: i64,
    thres_yoffedg: i64,
    most_logo_y: i64,
    exe_type: i64,
    fade_calcstep: i64,
    d0_src: i64,
    d0_rev: i64,
    d1_rev: i64,
    d1: i64,
    d1s: i64,
    d1r: i64,
    oppside: i64,
    val_offedg: i64,
    _d1s_fine: i64,
    _d1r_fine: i64,
    d1dif_fine: i64,
    d0calc_adif: i64,
    d1calc_adif: i64,
    d0calc_much: i64,
    d1calc_much: i64,
    most_logo_adif_y0: i64,
    most_logo_adif_y1: i64,
    d1dif: i64,
    e: &mut PixEval,
    flag_chkopp: &mut i64,
) {
    // 本体分岐（core と同一のイベント生成）は core 側から参照されるため
    // ここでは未使用引数経由で受け取った中間値を用いる。
    pix_branches(
        data0,
        data1,
        dp_y0,
        dp_y1,
        thres_ymax,
        thres_yedge,
        thres_ydif,
        thres_yoffedg,
        most_logo_y,
        exe_type,
        fade_calcstep,
        d0_src,
        d0_rev,
        d1_rev,
        d1,
        d1s,
        d1r,
        oppside,
        val_offedg,
        d1dif_fine,
        d0calc_adif,
        d1calc_adif,
        d0calc_much,
        d1calc_much,
        most_logo_adif_y0,
        most_logo_adif_y1,
        d1dif,
        e,
        flag_chkopp,
    );
}

#[allow(clippy::too_many_arguments)]
fn pix_branches(
    data0: u8,
    data1: u8,
    dp_y0: i64,
    dp_y1: i64,
    thres_ymax: i64,
    thres_yedge: i64,
    thres_ydif: i64,
    thres_yoffedg: i64,
    _most_logo_y: i64,
    exe_type: i64,
    fade_calcstep: i64,
    d0_src: i64,
    _d0_rev: i64,
    d1_rev: i64,
    d1: i64,
    d1s: i64,
    d1r: i64,
    oppside: i64,
    val_offedg: i64,
    d1dif_fine: i64,
    d0calc_adif: i64,
    d1calc_adif: i64,
    d0calc_much: i64,
    d1calc_much: i64,
    most_logo_adif_y0: i64,
    most_logo_adif_y1: i64,
    d1dif: i64,
    e: &mut PixEval,
    flag_chkopp: &mut i64,
) {
    if (i64::from(data0) > thres_ymax && i64::from(data1) > thres_ymax && d1r >= thres_ymax - 1)
        || (i64::from(data0) > thres_yedge
            && i64::from(data1) > thres_yedge
            && d1r > thres_yedge - 10)
        || (d1dif.abs() < 10
            && ((d0calc_adif >= (1 << 4) && most_logo_adif_y0 > 2)
                || (d1calc_adif >= (1 << 4) && most_logo_adif_y1 > 2)))
    {
        // skip — 原典の独立腕を同一空 body のまま順序維持で統合
    } else if ((d1dif.abs() < 14 || dp_y0 < THRES_DPYSMIN)
        && d0calc_much > 0
        && most_logo_adif_y0 > 32)
        || ((d1dif.abs() < 14 || dp_y1 < THRES_DPYSMIN)
            && d1calc_much > 0
            && most_logo_adif_y1 > 32)
    {
        if exe_type == 0 && thres_yoffedg >= 255 {
            if (d1 - d1s).abs() < 4 {
                e.mask |= PIX_CNTS_OFF;
            } else if (d1 - d1r).abs() < 5 {
                e.mask |= PIX_CNTS_ON;
            }
        }
    } else if (d1 - d1s).abs() < 4
        && thres_yoffedg < 255
        && ((dp_y0 < THRES_DPYSMIN && d0calc_adif >= (1 << 4) && most_logo_adif_y0 > 32)
            || (dp_y1 < THRES_DPYSMIN && d0calc_adif >= (1 << 4) && most_logo_adif_y1 > 32))
    {
        // skip (nologo)
    } else if d1dif.abs() >= 10 {
        if exe_type == 0 {
            let val = ((d1 - d1s) * fade_calcstep + d1dif / 2) / d1dif + LOGO_FADE_OFST;
            if (0..LOGO_FADE_MAXLEVEL as i64).contains(&val) {
                e.hist = val as i32;
            }
            if (d1 - d1s).abs() < 4 {
                e.mask |= PIX_CNT_OFF;
                e.offedg = val_offedg;
            } else if (d1 - d1r).abs() < 5 {
                e.mask |= PIX_CNT_ON;
            } else {
                e.mask |= PIX_CNT_MV;
                if oppside == 1 {
                    *flag_chkopp = 1;
                }
            }
        }
    } else {
        if d1dif.abs() >= 6 {
            if exe_type == 0 {
                if (d1 - d1s).abs() < 3 {
                    e.mask |= PIX_CNTF_OFF;
                    e.offedg = val_offedg;
                } else if (d1 - d1r).abs() < 3 {
                    e.mask |= PIX_CNTF_ON;
                } else if oppside == 1 {
                    *flag_chkopp = 1;
                }
            } else if exe_type == 1 && (d1 - d1r).abs() < 3 {
                e.mask |= PIX_CNTOPP_ON;
            }
        } else if d1dif_fine.abs() >= thres_ydif && d1dif_fine.abs() >= 56 {
            if exe_type == 0 {
                if (d1 - d1s).abs() <= 0 && (d1 - d1r).abs() >= 3 {
                    e.mask |= PIX_CNTF_OFF;
                    e.offedg = val_offedg;
                } else if (d1 - d1s).abs() >= 2 && (d1 - d1r).abs() <= 3 {
                    e.mask |= PIX_CNTF_ON;
                } else if oppside == 1 {
                    *flag_chkopp = 1;
                }
            } else if exe_type == 1 {
                if (d1 - d1s).abs() <= 0 && (d1 - d1r).abs() >= 3 {
                    // reference: no action
                } else if (d1 - d1s).abs() >= 2 && (d1 - d1r).abs() <= 3 {
                    e.mask |= PIX_CNTOPP_ON;
                }
            }
        } else if d1dif_fine.abs() >= thres_ydif && thres_ydif > 0 {
            if exe_type == 0 {
                if (d1 - d1s).abs() <= 0 {
                    e.mask |= PIX_CNTF_OFF;
                    e.offedg = val_offedg;
                } else if (d1 - d1r).abs() <= 1 {
                    e.mask |= PIX_CNTF_ON;
                } else if oppside == 1 {
                    *flag_chkopp = 1;
                }
            } else if exe_type == 1 {
                if (d1 - d1s).abs() <= 0 {
                    // reference: no action
                } else if (d1 - d1r).abs() <= 1 {
                    e.mask |= PIX_CNTOPP_ON;
                }
            }
        } else if exe_type == 0 {
            e.mask |= PIX_CNTF_ST;
            if oppside == 1 {
                *flag_chkopp = 1;
            }
        }

        // for fade accumulation
        if exe_type == 0 {
            if dp_y0 < dp_y1 {
                if (d1_rev - d0_src) >= (3 << 4) && d1 > d1s - 4 && d1 < d1r + 4 {
                    e.sum_valid = true;
                    e.sum_off = i64::from(data0);
                    e.sum_on = i64::from(data1);
                    e.sum_dif = d1r - d1s;
                }
            } else if dp_y0 > dp_y1 && (d0_src - d1_rev) >= (3 << 4) && d1 > d1r - 4 && d1 < d1s + 4
            {
                e.sum_valid = true;
                e.sum_off = i64::from(data1);
                e.sum_on = i64::from(data0);
                e.sum_dif = d1s - d1r;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn getdif_pix(
    c: &mut Calc,
    data0: u8,
    data1: u8,
    dp_y0: i16,
    y0: i16,
    dp_y1: i16,
    y1: i16,
    thres_ymax: i16,
    thres_yedge: i16,
    thres_ydif: i16,
    thres_yoffedg: i16,
    most_logo_y: i16,
    exe_type: i16,
) {
    let tail = getdif_pix_core(
        c,
        data0,
        data1,
        i64::from(dp_y0),
        i64::from(y0),
        i64::from(dp_y1),
        i64::from(y1),
        i64::from(thres_ymax),
        i64::from(thres_yedge),
        i64::from(thres_ydif),
        i64::from(thres_yoffedg),
        i64::from(most_logo_y),
        i64::from(exe_type),
    )
    .tail;
    // REFERENCE QUIRK: the recursion swaps data0/data1 but keeps dp/y pairs,
    // and passes thres_dpysmin(100) where the signature position is
    // thres_yoffedg (logoframe_det.c:1620-1625). Ported verbatim.
    if tail.call {
        getdif_pix_core(
            c,
            data1,
            data0,
            i64::from(dp_y0),
            i64::from(y0),
            i64::from(dp_y1),
            i64::from(y1),
            i64::from(thres_ymax),
            i64::from(thres_yedge),
            i64::from(thres_ydif),
            THRES_DPYSMIN,
            i64::from(most_logo_y),
            1,
        );
    }
}

/// LogoCalc_getdif_areasum (1628-1674)
pub fn getdif_areasum(
    c: &mut Calc,
    data0: u8,
    dp_y0: i16,
    y0: i16,
    area_y0: i16,
    thres_ymax: i16,
    most_logo_y: i16,
) {
    let e = eval_areasum(
        data0,
        i64::from(dp_y0),
        i64::from(y0),
        i64::from(area_y0),
        i64::from(thres_ymax),
        i64::from(most_logo_y),
    );
    apply_area_eval(c, &e);
}

/// LogoCalc_getdif_vccalc (1677-1775)
pub fn vccalc(vc: &mut Vc, c: &Calc, kind: i64, init: &mut [i64; 8]) {
    match kind {
        0 => {
            *init = [0; 8];
            return;
        }
        1 => {
            vc.cntd_last_on = c.cnt_logoon;
            vc.cntd_last_off = c.cnt_logooff;
            vc.cntf_last_on = c.cntf_logoon;
            vc.cntf_last_off = c.cntf_logooff;
            return;
        }
        _ => {}
    }
    let [mut vcntd_logoon, mut vcntd_logooff, mut vcntf_logoon, mut vcntf_logooff, mut vcntd_von, mut vcntd_voff, mut vcntf_von, mut vcntf_voff] =
        *init;

    let cntd_cur_on = c.cnt_logoon;
    let cntd_cur_off = c.cnt_logooff;
    let cntf_cur_on = c.cntf_logoon;
    let cntf_cur_off = c.cntf_logooff;

    let mut cntd_dif_on = cntd_cur_on - vc.cntd_last_on;
    let mut cntd_dif_off = cntd_cur_off - vc.cntd_last_off;
    if cntd_dif_on < 0 {
        cntd_dif_on = 0;
    } else if cntd_dif_on > 15 && kind == 3 {
        cntd_dif_on = 15;
    }
    if cntd_dif_off < 0 {
        cntd_dif_off = 0;
    } else if cntd_dif_off > 15 && kind == 3 {
        cntd_dif_off = 15;
    }
    if cntd_dif_on > 2 {
        vcntd_von += 1;
        if (cntd_dif_on >= 8 && kind == 2) || (cntd_dif_on >= 15 && kind == 3) {
            vcntd_von += 1;
        }
    }
    if cntd_dif_off > 2 {
        vcntd_voff += 1;
        if (cntd_dif_off >= 8 && kind == 2) || (cntd_dif_off >= 15 && kind == 3) {
            vcntd_voff += 1;
        }
    }
    vcntd_logoon += cntd_dif_on;
    vcntd_logooff += cntd_dif_off;

    let mut cntf_dif_on = cntf_cur_on - vc.cntf_last_on;
    let mut cntf_dif_off = cntf_cur_off - vc.cntf_last_off;
    if cntf_dif_on < 0 {
        cntf_dif_on = 0;
    } else if cntf_dif_on > 15 && kind == 3 {
        cntf_dif_on = 15;
    }
    if cntf_dif_off < 0 {
        cntf_dif_off = 0;
    } else if cntf_dif_off > 15 && kind == 3 {
        cntf_dif_off = 15;
    }
    if cntf_dif_on > 2 {
        vcntf_von += 1;
        if (cntf_dif_on >= 8 && kind == 2) || (cntf_dif_on >= 15 && kind == 3) {
            vcntf_von += 1;
        }
    }
    if cntf_dif_off > 2 {
        vcntf_voff += 1;
        if (cntf_dif_off >= 8 && kind == 2) || (cntf_dif_off >= 15 && kind == 3) {
            vcntf_voff += 1;
        }
    }
    vcntf_logoon += cntf_dif_on;
    vcntf_logooff += cntf_dif_off;
    *init = [
        vcntd_logoon,
        vcntd_logooff,
        vcntf_logoon,
        vcntf_logooff,
        vcntd_von,
        vcntd_voff,
        vcntf_von,
        vcntf_voff,
    ];
}

/// LogoCalc_getdif_vcresult (1778-1883). vc fields passed as VcRes snapshot.
#[allow(clippy::too_many_arguments)]
pub fn vcresult(
    c: &mut Calc,
    vc_cntd_logoon: i64,
    vc_cntd_logooff: i64,
    vc_cntf_logoon: i64,
    vc_cntf_logooff: i64,
    vc_cntd_von: i64,
    vc_cntd_voff: i64,
    vc_cntf_von: i64,
    vc_cntf_voff: i64,
) -> (i64, i64, i64, i64) {
    let (mut vc_cntd_logoon, mut vc_cntd_logooff, mut vc_cntf_logoon, mut vc_cntf_logooff) = (
        vc_cntd_logoon,
        vc_cntd_logooff,
        vc_cntf_logoon,
        vc_cntf_logooff,
    );
    c.cnt_logovc = vc_cntd_logoon + vc_cntd_logooff;
    c.cntf_logovc = vc_cntf_logoon + vc_cntf_logooff;

    if vc_cntd_von <= 3 && vc_cntd_voff <= 3 {
        if c.cnt_logovc > 20 {
            if c.cnt_logoon > 10 {
                c.cnt_logoon = 10;
            }
            if c.cnt_logooff > 10 {
                c.cnt_logooff = 10;
            }
            c.cnt_logovc = c.cnt_logoon + c.cnt_logooff;
        }
    } else if vc_cntd_von <= 3 {
        if c.cnt_logoon > c.cnt_logooff && c.cnt_logooff > 10 {
            c.cnt_logoon = c.cnt_logooff - 1;
        }
    } else if vc_cntd_voff <= 3 {
        if c.cnt_logooff > c.cnt_logoon && c.cnt_logoon > 10 {
            c.cnt_logooff = c.cnt_logoon - 1;
        }
    } else {
        if vc_cntd_logoon > vc_cntd_logooff && c.cnt_logoon < c.cnt_logooff && c.cnt_logoon > 0 {
            c.cnt_logooff = c.cnt_logoon - 1;
        }
        if vc_cntd_logoon < vc_cntd_logooff && c.cnt_logoon > c.cnt_logooff && c.cnt_logooff > 0 {
            c.cnt_logoon = c.cnt_logooff - 1;
        }
    }
    if c.cnt_logovc > 20 {
        if vc_cntd_logoon > c.cnt_logoon {
            vc_cntd_logoon = c.cnt_logoon;
            c.cnt_logovc = vc_cntd_logoon + vc_cntd_logooff;
        }
        if vc_cntd_logooff > c.cnt_logooff {
            vc_cntd_logooff = c.cnt_logooff;
            c.cnt_logovc = vc_cntd_logoon + vc_cntd_logooff;
        }
    }

    if vc_cntf_von <= 3 && vc_cntf_voff <= 3 {
        if c.cntf_logovc > 30 {
            c.cntf_logovc = 30;
        }
    } else if vc_cntf_von <= 3 {
        if c.cntf_logoon > c.cntf_logooff && c.cntf_logooff > 10 {
            c.cntf_logoon = c.cntf_logooff - 1;
        }
    } else if vc_cntf_voff <= 3 {
        if c.cntf_logooff > c.cntf_logoon && c.cntf_logoon > 10 {
            c.cntf_logooff = c.cntf_logoon - 1;
        }
    } else {
        if vc_cntf_logoon > vc_cntf_logooff && c.cntf_logoon < c.cntf_logooff && c.cntf_logoon > 0 {
            c.cntf_logooff = c.cntf_logoon - 1;
        }
        if vc_cntf_logoon < vc_cntf_logooff && c.cntf_logoon > c.cntf_logooff && c.cntf_logooff > 0
        {
            c.cntf_logoon = c.cntf_logooff - 1;
        }
    }
    if c.cntf_logovc > 30 {
        if vc_cntf_logoon > c.cntf_logoon {
            vc_cntf_logoon = c.cntf_logoon;
            c.cntf_logovc = vc_cntf_logoon + vc_cntf_logooff;
        }
        if vc_cntf_logooff > c.cntf_logooff {
            vc_cntf_logooff = c.cntf_logooff;
            c.cntf_logovc = vc_cntf_logoon + vc_cntf_logooff;
        }
    }
    (
        vc_cntd_logoon,
        vc_cntd_logooff,
        vc_cntf_logoon,
        vc_cntf_logooff,
    )
}

/// §6.5 デバッグ用: pix 判定の中間値を文字列出力（WGSL との突合）。
#[allow(clippy::too_many_arguments)]
pub fn dbg_pix_inputs(
    data0: u8,
    data1: u8,
    dp_y0: i64,
    y0: i64,
    dp_y1: i64,
    y1: i64,
    thres_ymax: i64,
    thres_yedge: i64,
    thres_ydif: i64,
    thres_yoffedg: i64,
    most_logo_y: i64,
    fade_calcstep: i64,
) -> String {
    let r = getdif_pix_eval(
        data0,
        data1,
        dp_y0,
        y0,
        dp_y1,
        y1,
        thres_ymax,
        thres_yedge,
        thres_ydif,
        thres_yoffedg,
        most_logo_y,
        0,
        fade_calcstep,
    );
    format!("mask={} hist={} chkopp={}", r.e.mask, r.e.hist, r.tail.call)
}

/// LogoCalc_getdif (1885-2025): one frame against the logo region.
/// `data` is the full 8-bit luma plane; `pitch` the row stride (design 6.0).
#[allow(clippy::too_many_arguments)]
pub fn getdif(
    c1: &mut Calc,
    c2: &mut Calc,
    p: &Param,
    data: &[u8],
    pitch: usize,
    thres_ymax: i16,
    thres_yedge: i16,
    thres_ydif: i16,
    thres_yoffedg: i16,
) {
    const INTERVAL: i64 = 2;
    let lw = i64::from(p.width);
    let lh = i64::from(p.height);
    let mut vc1 = Vc::default();
    let mut vc2 = Vc::default();
    let mut init1 = [0i64; 8];
    let mut init2 = [0i64; 8];
    vccalc(&mut vc1, c1, 0, &mut init1);
    vccalc(&mut vc2, c2, 0, &mut init2);

    let base = (i64::from(p.pos_y) * pitch as i64 + i64::from(p.pos_x)) as usize;

    let mut interlace = i64::from(p.pos_y) % 2;
    for i in 0..lh {
        let numline = i * lw;
        let row = base + (i * pitch as i64) as usize;
        if interlace == 0 {
            col_pass(
                c1,
                &mut vc1,
                &mut init1,
                p,
                data,
                row,
                numline,
                lw,
                INTERVAL,
                thres_ymax,
                thres_yedge,
                thres_ydif,
                thres_yoffedg,
                2,
            );
        } else {
            col_pass(
                c2,
                &mut vc2,
                &mut init2,
                p,
                data,
                row,
                numline,
                lw,
                INTERVAL,
                thres_ymax,
                thres_yedge,
                thres_ydif,
                thres_yoffedg,
                2,
            );
        }
        interlace = 1 - interlace;
    }

    let mut interlace = i64::from(p.pos_y) % 2;
    for i in 0..lh {
        let numline = i * lw;
        let row = base + (i * pitch as i64) as usize;
        if interlace == 0 {
            row_pass(
                c1,
                &mut vc1,
                &mut init1,
                p,
                data,
                pitch,
                row,
                numline,
                lw,
                INTERVAL,
                thres_ymax,
                thres_yedge,
                thres_ydif,
                thres_yoffedg,
                i,
                lh,
            );
        } else {
            row_pass(
                c2,
                &mut vc2,
                &mut init2,
                p,
                data,
                pitch,
                row,
                numline,
                lw,
                INTERVAL,
                thres_ymax,
                thres_yedge,
                thres_ydif,
                thres_yoffedg,
                i,
                lh,
            );
        }
        interlace = 1 - interlace;
    }

    let _ = vcresult(
        c1, init1[0], init1[1], init1[2], init1[3], init1[4], init1[5], init1[6], init1[7],
    );
    let _ = vcresult(
        c2, init2[0], init2[1], init2[2], init2[3], init2[4], init2[5], init2[6], init2[7],
    );

    c1.total_dif = p.total_dif_c1;
    c2.total_dif = p.total_dif_c2;
}

#[allow(clippy::too_many_arguments)]
fn col_pass(
    c: &mut Calc,
    vc: &mut Vc,
    init: &mut [i64; 8],
    p: &Param,
    data: &[u8],
    row: usize,
    numline: i64,
    lw: i64,
    interval: i64,
    thres_ymax: i16,
    thres_yedge: i16,
    thres_ydif: i16,
    thres_yoffedg: i16,
    kind: i64,
) {
    vccalc(vc, c, 1, init);
    for j in 0..lw {
        let idx0 = (numline + j) as usize;
        let idx1 = (numline + j + interval) as usize;
        if p.dif_y_col[idx0] > 0 {
            let d0 = data[row + j as usize];
            let d1 = data[row + j as usize + interval as usize];
            getdif_pix(
                c,
                d0,
                d1,
                p.dp_y[idx0],
                p.lum16[idx0],
                p.dp_y[idx1],
                p.lum16[idx1],
                thres_ymax,
                thres_yedge,
                thres_ydif,
                thres_yoffedg,
                p.most_logo_y,
                0,
            );
        }
        if p.area_y[idx0] > 0 {
            let d0 = data[row + j as usize];
            getdif_areasum(
                c,
                d0,
                p.dp_y[idx0],
                p.lum16[idx0],
                p.area_y[idx0],
                thres_ymax,
                p.most_logo_y,
            );
        }
    }
    vccalc(vc, c, kind, init);
}

#[allow(clippy::too_many_arguments)]
fn row_pass(
    c: &mut Calc,
    vc: &mut Vc,
    init: &mut [i64; 8],
    p: &Param,
    data: &[u8],
    pitch: usize,
    row: usize,
    numline: i64,
    lw: i64,
    interval: i64,
    thres_ymax: i16,
    thres_yedge: i16,
    thres_ydif: i16,
    thres_yoffedg: i16,
    i: i64,
    lh: i64,
) {
    vccalc(vc, c, 1, init);
    // reference computes the lower-row pointers only within the guard; the
    // dif_y_row mask is zero on the last `interval` rows, so reads below are
    // bounded exactly like the C (pitch*interval may point past the region end
    // but stays inside the padded frame for the guarded rows)
    for j in 0..lw {
        let idx0 = (numline + j) as usize;
        if p.dif_y_row[idx0] > 0 && i + interval < lh {
            let idx1 = (numline + j + lw * interval) as usize;
            let d0 = data[row + j as usize];
            // C: pt_data1 = pt_data_line + pitch*interval (row is already offset by i)
            let d1 = data[row + (interval as usize) * pitch + j as usize];
            getdif_pix(
                c,
                d0,
                d1,
                p.dp_y[idx0],
                p.lum16[idx0],
                p.dp_y[idx1],
                p.lum16[idx1],
                thres_ymax,
                thres_yedge,
                thres_ydif,
                thres_yoffedg,
                p.most_logo_y,
                0,
            );
        }
    }
    vccalc(vc, c, 3, init);
}

/// LogoCalc_summary (2027-2312)
pub fn summary(c: &mut Calc, p: &Param) {
    let mut val_hist_max = 0i64;
    // reference computes val_hist_sum as a dead accumulator (logoframe_det.c
    // :2050-2057); omitted here with no behavioral effect.
    let mut n_hist_max = 0usize;
    let mut i = LOGO_FADE_OFST;
    while i <= LOGO_FADE_OFST + c.fade_calcstep {
        let val_hist =
            c.hist_y[(i - 1) as usize] + c.hist_y[i as usize] + c.hist_y[(i + 1) as usize];
        if val_hist > val_hist_max {
            val_hist_max = val_hist;
            n_hist_max = i as usize;
        }
        i += 1;
    }
    let rate_fade_hist: i64 = if val_hist_max > 0 {
        ((c.hist_y[n_hist_max - 1] * ((n_hist_max - 1) as i64 - LOGO_FADE_OFST)
            + c.hist_y[n_hist_max] * (n_hist_max as i64 - LOGO_FADE_OFST)
            + c.hist_y[n_hist_max + 1] * (n_hist_max as i64 + 1 - LOGO_FADE_OFST))
            * 1000
            + val_hist_max * c.fade_calcstep / 2)
            / (val_hist_max * c.fade_calcstep)
    } else {
        0
    };
    let rate_fade_hist = rate_fade_hist.clamp(0, 1000);

    let sumt_areanum = c.sum_areanum;
    let sumt_areafact = c.sum_areaon - c.sum_areaoff;
    let sumt_areadif = c.sum_areadif;
    let rate_area_avg: i64 = if sumt_areadif > 0 {
        (sumt_areafact * 1000 + sumt_areadif / 2) / sumt_areadif
    } else {
        0
    };
    let rate_area_avg = rate_area_avg.clamp(0, 1000);

    let mut invalid_logooff = 0i64;
    let cnttmp1 = c.cnt_logooff * 2 + c.cntf_logooff;
    let cnttmp2 = c.cnt_offedg * 2 + c.cntf_offedg;
    if cnttmp1 - cnttmp2 <= c.cnt_logoon * 2 + c.cntf_logoon {
        if cnttmp1 > c.cnt_logoon * 2 + c.cntf_logoon {
            invalid_logooff = 1;
        }
    } else {
        c.cnt_logooff -= c.cnt_offedg;
        c.cntf_logooff -= c.cntf_offedg;
    }
    let cnttmpsum = c.cnts_logoon + c.cnts_logooff;
    if cnttmpsum > 0 {
        let (mut t1, mut t2) = (c.cnts_logoon, c.cnts_logooff);
        if cnttmpsum > 30 {
            t1 = c.cnts_logoon * 30 / cnttmpsum;
            t2 = c.cnts_logooff * 30 / cnttmpsum;
        }
        c.cntf_logoon += t1;
        c.cntf_logooff += t2;
        c.cntf_logovc += t1 + t2;
    }

    let cnt_logo_both0 = c.cnt_logoon + c.cnt_logooff;
    let mut cnt_logo_both = cnt_logo_both0;
    let rate_logoon_cnt: i64 = if cnt_logo_both > 0 {
        (c.cnt_logoon * 1000 + cnt_logo_both / 2) / cnt_logo_both
    } else {
        0
    };
    let mut cnt_logo_lim = c.cnt_logovc;

    let mut cntf_logoon = c.cntf_logoon;
    let mut cntf_logooff = c.cntf_logooff;
    let cntopp = c.cntopp_logoon;
    let mut cntf_valid = c.cntf_logovc;
    let cntf_offlimit = cntf_logooff * 49 / 51 - 1;
    if cntf_logooff < cntf_logoon && cntf_logooff < cntopp * 3 {
        cntf_valid = cntf_logoon + cntf_logooff / 4 - cntopp;
        if cntf_valid > c.cntf_logovc {
            cntf_valid = c.cntf_logovc;
        }
        cntf_logooff += cntopp * 2;
        if cntf_logooff > cntf_logoon {
            cntf_logooff = cntf_logoon;
        }
    } else if cntf_offlimit > cntf_logoon && cntf_logoon > cntopp * 3 {
        cntf_logoon += cntf_logoon - cntopp;
        if cntf_logoon > cntf_offlimit {
            cntf_logoon = cntf_offlimit;
        }
    }

    let cntf_logo_both = cntf_logoon + cntf_logooff;
    let rate_logoon_cntf: i64 = if cntf_logo_both > 0 {
        (cntf_logoon * 1000 + cntf_logo_both / 2) / cntf_logo_both
    } else {
        0
    };

    let num_d1 = cnt_logo_both0 * 2;
    let num_d2 = cntf_logo_both;
    let mut num_sum = num_d1 + num_d2;
    if num_sum == 0 {
        num_sum = 1;
    }
    let weight_logocnt = (num_d1 * 1000 + num_sum / 2) / num_sum;
    let rate_logoon =
        (rate_logoon_cnt * weight_logocnt + rate_logoon_cntf * (1000 - weight_logocnt) + 10000 / 2)
            / 10000;

    let num_d1 = val_hist_max * 2;
    let num_d2 = sumt_areanum;
    let mut num_sum = num_d1 + num_d2;
    if num_sum == 0 {
        num_sum = 1;
    }
    let weight_fadecnt = (num_d1 * 1000 + num_sum / 2) / num_sum;
    let rate_fade =
        (rate_fade_hist * weight_fadecnt + rate_area_avg * (1000 - weight_fadecnt) + 10000 / 2)
            / 10000;

    c.rate_areaavg_t = ((rate_area_avg + 5) / 10) as i16;
    c.rate_logoon = rate_logoon as i16;
    c.rate_fade = rate_fade as i16;

    if cnt_logo_both0 < 4 && cntf_valid < 8 {
        c.flag_nosample = 1;
    } else {
        c.flag_nosample = 0;
    }
    if cnt_logo_both0 + c.cnt_logomv < 4 {
        c.flag_nofadesm = 1;
    } else {
        c.flag_nofadesm = 0;
    }

    if c.total_dif < 200 && c.total_dif >= 10 {
        if cnt_logo_both * 2 > c.total_dif {
            if cnt_logo_both <= 100 {
                cnt_logo_both = 101;
            }
            if cnt_logo_lim * 2 > c.total_dif && cnt_logo_lim <= 60 {
                if cnt_logo_lim * 4 > c.total_dif * 3 {
                    cnt_logo_lim = 61;
                } else {
                    cnt_logo_lim = 60;
                }
            }
        } else if cntf_valid * 2 > c.total_dif {
            if cntf_valid <= 100 {
                if cntf_valid * 4 > c.total_dif * 3 {
                    cntf_valid = 101;
                } else {
                    cntf_valid = 100;
                }
            }
        } else if (cnt_logo_lim + cntf_valid / 2) * 2 > c.total_dif {
            if cntf_valid < (100 - cnt_logo_lim) * 2 {
                cntf_valid = (100 - cnt_logo_lim) * 2;
            }
        } else if cnt_logo_lim * 6 > c.total_dif && cnt_logo_lim > 4 {
            if cnt_logo_both <= 20 {
                cnt_logo_both = 21;
            }
            if cnt_logo_lim <= 20 {
                cnt_logo_lim = 21;
            }
        } else if cntf_valid > 4
            && (cntf_valid * 5 > c.total_dif || (cntf_valid * 7 > c.total_dif && cntf_valid > 25))
        {
            if i64::from(p.thres_dp_y) < LOGO_MAX_DP * 13 / 100 {
                if cntf_valid <= 35 {
                    cntf_valid = 35;
                }
            } else if cntf_valid <= 34 {
                cntf_valid = 34;
            }
        }
    }

    if c.flag_nosample != 0 || invalid_logooff > 0 && rate_logoon < 50 {
        c.rank_unclear = 9;
    } else if (cnt_logo_both > 100 && cnt_logo_lim > 60 && !(5..=90).contains(&rate_logoon))
        || (cntf_valid > 100 && !(4..=90).contains(&rate_logoon))
    {
        c.rank_unclear = 0;
    } else if (cnt_logo_both >= 100 && cnt_logo_lim >= 60 && !(15..=80).contains(&rate_logoon))
        || (cntf_valid >= 100 && !(10..=80).contains(&rate_logoon))
        || (cnt_logo_lim + cntf_valid / 2 > 100 && !(10..=80).contains(&rate_logoon))
    {
        c.rank_unclear = 1;
    } else if (cnt_logo_both > 20 && cnt_logo_lim > 20 && !(35..=65).contains(&rate_logoon))
        || (cntf_valid > 35 && !(35..=65).contains(&rate_logoon))
        || (cntf_valid == 35 && !(15..=75).contains(&rate_logoon))
    {
        c.rank_unclear = 2;
    } else {
        c.rank_unclear = 3;
    }
}

/// LogoCalc_areasummary_getloc (2316-2418)
pub fn areasummary_getloc(hist: &[i64; 256], st: i64, ed: i64) -> (i64, i64, i64) {
    let mut num_max = 0i64;
    let mut loc_max = 1i64;
    for i in st..=ed {
        let num = hist[(i - 1) as usize] + hist[i as usize] + hist[(i + 1) as usize];
        if num_max < num {
            num_max = num;
            loc_max = i;
        }
    }
    let mut hist_clip = [0i64; 11];
    let mut num_avg5 = 0i64;
    let mut val_calc = 0i64;
    for i in 0..=10i64 {
        let loc = loc_max + i - 5;
        hist_clip[i as usize] = if (0..=255).contains(&loc) {
            hist[loc as usize]
        } else {
            0
        };
        if (3..=7).contains(&i) {
            num_avg5 += hist_clip[i as usize];
            val_calc += hist_clip[i as usize] * loc;
        }
    }
    let mloc_avg5 = if num_avg5 > 0 {
        (val_calc * 16 + num_avg5 / 2) / num_avg5
    } else {
        let loc_max2 = (st + ed) / 2;
        loc_max2 * 16
    };
    let loc_avg5_div = mloc_avg5 / 16;
    let loc_avg5_mod = mloc_avg5 % 16;
    let ind_avg5 = loc_avg5_div + 5 - loc_max;
    let ind_avg5 = ind_avg5.clamp(3, 7);
    let mut val_num = 0i64;
    let mut val_data = 0i64;
    for i in 0..7i64 {
        let val = (hist_clip[(ind_avg5 + i - 3) as usize] * (16 - loc_avg5_mod)
            + hist_clip[(ind_avg5 + i - 2) as usize] * loc_avg5_mod
            + 8)
            / 16;
        let mul = match i {
            0 | 6 => 256,
            1 | 5 => 64,
            2 | 4 => 16,
            _ => 0,
        };
        val_num += val;
        val_data += val * mul;
    }
    let val_data = if val_num > 0 {
        (val_data + val_num / 2) / val_num
    } else {
        99
    };
    (mloc_avg5, num_avg5, val_data)
}

/// LogoCalc_areasummary_one (2420-2559)
/// 原典 logoframe_det.c LogoGetRateCalc の決定表を忠実移植。
/// 非隣接腕に同一値 (-1/35/65) が現れるため腕順序と分離を維持する。
#[allow(clippy::if_same_then_else)]
pub fn areasummary_one(a: &mut AreaCalc, thres_yedge: i64, area_type: i64) {
    let (loc_histon_max, val_histon_max, vari_histon_max) =
        areasummary_getloc(&a.hista_areaon, 1, 254);
    let sloc_histon_max = (loc_histon_max + 8) / 16;
    let (loc_histcal_max, _val_histcal_max, vari_histcal_max) =
        areasummary_getloc(&a.hista_areacal, 1, 254);
    let sloc_histcal_max = (loc_histcal_max + 8) / 16;
    let mut loc_st = sloc_histcal_max - 4;
    let mut loc_ed = sloc_histcal_max + 4;
    if loc_st > sloc_histon_max - 2 {
        loc_st = sloc_histon_max - 2;
    }
    if loc_ed < sloc_histon_max + 2 {
        loc_ed = sloc_histon_max + 2;
    }
    if loc_st < 1 {
        loc_st = 1;
    }
    if loc_ed > 254 {
        loc_ed = 254;
    }
    let (loc_histoff_max, val_histoff_max, vari_histoff_max) =
        areasummary_getloc(&a.hista_areaoff, loc_st, loc_ed);

    if a.num_hista_on > 0 {
        a.rate_areavalid = ((val_histon_max * 100 + a.num_hista_on / 2) / a.num_hista_on) as i16;
    } else {
        a.rate_areavalid = 0;
    }
    let rate_histoff = if a.num_hista_off > 0 {
        (val_histoff_max * 100 + a.num_hista_off / 2) / a.num_hista_off
    } else {
        0
    };

    a.diff_arealogo = (loc_histon_max - loc_histcal_max) as i16;
    a.diff1_arealogo = (loc_histon_max - loc_histoff_max) as i16;
    a.diff2_arealogo = (loc_histoff_max - loc_histcal_max) as i16;
    a.vari_arealogo_on = vari_histon_max as i16;
    a.vari_arealogo_cal = vari_histcal_max as i16;
    a.vari_arealogo_off = vari_histoff_max as i16;

    let diff1 = i64::from(a.diff1_arealogo).abs();
    let diff2 = i64::from(a.diff2_arealogo).abs();
    let rate_valid = i64::from(a.rate_areavalid);
    let vari_off = i64::from(a.vari_arealogo_off);

    let rate = if rate_valid <= 70
        || rate_histoff < 60
        || (area_type == 1 && vari_off >= 56)
        || (diff1 >= 56 && diff2 >= 56)
    {
        -1
    } else if diff1 - diff2 >= 48 {
        65
    } else if diff2 - diff1 >= 48 {
        35
    } else if rate_valid <= 80 || (area_type == 1 && vari_off > 40) {
        -1
    } else if diff1 - diff2 >= 32 {
        65
    } else if diff2 - diff1 >= 32 {
        35
    } else if (diff1 >= 10 && diff2 >= 30 && vari_off <= 30)
        || (diff1 >= 7 && vari_off <= diff1 * 2 && diff1 + 16 < diff2)
    {
        -1
    } else if diff1 >= 7 && vari_off <= diff1 * 2 {
        65
    } else if rate_valid <= 90 {
        -1
    } else if diff1 - diff2 >= 24 {
        65
    } else if diff2 - diff1 >= 24 && vari_off <= 48 {
        35
    } else if diff1 <= 6 && diff2 >= 20 && vari_off <= 24 {
        35
    } else if diff1 <= 5 && diff2 >= 16 && vari_off <= 16 {
        35
    } else if thres_yedge < sloc_histon_max || area_type > 0 {
        -1
    } else if diff1 <= 2 && diff2 - diff1 >= 12 && vari_off <= 9 {
        35
    } else if diff1 == 0 && diff2 >= 10 && vari_off == 0 {
        35
    } else {
        -1
    };
    a.rate_arealogo = rate as i16;
}

/// LogoCalc_areasummary (2562-2662)
pub fn areasummary(c: &mut Calc, thres_yedge: i16, num_areaset: i16) {
    if i64::from(c.rank_unclear) >= 3 {
        let mut flag_on = 0i64;
        let mut flag_off = 0i64;
        let mut rate_local = 0i64;
        for i in 0..LOGO_AREANUM {
            let a = &mut c.area[i];
            if a.num_hista_off >= 3 && a.num_hista_on >= 3 {
                let area_type = if i == 0 { 0 } else { 1 };
                areasummary_one(a, i64::from(thres_yedge), area_type);
                if a.rate_arealogo >= 0 && i > 0 {
                    rate_local += i64::from(a.rate_arealogo);
                    if a.rate_arealogo >= 50 {
                        flag_on += 1;
                    } else {
                        flag_off += 1;
                    }
                }
            } else {
                a.rate_arealogo = -1;
            }
        }
        if num_areaset == 0 {
            flag_on = 0;
            flag_off = 0;
        }
        let flag_both = flag_on + flag_off;
        if flag_both > 0 {
            rate_local = (rate_local + flag_both / 2) / flag_both;
        }
        let rate_main = i64::from(c.area[0].rate_arealogo);
        let rate_area: i64 = if flag_both == 0 {
            rate_main
        } else if rate_main < 0 {
            if flag_both < 3 {
                (rate_local + 50 + 1) / 2
            } else {
                rate_local
            }
        } else {
            let weight_local = flag_both;
            (rate_main * (10 - weight_local) + rate_local * weight_local + 5) / 10
        };

        if rate_area >= 0 {
            if c.rank_unclear == 9 {
                c.rank_unclear = 3;
                c.rate_logoon = rate_area as i16;
            } else if c.rank_unclear == 3 {
                let cnt_logo_both =
                    c.cnt_logoon + c.cnt_logooff + (c.cntf_logoon + c.cntf_logooff) / 2;
                if cnt_logo_both < 10 {
                    c.rate_logoon = ((i64::from(c.rate_logoon) + rate_area * 2) / 3) as i16;
                } else {
                    c.rate_logoon = ((i64::from(c.rate_logoon) + rate_area) / 2) as i16;
                }
            }
        }
        c.rate_arealogo = rate_area as i16;
    } else {
        c.rate_arealogo = -2;
    }
}

/// LogoCalc (2705-2730): one frame of detection against the prepared logo.
#[allow(clippy::too_many_arguments)]
pub fn logo_calc_frame(
    p: &Param,
    t: &crate::logo_set::Thres,
    store: &mut crate::logo_find::FrameStore,
    data: &[u8],
    pitch: usize,
    nframe: usize,
) {
    let mut c1 = clear(t.num_fadein, t.num_fadeout);
    let mut c2 = clear(t.num_fadein, t.num_fadeout);
    getdif(
        &mut c1,
        &mut c2,
        p,
        data,
        pitch,
        t.thres_ymax,
        t.thres_yedge,
        t.thres_ydif,
        t.thres_yoffedg,
    );
    summary(&mut c1, p);
    summary(&mut c2, p);
    areasummary(&mut c1, t.thres_yedge, t.num_areaset);
    areasummary(&mut c2, t.thres_yedge, t.num_areaset);
    savedata(store, &c1, &c2, nframe);
}

/// LogoCalc_savedata frame arrays (owned by LogoStore in logo_find.rs;
/// this mirrors its per-frame write exactly, 2665-2704).
#[allow(clippy::too_many_arguments)]
pub fn savedata(store: &mut crate::logo_find::FrameStore, c1: &Calc, c2: &Calc, nframe: usize) {
    store.rate_logooni1[nframe] = c1.rate_logoon as i8;
    store.rate_logooni2[nframe] = c2.rate_logoon as i8;
    store.rate_fadei1[nframe] = c1.rate_fade as i8;
    store.rate_fadei2[nframe] = c2.rate_fade as i8;
    store.rate_logoon[nframe] = ((i64::from(c1.rate_logoon) + i64::from(c2.rate_logoon)) / 2) as i8;
    store.rate_fade[nframe] = ((i64::from(c1.rate_fade) + i64::from(c2.rate_fade)) / 2) as i8;
    store.flag_nosample[nframe] =
        (((i64::from(c2.flag_nosample)) << 1) + i64::from(c1.flag_nosample)) as i8;
    store.flag_nofadesm[nframe] =
        (((i64::from(c2.flag_nofadesm)) << 1) + i64::from(c1.flag_nofadesm)) as i8;
    let tmp = if c1.rank_unclear >= c2.rank_unclear {
        c1.rank_unclear
    } else {
        c2.rank_unclear
    };
    store.rank_unclear[nframe] = tmp as i8;
    if c1.rank_unclear == 9 && c2.rank_unclear != 9 {
        store.rank_unclear[nframe] = c2.rank_unclear as i8;
        store.rate_logoon[nframe] = c2.rate_logoon as i8;
        store.rate_fade[nframe] = c2.rate_fade as i8;
    } else if c1.rank_unclear != 9 && c2.rank_unclear == 9 {
        store.rank_unclear[nframe] = c1.rank_unclear as i8;
        store.rate_logoon[nframe] = c1.rate_logoon as i8;
        store.rate_fade[nframe] = c1.rate_fade as i8;
    }
}
