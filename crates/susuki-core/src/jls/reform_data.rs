//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! JlsReformData: input-data pre-adjustment (adjustData pipeline).
//! Mirrors JlsReformData.cpp 1-1246.

use super::cnvstr::Cnv;
use super::dataset::*;
use super::dataset_search::*;
use super::reform_base::CalcDifInfo;

pub const SCP_PRIOR_DUPE: i32 = -1;
pub const SCP_PRIOR_NONE: i32 = 0;
pub const SCP_PRIOR_LV1: i32 = 1;
pub const SCP_PRIOR_DECIDE: i32 = 2;
pub const LOGO_PRIOR_DUPE: i32 = -1;
pub const LOGO_PRIOR_LV1: i32 = 1;

/// priorLogoFromScp
fn prior_logo_from_scp(n: i32) -> i32 {
    n
}

/// getFrm30fpsFromMsec
pub fn get_frm30fps_from_msec(msec: i32) -> i32 {
    let frate_n: i64 = 30000;
    let frate_d: i64 = 1001;
    let amsec = (msec as i64).abs();
    let r = ((amsec * frate_n) + (frate_d * 1000 / 2)) / (frate_d * 1000);
    if msec >= 0 {
        r as i32
    } else {
        -(r as i32)
    }
}

/// getMsec30fpsFromFrm
pub fn get_msec30fps_from_frm(frm: i32) -> i32 {
    let frate_n: i64 = 30000;
    let frate_d: i64 = 1001;
    let afrm = (frm as i64).abs();
    let r = (afrm * frate_d * 1000 + (frate_n / 2)) / frate_n;
    if frm >= 0 {
        r as i32
    } else {
        -(r as i32)
    }
}

/// calcDifFunc
pub fn calc_dif_func(ds: &Dataset, calc: &mut CalcDifInfo, msec_src: i32, msec_dst: i32) -> i32 {
    let mut flag = 0;
    let msec_dif_org = msec_dst - msec_src;
    let msec_dif_abs = msec_dif_org.abs();
    let sec_dif = (msec_dif_abs + 500) / 1000;
    let msec_gap = (sec_dif * 1000 - msec_dif_abs).abs();
    if ((msec_gap <= ds.msec_val_near1 && sec_dif >= 3)
        || (msec_gap <= ds.msec_val_near2 && sec_dif == 10)
        || (msec_gap <= ds.msec_val_near2 && sec_dif >= 15 && sec_dif % 15 == 0))
        && sec_dif <= 120
    {
        flag = 1;
        if sec_dif <= 25
            || sec_dif == 30
            || sec_dif == 45
            || sec_dif == 60
            || sec_dif == 90
            || sec_dif == 120
        {
            flag = 2;
        }
    }
    if msec_dif_org < 0 && sec_dif != 0 {
        calc.sgn = -1;
    } else if msec_dif_org > 0 && sec_dif != 0 {
        calc.sgn = 1;
    } else {
        calc.sgn = 0;
    }
    calc.sec = sec_dif;
    calc.gap = msec_gap;
    flag
}

/// calcDifExact
pub fn calc_dif_exact(ds: &Dataset, calc: &mut CalcDifInfo, msec_src: i32, msec_dst: i32) -> i32 {
    let mut flag = 0;
    let msec_dif_org = msec_dst - msec_src;
    let msec_dif_abs = msec_dif_org.abs();
    let sec_dif = (msec_dif_abs + 500) / 1000;
    let msec_gap = (sec_dif * 1000 - msec_dif_abs).abs();
    if msec_gap <= ds.msec_val_near1
        && (sec_dif == 10
            || sec_dif == 15
            || sec_dif == 30
            || sec_dif == 45
            || sec_dif == 60
            || sec_dif == 90
            || sec_dif == 120)
    {
        flag = 2;
    } else if msec_gap <= ds.msec_val_exact && (3..=25).contains(&sec_dif) {
        flag = 1;
    }
    if msec_dif_org < 0 && sec_dif != 0 {
        calc.sgn = -1;
    } else if msec_dif_org > 0 && sec_dif != 0 {
        calc.sgn = 1;
    } else {
        calc.sgn = 0;
    }
    calc.sec = sec_dif;
    calc.gap = msec_gap;
    flag
}

/// detectCutmrgDispval
pub fn detect_cutmrg_dispval(mrg: i32) -> String {
    // --- 16倍した値のベース値を取得 ---
    let mrg_abs = mrg.abs();
    let valbase = get_frm30fps_from_msec(mrg_abs * 16 + 8);
    // --- 結果 ---
    let valint = valbase / 16;
    let valmod = valbase % 16;
    // --- 表示文字列作成 ---
    let mut strdisp = String::new();
    if mrg < 0 && (valint != 0 || valmod != 0) {
        strdisp.push('-');
    }
    strdisp.push_str(&valint.to_string());
    if valmod > 0 {
        let valfrac = (valmod * 100 + 8) / 16;
        strdisp.push('.');
        let valf1 = valfrac / 10;
        let valf2 = valfrac % 10;
        strdisp.push_str(&valf1.to_string());
        if valf2 > 0 {
            strdisp.push_str(&valf2.to_string());
        }
    }
    strdisp
}

/// detectCutmrgSub
pub fn detect_cutmrg_sub(ds: &Dataset, cnv: &Cnv, rise: i32) -> i32 {
    let num_logo = ds.size_logo();
    let num_scpos = ds.size_scp();
    const MINUS: i32 = 10;

    let mut hist = [0i32; 200];
    let mut hist_h = [0i32; 200];
    let mut fade_cnt = 0i32;
    let mut loc_max = 0i32;

    let mut loopn = 2;
    while loopn > 0 {
        // 使用する確定度
        let stat_level = if loopn == 2 {
            SCP_PRIOR_DECIDE
        } else {
            SCP_PRIOR_NONE
        };

        fade_cnt = 0;
        let mut sum_scale = 0;
        for i in 0..200 {
            hist[i] = 0;
            hist_h[i] = 0;
        }
        for i in 0..num_logo {
            let dtlogo = match ds.get_logo(i) {
                Some(l) => l,
                None => continue,
            };
            let mut scale: i32;
            let intl: i32;
            let msec_logo: i32;
            {
                // msec_logo, scale, intl, fade_cnt
                let msec_dif: i32;
                let fade: i32;
                if rise > 0 {
                    msec_logo = dtlogo.rise;
                    msec_dif = dtlogo.rise_r - dtlogo.rise_l;
                    fade = dtlogo.fade_rise;
                    intl = dtlogo.intl_rise;
                } else {
                    msec_logo = dtlogo.fall;
                    msec_dif = dtlogo.fall_r - dtlogo.fall_l;
                    fade = -dtlogo.fade_fall;
                    intl = dtlogo.intl_fall;
                }
                let mut msec_fade = 0;
                if fade != 0 {
                    let msec_tmp = cnv
                        .tc
                        .get_msec_adjust_frm_from_msec(msec_logo as i64, fade as i64)
                        as i32;
                    msec_fade = (msec_tmp - msec_logo).abs();
                }
                // ロゴの不確定度で重みづけ
                scale = 1;
                if msec_logo <= 50 || msec_logo >= ds.msec_total_max - 50 {
                    scale = 0;
                } else if msec_dif <= 50 || msec_dif <= msec_fade + 50 {
                    scale = 3;
                }
                sum_scale += scale;
                // フェード設定ありの判別
                if fade != 0 {
                    fade_cnt += scale;
                } else {
                    fade_cnt -= scale;
                }
            }
            // 確定無音シーンチェンジをマージン位置候補として追加していく
            if scale > 0 {
                let mut j = 1;
                while j < num_scpos - 1 {
                    let msec_scpj = ds.msec_scp(j);
                    let msec_dif = if rise > 0 {
                        msec_logo - msec_scpj
                    } else {
                        msec_scpj - msec_logo
                    };
                    let mut tmp_dif = get_frm30fps_from_msec(msec_dif); // 30fps換算で推測
                    tmp_dif += MINUS;
                    if ds.scp_statpos(j) >= stat_level && (0..200).contains(&tmp_dif) {
                        hist[tmp_dif as usize] += scale;
                        if intl > 0 {
                            hist_h[tmp_dif as usize] += scale;
                        }
                    }
                    j += 1;
                }
            }
        }

        // 候補から位置を決める
        loc_max = 0;
        let mut val_max = 0;
        for i in 2..200 - 2 {
            let val: i32 = if fade_cnt > 0 {
                hist[i - 2] + hist[i - 1] + hist[i] + hist[i + 1] + hist[i + 2]
            } else {
                hist[i - 1] + hist[i] + hist[i + 1]
            };
            let mut val = val;
            if (i as i32) >= MINUS - 2 && (i as i32) <= 36 + MINUS {
                // 標準的なマージン範囲
                val *= 2; // 大き目に設定
            } else if val == sum_scale {
                // 全位置で検出した場合
                val = val * 5 / 3; // 少し大き目に設定
            } else if (i as i32) >= 60 + MINUS {
                // 2秒以上はほぼ対象外
                val /= 4;
            }
            if val_max < val
                || (val_max == val
                    && loc_max > 0
                    && hist_h[(loc_max - 1) as usize] < (hist[i + 1] - hist_h[i + 1]))
            {
                val_max = val;
                loc_max = i as i32;
            }
        }
        // 候補が存在するか確認
        if num_logo == 1 && val_max > 0 {
            // ロゴ１つだけの場合
            loopn = 0;
        }
        if val_max <= 1 || val_max <= num_logo * 2 / 3 {
            // 候補不足
            loopn -= 1;
            loc_max = 0;
        } else {
            loopn = 0;
        }
    }

    // 候補の中から確定させる
    let mut loc_det = MINUS;
    let mut loc_half = 0;
    if loc_max >= 2 {
        if fade_cnt <= 0 {
            // フェードなし
            loc_det = loc_max - 1;
            let mut val_max = 0;
            let mut tmp_hist1 = [0i32; 3];
            let mut tmp_hist2 = [0i32; 3];
            for i in (loc_max - 1)..=(loc_max + 1) {
                let val = hist[i as usize];
                tmp_hist1[(i - loc_max + 1) as usize] = hist[i as usize] - hist_h[i as usize];
                tmp_hist2[(i - loc_max + 1) as usize] = hist_h[i as usize];
                if val_max < val {
                    val_max = val;
                    loc_det = i;
                }
            }
            if loc_det == loc_max - 1 {
                if tmp_hist2[0] < 3 {
                    // HALFデータなし
                    loc_half = 0;
                    if loc_det <= MINUS {
                        loc_half = 1;
                    }
                } else if (tmp_hist1[1] + tmp_hist1[2]) * 5 <= tmp_hist1[0] {
                    // 全部最小
                    loc_half = 1;
                } else {
                    // ALL:中間 HALF:最小
                    loc_det += 1;
                    loc_half = 0;
                }
            } else if loc_det == loc_max + 1 {
                if tmp_hist2[0] < 3 && tmp_hist2[1] < 3 && tmp_hist2[2] < 3 {
                    // HALFデータなし
                    loc_half = 0;
                    if tmp_hist1[1] >= 3 && tmp_hist1[1] > tmp_hist1[0] * 2 {
                        loc_det -= 1;
                    }
                    if loc_det <= MINUS {
                        loc_half = 1;
                    }
                } else if tmp_hist2[0] >= 3 && tmp_hist2[0] > tmp_hist2[2] * 2 {
                    loc_det -= 1; // ALL:中間 HALF:最小
                    loc_half = 0;
                } else if tmp_hist2[1] >= 3 && tmp_hist2[1] > tmp_hist2[0] * 2 {
                    loc_half = 0; // ALL:最大 HALF:中間
                } else {
                    loc_half = 1; // 全部中間
                }
            } else {
                if tmp_hist2[0] < 3 && tmp_hist2[1] < 3 {
                    // HALFデータなし
                    loc_half = 0;
                    if tmp_hist1[0] >= 3 && tmp_hist1[0] > tmp_hist1[2] * 2 {
                        loc_det -= 1;
                    }
                    if loc_det <= MINUS {
                        loc_half = 1;
                    }
                } else if tmp_hist2[0] >= 3 && tmp_hist2[0] > tmp_hist2[2] * 2 {
                    loc_half = 0; // ALL:中間 HALF:最小
                } else if tmp_hist1[2] >= 3 && tmp_hist1[2] > tmp_hist1[0] * 2 {
                    loc_det += 1; // ALL:最大 HALF:中間
                    loc_half = 0;
                } else {
                    loc_half = 1; // 全部中間
                }
            }
        } else {
            // フェード時
            loc_det = loc_max - 2;
            let mut val_max = hist[(loc_max - 2) as usize];
            if loc_det == 0 {
                loc_det = 1;
                val_max = hist[1];
            }
            for i in (loc_max - 1)..=(loc_max + 2) {
                let val = hist[i as usize];
                if val_max * 2 <= val {
                    val_max = val;
                    loc_det = i;
                }
            }
            if hist[(loc_det + 1) as usize] >= 3
                && hist[(loc_det - 1) as usize] * 2 < hist[(loc_det + 1) as usize]
                && hist[(loc_det + 1) as usize] > hist[loc_det as usize]
            {
                loc_det += 1;
            }
        }
    }
    // --- 結果をミリ秒換算で返す ---
    let mut msec_mrg = get_msec30fps_from_frm(loc_det - MINUS);
    if loc_half != 0 {
        msec_mrg += get_msec30fps_from_frm(loc_half) / 2;
    }
    msec_mrg
}

/// detectCutmrg
pub fn detect_cutmrg(ds: &mut Dataset, cnv: &Cnv) {
    // --- 自動検出結果 ---
    let cutin = detect_cutmrg_sub(ds, cnv, 1);
    let cutout = detect_cutmrg_sub(ds, cnv, 0);
    // --- 未設定だった場合は更新 ---
    if ds.ext_opt.fix_cut_in == 0 {
        ds.ext_opt.msec_cut_in = cutin;
    }
    if ds.ext_opt.fix_cut_out == 0 {
        ds.ext_opt.msec_cut_out = cutout;
    }

    // --- display auto detect CurMrgIn/Out ---
    let str_cutin = detect_cutmrg_dispval(cutin);
    let str_cutout = detect_cutmrg_dispval(cutout);
    // 原典 cout の診断。stdout は結果専用 (§3.5) のため ECHO 行で stderr へ
    eprintln!(
        "ECHO\tauto detect CutMrgIn={} CutMrgOut={}",
        str_cutin, str_cutout
    );

    let str_cutin = detect_cutmrg_dispval(ds.ext_opt.msec_cut_in);
    let str_cutout = detect_cutmrg_dispval(ds.ext_opt.msec_cut_out);
    eprintln!(
        "ECHO\tcurrent set CutMrgIn={} CutMrgOut={}",
        str_cutin, str_cutout
    );
}

/// updateCutmrg
pub fn update_cutmrg(ds: &mut Dataset, cnv: &Cnv) {
    let num_logo = ds.size_logo();
    let mut i = 0;
    while i < num_logo {
        let mut dtlogo = match ds.get_logo(i) {
            Some(l) => l,
            None => break,
        };
        let msec_val_exact = ds.msec_val_exact;
        let msec_val_near3 = ds.msec_val_near3;

        // rise設定
        {
            let intl = dtlogo.intl_rise;
            let mut msec_cutin = ds.ext_opt.msec_cut_in;
            if intl > 0 {
                let msec_org = dtlogo.org_rise;
                let msec_nxt = cnv.tc.get_msec_adjust_frm_from_msec(msec_org as i64, 1) as i32;
                msec_cutin += (msec_nxt - msec_org) / 2;
            }
            let mut n1 = dtlogo.org_rise - msec_cutin;
            let mut n2 = dtlogo.org_rise_l - msec_cutin;
            let mut n3 = dtlogo.org_rise_r - msec_cutin;
            if n1 < 0 {
                n1 = 0;
            }
            if n2 < 0 {
                n2 = 0;
            }
            if n3 < 0 {
                n3 = 0;
            }
            // --- シーンチェンジ位置により更新判断 ---
            let mut nsc_base = ds.nsc_from_msec_all(dtlogo.org_rise);
            let msec_base = ds.msec_scp(nsc_base);
            if nsc_base > 0 && ds.scp_still(nsc_base) {
                // 動きなし位置は除く
                nsc_base = -1;
            }
            let wide_cut_in = ds.ext_opt.wide_cut_in;
            if wide_cut_in == 0
                || (wide_cut_in == 2 && msec_cutin > msec_val_near3)
                || nsc_base < 0
                || dtlogo.org_rise_l - msec_val_exact > msec_base
                || dtlogo.org_rise_r + msec_val_exact < msec_base
                || (dtlogo.org_rise - msec_base >= (dtlogo.org_rise - n1) / 2)
            {
                // --- 更新 ---
                dtlogo.rise = cnv.tc.get_msec_from_frm(align_frm(cnv, n1)) as i32;
                dtlogo.rise_l = cnv.tc.get_msec_from_frm(align_frm(cnv, n2)) as i32;
                dtlogo.rise_r = cnv.tc.get_msec_from_frm(align_frm(cnv, n3)) as i32;
            } else {
                dtlogo.rise = dtlogo.org_rise;
                dtlogo.rise_l = dtlogo.org_rise_l;
                dtlogo.rise_r = dtlogo.org_rise_r;
            }
        }

        // fall設定
        {
            let intl = dtlogo.intl_fall;
            let mut msec_cutout = ds.ext_opt.msec_cut_out;
            if intl > 0 {
                let msec_org = dtlogo.org_fall;
                let msec_nxt = cnv.tc.get_msec_adjust_frm_from_msec(msec_org as i64, -1) as i32;
                msec_cutout += (msec_nxt - msec_org) / 2;
            }
            let mut n1 = dtlogo.org_fall + msec_cutout;
            let mut n2 = dtlogo.org_fall_l + msec_cutout;
            let mut n3 = dtlogo.org_fall_r + msec_cutout;
            if n1 < 0 {
                n1 = 0;
            }
            if n2 < 0 {
                n2 = 0;
            }
            if n3 < 0 {
                n3 = 0;
            }
            let msec_max = ds.msec_total_max;
            if n1 > msec_max {
                n1 = msec_max;
            }
            if n2 > msec_max {
                n2 = msec_max;
            }
            if n3 > msec_max {
                n3 = msec_max;
            }
            // --- シーンチェンジ位置により更新判断 ---
            let mut nsc_base = ds.nsc_from_msec_all(dtlogo.org_fall);
            let msec_base = ds.msec_scp_bk(nsc_base);
            if nsc_base > 0 && ds.scp_still(nsc_base) {
                // 動きなし位置は除く
                nsc_base = -1;
            }
            let wide_cut_out = ds.ext_opt.wide_cut_out;
            if wide_cut_out == 0
                || (wide_cut_out == 2 && msec_cutout > msec_val_near3)
                || nsc_base < 0
                || dtlogo.org_fall_l - msec_val_exact > msec_base
                || dtlogo.org_fall_r + msec_val_exact < msec_base
                || (msec_base - dtlogo.org_fall >= (n1 - dtlogo.org_fall) / 2)
            {
                // --- 更新 ---
                dtlogo.fall = cnv.tc.get_msec_from_frm(align_frm(cnv, n1)) as i32;
                dtlogo.fall_l = cnv.tc.get_msec_from_frm(align_frm(cnv, n2)) as i32;
                dtlogo.fall_r = cnv.tc.get_msec_from_frm(align_frm(cnv, n3)) as i32;
            } else {
                dtlogo.fall = dtlogo.org_fall;
                dtlogo.fall_l = dtlogo.org_fall_l;
                dtlogo.fall_r = dtlogo.org_fall_r;
            }
        }
        ds.set_logo(dtlogo, i);
        i += 1;
    }
}

/// getMsecAlignFromMsec helper
fn align_frm(cnv: &Cnv, msec: i32) -> i64 {
    cnv.tc.get_frm_from_msec(msec as i64)
}

/// adjustScpos
pub fn adjust_scpos(ds: &mut Dataset) {
    let num_scpos = ds.size_scp();
    let mut nd5_bsum = vec![0i32; num_scpos as usize];
    let mut nd5_sum = vec![0i32; num_scpos as usize];
    let mut nd5_nxt = vec![-1i32; num_scpos as usize];
    let mut nd15_flag = vec![0i32; num_scpos as usize];
    let msec_val_near1 = ds.msec_val_near1;
    let _msec_val_exact = ds.msec_val_exact;

    // ５の倍数秒のシーンチェンジ間隔場所を探索
    for i in 0..num_scpos {
        let msec_src = ds.msec_scp(i);
        let mut sec_last = 0;
        if i > 0 && i < num_scpos - 1 {
            // 先頭と最後は除く
            for j in 1..num_scpos - 1 {
                let msec_dst = ds.msec_scp(j);
                let mut calc = CalcDifInfo::default();
                if calc_dif_func(ds, &mut calc, msec_src, msec_dst) > 0 {
                    if calc.sgn < 0 {
                        if calc.sec % 5 == 0 {
                            nd5_bsum[i as usize] += 1;
                        }
                        if calc.sec % 15 == 0 {
                            nd15_flag[i as usize] = 1;
                        }
                    } else if calc.sgn > 0 {
                        if calc.sec % 5 == 0 {
                            if nd5_sum[i as usize] == 0 {
                                nd5_nxt[i as usize] = j;
                            }
                            if sec_last != calc.sec {
                                nd5_sum[i as usize] += 1;
                                sec_last = calc.sec;
                            }
                        }
                        if calc.sec % 15 == 0 {
                            nd15_flag[i as usize] = 1;
                        }
                    }
                }
            }
        }
        if nd5_bsum[i as usize] > 0 || nd5_sum[i as usize] > 0 {
            ds.set_scp_statpos(i, SCP_PRIOR_LV1);
        } else {
            ds.set_scp_statpos(i, SCP_PRIOR_NONE);
        }
    }
    // 15の倍数のシーンチェンジ間隔がある場所に関係あるかチェック
    for i in 0..num_scpos {
        if ds.scp_statpos(i) > SCP_PRIOR_NONE {
            let mut flag_nd15 = 0;
            // 関係性をチェック
            let mut j = i;
            while j >= 1 && j < num_scpos - 1 && flag_nd15 == 0 {
                // 先頭と最後は除く
                if nd15_flag[j as usize] > 0 {
                    // 15の倍数だった場合フラグを立てる
                    flag_nd15 = 1;
                }
                j = nd5_nxt[j as usize];
            }
            // 関係性があった場合のセット
            if flag_nd15 > 0 {
                j = i;
                while j >= 0 && j < num_scpos {
                    nd15_flag[j as usize] = 1;
                    j = nd5_nxt[j as usize];
                }
            }
        }
    }
    for i in 0..num_scpos {
        if nd15_flag[i as usize] == 0 {
            // 15の倍数に関連なければ同期ステータスを解除
            ds.set_scp_statpos(i, SCP_PRIOR_NONE);
        }
    }

    // ２５フレーム以内に５の倍数秒間隔シーンチェンジが重なる場合、片方のみにする
    {
        let mut ncmp_i = -1i32;
        let mut ncmp_msec = 0;
        for i in 1..num_scpos - 1 {
            // 先頭と最後は除く
            let msec_src = ds.msec_scp(i);
            if msec_src - ncmp_msec >= 870 && ncmp_i >= 0 {
                ncmp_i = -1;
            }
            if ds.scp_statpos(i) > SCP_PRIOR_NONE {
                if ncmp_i < 0 {
                    ncmp_i = i;
                    ncmp_msec = msec_src;
                } else {
                    let mut count_src = 0;
                    let mut count_cmp = 0;
                    for j in 1..num_scpos - 1 {
                        // 先頭と最後以外で比較
                        if ds.scp_statpos(j) > SCP_PRIOR_NONE {
                            let msec_dst = ds.msec_scp(j);
                            let mut calc1 = CalcDifInfo::default();
                            let mut calc2 = CalcDifInfo::default();
                            let flag1 = calc_dif_func(ds, &mut calc1, msec_src, msec_dst);
                            let flag2 = calc_dif_func(ds, &mut calc2, ncmp_msec, msec_dst);
                            if flag1 > 0
                                && flag2 > 0
                                && calc1.sec % 5 == 0
                                && calc1.sec == calc2.sec
                                && calc1.sec <= 60
                            {
                                if calc1.gap < calc2.gap {
                                    count_src += 3;
                                } else if calc1.gap > calc2.gap {
                                    count_cmp += 3;
                                }
                            } else if flag1 > 0
                                && (flag2 == 0 || calc2.sec % 5 != 0)
                                && calc1.sec % 5 == 0
                            {
                                if calc1.sec <= 60 {
                                    count_src += 1;
                                    if calc1.gap <= 3 {
                                        count_src += 1;
                                    }
                                }
                            } else if flag2 > 0
                                && (flag1 == 0 || calc1.sec % 5 != 0)
                                && calc2.sec % 5 == 0
                                && calc2.sec <= 60
                            {
                                count_cmp += 1;
                                if calc2.gap <= 3 {
                                    count_cmp += 1;
                                }
                            }
                        }
                    }
                    if count_src <= 1 && count_cmp <= 1 {
                        // 関連フレームが何もない場合は残す
                    } else if count_src > count_cmp {
                        for j in 0..ncmp_i {
                            // 差し替え元参照を入れ替える
                            if nd5_nxt[j as usize] == ncmp_i {
                                nd5_nxt[j as usize] = i;
                            }
                        }
                        ds.set_scp_statpos(ncmp_i, SCP_PRIOR_DUPE);
                        ncmp_i = i;
                        ncmp_msec = msec_src;
                    } else {
                        for j in 0..i {
                            if nd5_nxt[j as usize] == i {
                                // 差し替え元参照を入れ替える
                                nd5_nxt[j as usize] = ncmp_i;
                            }
                        }
                        ds.set_scp_statpos(i, SCP_PRIOR_DUPE);
                    }
                }
            }
        }
    }

    // 先頭から順番にシーンチェンジ位置をたどっていく
    {
        let mut nst = 0;
        while nst < num_scpos - 1 {
            // 非同期終了地点を検索
            let mut ned = nst + 1;
            while (ds.scp_statpos(ned) <= SCP_PRIOR_NONE || nd5_sum[ned as usize] == 0)
                && ned < num_scpos - 1
            {
                ned += 1;
            }
            // 少し先に正しい非同期終了地点がないか検索
            if ned < num_scpos - 1 {
                let mut nc_nxt = nd5_nxt[ned as usize];
                let mut i = ned + 1;
                while i < nc_nxt {
                    if ds.scp_statpos(i) >= SCP_PRIOR_NONE
                        && nd5_sum[i as usize] > 0
                        && nd5_sum[i as usize] > nd5_sum[ned as usize]
                    {
                        // 5の倍数間隔が多ければ非同期終了地点を変更
                        ned = i;
                        nc_nxt = nd5_nxt[ned as usize];
                    }
                    i += 1;
                }
            }

            // 非同期２地点間のシーンチェンジがないか検索
            // 間隔が5の倍数ではなくても非同期両端から3〜10秒のシーンチェンジは入れる
            let msec_st = ds.msec_scp(nst);
            let msec_ed = ds.msec_scp(ned);
            ds.set_scp_statpos(ned, SCP_PRIOR_DECIDE);
            let mut ncmp_i = -1i32;
            let mut ncmp_msec = 0;
            for i in nst + 1..ned {
                let msec_dst = ds.msec_scp(i);
                // １つ前の候補地点から離れたら１つ前の候補は確定する
                if msec_dst - ncmp_msec >= 940 && ncmp_i >= 0 {
                    ncmp_i = -1;
                }
                // 非同期２地点のどちらからも３秒以上離れていることが前提
                if msec_dst - msec_st >= 2670 && msec_ed - msec_dst >= 2670 {
                    let mut dis_ncmp = 0;
                    let mut dis_dst = 1;
                    // 前方からの間隔チェック（フレーム０は除く）
                    let mut calc1 = CalcDifInfo::default();
                    let mut calc2 = CalcDifInfo::default();
                    if calc_dif_func(ds, &mut calc1, msec_st, msec_dst) > 0
                        && nst > 0
                        && calc1.sec < 10
                    {
                        dis_dst = 0;
                        if ncmp_i >= 0
                            && nst != 0
                            && calc_dif_func(ds, &mut calc2, msec_st, ncmp_msec) > 0
                        {
                            if calc1.gap < calc2.gap {
                                dis_ncmp = 1;
                            } else {
                                dis_dst = 1;
                            }
                        }
                    }
                    // 後方からの間隔チェック
                    let mut calc1 = CalcDifInfo::default();
                    let mut calc2 = CalcDifInfo::default();
                    if calc_dif_func(ds, &mut calc1, msec_dst, msec_ed) > 0 && calc1.sec < 10 {
                        dis_dst = 0;
                        if ncmp_i >= 0
                            && ned < num_scpos - 1
                            && calc_dif_func(ds, &mut calc2, ncmp_msec, msec_ed) > 0
                        {
                            if calc1.gap < calc2.gap {
                                dis_ncmp = 1;
                            } else {
                                dis_dst = 1;
                            }
                        }
                    }
                    if dis_ncmp > 0 {
                        // １つ前の候補が無効と判定された場合
                        ds.set_scp_statpos(ncmp_i, SCP_PRIOR_DUPE);
                        ncmp_i = -1;
                    }
                    if dis_dst == 0 {
                        // 現在の候補が無効ではないと判定された場合
                        ds.set_scp_statpos(i, SCP_PRIOR_DECIDE);
                        ncmp_i = i;
                        ncmp_msec = msec_dst;
                    }
                }
            }
            // 同期しているシーンチェンジの最終位置を取得
            let mut nc_st = ned;
            let mut nc_nxt = nd5_nxt[nc_st as usize];
            let mut i = nc_st + 1;
            while nc_nxt > 0 && i <= nc_nxt && i < num_scpos {
                if i == nc_nxt {
                    // 同期フレーム到着時
                    nc_st = i;
                    nc_nxt = nd5_nxt[nc_st as usize];
                    ds.set_scp_statpos(nc_st, SCP_PRIOR_DECIDE);
                } else if i == num_scpos - 1 {
                    // 最終フレーム時
                    nc_st = i;
                    nc_nxt = 0;
                } else {
                    if nd5_sum[i as usize] > nd5_sum[nc_st as usize]
                        && ds.scp_statpos(i) >= SCP_PRIOR_NONE
                    {
                        // 同期フレーム到着前に候補があった場合
                        let ntmp_nxt = nd5_nxt[i as usize];
                        let msec_tmp_st = ds.msec_scp(i); // 新規候補位置
                        let msec_tmp_ed = ds.msec_scp(ntmp_nxt);
                        let msec_tmp_st2 = ds.msec_scp(nc_st); // 元の位置
                        let msec_tmp_ed2 = ds.msec_scp(nc_nxt);
                        let mut calc1 = CalcDifInfo::default();
                        let mut calc2 = CalcDifInfo::default();
                        let flag1 = calc_dif_func(ds, &mut calc1, msec_tmp_st, msec_tmp_ed);
                        let flag2 = calc_dif_func(ds, &mut calc2, msec_tmp_st2, msec_tmp_ed2);
                        if flag1 > 0 && calc1.gap <= msec_val_near1 {
                            // 誤差が小さかった場合のみ非同期地点とする
                            if flag2 == 0
                                || (calc2.sec % 15 != 0
                                    && (calc1.sec <= 30 || calc1.sec <= calc2.sec))
                                || (msec_tmp_ed <= msec_tmp_ed2 + 10 * 1000)
                            {
                                nc_nxt = 0;
                            }
                        }
                    }
                }
                i += 1;
            }
            nst = nc_st;
        }
    }

    // 開始直後のシーンチェンジのチェック
    {
        let mut ncmp_stat = SCP_PRIOR_DUPE;
        let mut i = 1;
        while ds.msec_scp(i) <= 30400 && i < num_scpos {
            // 30秒以内をチェック
            let mut ntmp_stat = ds.scp_statpos(i);
            if ntmp_stat == SCP_PRIOR_DECIDE && ncmp_stat <= SCP_PRIOR_LV1 {
                // 最初の確定候補
                ntmp_stat = SCP_PRIOR_LV1;
                if nd15_flag[i as usize] > 0 {
                    // 15の倍数が後で存在する場合のみ
                    let mut flag1 = 0;
                    let mut flag2 = 0;
                    let mut j = 0; // 念のためループ防止
                                   // １回目の間隔取得
                    let nc_st = i;
                    let mut nc_nxt = nc_st;
                    let mut calc1 = CalcDifInfo::default();
                    let mut calc2 = CalcDifInfo::default();
                    while nc_nxt > 0 && flag1 <= 1 && j < 100 {
                        nc_nxt = nd5_nxt[nc_nxt as usize];
                        j += 1;
                        if nc_nxt > 0 {
                            let msec_src = ds.msec_scp(nc_st);
                            let msec_dst = ds.msec_scp(nc_nxt);
                            flag1 = calc_dif_exact(ds, &mut calc1, msec_src, msec_dst);
                        }
                    }
                    // ２回目の間隔取得
                    let nc_st = nc_nxt;
                    while nc_nxt > 0 && flag2 <= 0 && j < 100 {
                        nc_nxt = nd5_nxt[nc_nxt as usize];
                        j += 1;
                        if nc_nxt > 0 {
                            let msec_src = ds.msec_scp(nc_st);
                            let msec_dst = ds.msec_scp(nc_nxt);
                            flag2 = calc_dif_exact(ds, &mut calc2, msec_src, msec_dst);
                        }
                    }
                    if flag1 > 0 && flag2 > 0 {
                        ntmp_stat = SCP_PRIOR_DECIDE;
                    }
                }
                ds.set_scp_statpos(i, ntmp_stat);
            }
            if ntmp_stat > SCP_PRIOR_LV1 {
                // 同期が既にある場合は何もしない
                ncmp_stat = ntmp_stat;
            } else if ds.msec_scp(i) <= 30400 {
                // 開始30秒以内が候補
                if ntmp_stat > ncmp_stat {
                    // 最初の同期なしシーンチェンジが候補
                    ncmp_stat = ntmp_stat;
                }
            }
            i += 1;
        }
    }

    // 単なる候補は初期状態に戻す
    for i in 1..num_scpos - 1 {
        // 先頭と最後は除く
        if ds.scp_statpos(i) == SCP_PRIOR_LV1 {
            ds.set_scp_statpos(i, SCP_PRIOR_NONE);
        }
    }
}

/// adjustLogo
pub fn adjust_logo(ds: &mut Dataset, cnv: &Cnv) {
    // ロゴデータ指定がない場合、全体をロゴデータとする
    if ds.size_logo() == 0 || ds.ext_opt.flag_no_logo > 0 {
        ds.clear_data_logo_all();
        let fall = ds.msec_total_max;
        let dtlogo = LogoRecord {
            fall,
            fall_l: fall,
            fall_r: fall,
            ..Default::default()
        };
        ds.push_logo(dtlogo);
        // ロゴ読み込みなしに変更
        ds.ext_opt.flag_no_logo = 1;
    }

    // 旧調整を行わない場合
    if ds.ext_opt.old_adjust == 0 {
        return;
    }

    let num_logo = ds.size_logo();
    let msec_val_near1 = ds.msec_val_near1;
    let msec_val_exact = ds.msec_val_exact;
    let msec_val_lap2 = ds.msec_val_lap2;
    let msec_val_near3 = ds.msec_val_near3;
    let msec_total_max = ds.msec_total_max;

    for i in 0..num_logo * 2 {
        let n = i / 2; // logo number
        let edge = i % 2; // 0:start edge  1:end edge

        let mut dtlogo = ds.get_logo(n).unwrap();

        let msec_lg_c: i32;
        let mut msec_lg_l: i32;
        let mut msec_lg_r: i32;
        if edge == EDGE_RISE {
            msec_lg_c = dtlogo.rise;
            msec_lg_l = dtlogo.rise_l;
            msec_lg_r = dtlogo.rise_r;
            dtlogo.stat_rise = LOGO_PRIOR_NONE; // 初期化
        } else {
            msec_lg_c = dtlogo.fall;
            msec_lg_l = dtlogo.fall_l;
            msec_lg_r = dtlogo.fall_r;
            dtlogo.stat_fall = LOGO_PRIOR_NONE; // 初期化
        }
        msec_lg_l -= msec_val_near1;
        msec_lg_r += msec_val_near1;
        let msec_lgn_l = msec_lg_l - 2 * 1000;
        let msec_lgn_r = msec_lg_r + 2 * 1000;

        let mut lgr_num = -1i32;
        let mut lgr_stat = LOGO_PRIOR_DUPE;
        let mut lgr_dif = -1;
        let mut lgr_msec = -1;
        let mut lgm1_num = -1;
        let mut lgm1_dif = -1;
        let mut lgm1_msec = -1;
        let mut lgm2_num = -1;
        let mut lgm2_dif = -1;
        let mut lgm2_msec = -1;
        let mut lgn_num = -1;
        let mut lgn_dif = -1;
        let mut lgn2_num = -1;
        let mut lgn2_dif = -1;
        // ロゴ位置に近い無音シーンチェンジを検索
        let num_scp = ds.size_scp();
        for j in 1..num_scp - 1 {
            let dtscp = match ds.get_scp(j) {
                Some(s) => s,
                None => continue,
            };
            let msec_sc = if edge == EDGE_RISE {
                dtscp.msec
            } else {
                dtscp.msbk
            };
            let msec_smute_s = dtscp.msmute_s - msec_val_exact;
            let msec_smute_e = dtscp.msmute_e + msec_val_exact;
            let stat_sc = prior_logo_from_scp(dtscp.statpos);
            let msec_dif = (msec_sc - msec_lg_c).abs();

            // ロゴ区間内にある無音シーンチェンジ検出
            if msec_sc >= msec_lg_l
                && msec_sc <= msec_lg_r
                && ((stat_sc >= lgr_stat && (msec_dif < lgr_dif || lgr_dif < 0))
                    || stat_sc > lgr_stat)
            {
                lgr_num = j;
                lgr_stat = stat_sc;
                lgr_dif = msec_dif;
                lgr_msec = msec_sc;
            }
            // 誤検出調査用のロゴ区間付近無音シーンチェンジ検出
            if msec_sc >= msec_lgn_l
                && msec_sc <= msec_lgn_r
                && (msec_dif < lgn_dif || lgn_dif < 0)
                && stat_sc <= LOGO_PRIOR_LV1
            {
                lgn_num = j;
                lgn_dif = msec_dif;
            }
            // 誤検出調査2用のロゴ区間付近無音シーンチェンジ検出
            if msec_sc >= msec_smute_s
                && msec_sc <= msec_smute_e
                && (msec_dif < lgn2_dif || lgn2_dif < 0)
                && stat_sc <= LOGO_PRIOR_LV1
            {
                lgn2_num = j;
                lgn2_dif = msec_dif;
            }
            // 近くの確定無音シーンチェンジ位置を検出
            if (msec_dif < lgm1_dif || lgm1_dif < 0) && stat_sc == LOGO_PRIOR_DECIDE {
                lgm1_num = j;
                lgm1_dif = msec_dif;
                lgm1_msec = msec_sc;
            }
            // 隣の確定無音シーンチェンジ位置を検出
            if (msec_dif < lgm2_dif || lgm2_dif < 0)
                && stat_sc == LOGO_PRIOR_DECIDE
                && msec_dif > msec_val_lap2
                && ((edge == EDGE_RISE && msec_sc < msec_lg_c)
                    || (edge != EDGE_RISE && msec_sc > msec_lg_c))
            {
                lgm2_num = j;
                lgm2_dif = msec_dif;
                lgm2_msec = msec_sc;
            }
        }
        // ロゴの確定度
        if edge == EDGE_RISE {
            dtlogo.stat_rise = lgr_stat;
        } else {
            dtlogo.stat_fall = lgr_stat;
        }
        // ロゴ位置を変更するか判断
        let mut flag_lg_change = 0;
        let mut flag_sc_change = 0;
        let mut num_sc_change = -1i32;
        let mut msec_change = 0i32;
        // 傍に無音シーンチェンジがあった場合
        if lgm1_dif > msec_val_exact && lgm1_dif <= msec_val_near3 && lgm2_num >= 0 {
            let mut calc1 = CalcDifInfo::default();
            let mut calc2 = CalcDifInfo::default();
            let mut flag1 = calc_dif_func(ds, &mut calc1, lgm2_msec, msec_lg_c);
            let mut flag2 = calc_dif_func(ds, &mut calc2, lgm2_msec, lgm1_msec);
            if flag1 == 1 {
                flag1 = 0;
            }
            if flag2 == 1 {
                flag2 = 0;
            }
            // ロゴ範囲内に確定した無音シーンチェンジがあった場合の処理
            if lgr_stat == LOGO_PRIOR_DECIDE {
                if flag1 == 0 && flag2 == 0 {
                    // 両方とも単位距離外の場合
                    flag2 = 1; // ロゴを無音シーンチェンジ位置に移す
                } else if flag1 != 0 && flag2 != 0 {
                    // 両方とも単位距離内の場合
                    if calc1.sec != calc2.sec {
                        // 時間が全然違う場合
                        flag1 = 0; // ロゴを無音シーンチェンジ位置に移す
                    } else {
                        calc1.gap += msec_val_near1; // ロゴ側を厳しく取る
                    }
                }
            }
            // 確定シーンチェンジから単位距離になる位置に近い方を選択する
            if flag1 == 0 && flag2 == 0 {
                // 両方とも単位距離外の場合
            } else if (calc1.sec == calc2.sec && calc1.gap > calc2.gap) || flag1 == 0 {
                flag_lg_change = 1;
                msec_change = lgm1_msec;
                // 原典 JlsReformData.cpp の受動的代入（この値は経路により後続で読まれない）
                #[allow(unused_assignments)]
                {
                    lgr_num = lgm1_num;
                }
            } else if (calc1.sec == calc2.sec && calc1.gap < calc2.gap) || flag2 == 0 {
                flag_sc_change = 1;
                num_sc_change = lgm1_num;
                msec_change = msec_lg_c;
            }
        } else if lgr_stat == LOGO_PRIOR_DECIDE {
            // ロゴ範囲内に確定した無音シーンチェンジがあった場合
            flag_lg_change = 1;
            msec_change = lgr_msec;
        } else if lgn_num >= 0 && lgm2_num >= 0 {
            // 不確定／確定シーンチェンジが近くに両方ある場合
            let mut calc = CalcDifInfo::default();
            if calc_dif_func(ds, &mut calc, lgm2_msec, msec_lg_c) > 0
                && (calc.sec % 15 == 0 || calc.sec == 10)
            {
                // ロゴが確定シーンチェンジから単位距離になる場合
                if lgn_dif > msec_val_exact {
                    // ロゴと無音シーンチェンジが隣接していない場合
                    flag_sc_change = 1;
                    num_sc_change = lgn_num;
                    msec_change = msec_lg_c;
                }
            }
        } else if lgn_num >= 0
            && edge == EDGE_RISE
            && dtlogo.rise <= 30 * 1000 + 400
            && (n == 0 || n == 1)
        {
            // 先頭部分のロゴ
            if lgr_num >= 0 {
                // 候補無音シーンチェンジあり
                if msec_lg_l <= 0 && lgr_msec > msec_lg_c * 2 {
                    // 0地点の方が近い場合何もしない
                } else {
                    flag_lg_change = 1;
                    msec_change = lgr_msec;
                }
            } else {
                // ロゴ区間にシーンチェンジなし
                if lgn2_num >= 0 {
                    // ロゴ区間に無音あり
                    if ds.scp_statpos(lgn2_num) <= SCP_PRIOR_NONE {
                        flag_sc_change = 1;
                        num_sc_change = lgn2_num;
                        msec_change = msec_lg_c;
                    }
                }
            }
        }
        // ロゴ変更が最終シーンチェンジより後のフレームだった場合は変更せず
        if flag_lg_change > 0 && msec_change > msec_total_max {
            flag_lg_change = 0;
        }
        // ロゴ位置の変更
        if flag_lg_change > 0 {
            if edge == EDGE_RISE {
                if dtlogo.rise != msec_change {
                    dtlogo.rise = msec_change;
                    if dtlogo.rise_l > msec_change {
                        dtlogo.rise_l = msec_change;
                    }
                    if dtlogo.rise_r < msec_change {
                        dtlogo.rise_r = msec_change;
                    }
                    let frm_old = cnv.tc.get_frm_from_msec(dtlogo.rise as i64);
                    let frm_change = cnv.tc.get_frm_from_msec(msec_change as i64);
                    if (frm_old - frm_change).abs() > 2 {
                        eprintln!("ECHO\tchange logo : {} -> {}", frm_old, frm_change);
                    }
                }
            } else {
                if dtlogo.fall != msec_change {
                    dtlogo.fall = msec_change;
                    if dtlogo.fall_l > msec_change {
                        dtlogo.fall_l = msec_change;
                    }
                    if dtlogo.fall_r < msec_change {
                        dtlogo.fall_r = msec_change;
                    }
                    let frm_old = cnv.tc.get_frm_from_msec(dtlogo.fall as i64);
                    let frm_change = cnv.tc.get_frm_from_msec(msec_change as i64);
                    if (frm_old - frm_change).abs() > 2 {
                        eprintln!("ECHO\tchange logo : {} -> {}", frm_old, frm_change);
                    }
                }
            }
        }
        // ロゴデータ更新
        ds.set_logo(dtlogo, n);

        // シーンチェンジ位置変更
        if flag_sc_change > 0 {
            let mut dtscp = ds.get_scp(num_sc_change).unwrap();
            if edge == EDGE_RISE {
                if dtscp.msec != msec_change {
                    let frm_old = cnv.tc.get_frm_from_msec(dtscp.msec as i64);
                    let frm_change = cnv.tc.get_frm_from_msec(msec_change as i64);
                    let msec_new = cnv.tc.get_msec_from_frm(frm_change) as i32;
                    let msec_newm1 = cnv.tc.get_msec_from_frm(frm_change - 1) as i32;
                    eprintln!("ECHO\tchange scpos : {} -> {}", frm_old, frm_change);
                    let tmp_stat = dtscp.statpos;
                    dtscp.statpos = SCP_PRIOR_DUPE;
                    ds.set_scp(dtscp, num_sc_change);
                    ds.insert_scpos(msec_new, msec_newm1, num_sc_change, tmp_stat);
                }
            } else {
                if dtscp.msbk != msec_change {
                    let frm_old = cnv.tc.get_frm_from_msec(dtscp.msec as i64);
                    let frm_change = cnv.tc.get_frm_from_msec(msec_change as i64);
                    let msec_new = cnv.tc.get_msec_from_frm(frm_change) as i32;
                    let msec_newp1 = cnv.tc.get_msec_from_frm(frm_change + 1) as i32;
                    eprintln!("ECHO\tchange scpos : {} -> {}", frm_old, frm_change);
                    let tmp_stat = dtscp.statpos;
                    dtscp.statpos = SCP_PRIOR_DUPE;
                    ds.set_scp(dtscp, num_sc_change);
                    ds.insert_scpos(msec_newp1, msec_new, num_sc_change, tmp_stat);
                }
            }
        }
    }
}

/// adjustData
pub fn adjust_data(ds: &mut Dataset, cnv: &Cnv) {
    ds.set_flag_setup_adj(true);
    adjust_scpos(ds);
    // --- ロゴマージン調整 ---
    if (ds.ext_opt.nl_g_exact & 0x01) == 0 {
        detect_cutmrg(ds, cnv);
        update_cutmrg(ds, cnv);
    }
    adjust_logo(ds, cnv);
}
