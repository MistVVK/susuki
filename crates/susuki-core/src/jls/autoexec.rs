//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! JlsAutoScript: Auto-family command execution layer (first slice:
//! exeCmdParam + AutoUp/AutoBorder/AutoIClear/AutoChg/AutoIns/AutoDel).
//! Mirrors JlsAutoScript.cpp 384-1005, 3174-3264 helpers.

use super::autoarg::{AutoArg, CmdAutoType, ParamAuto};
use super::cmdarg::CmdArg;
use super::cmdlimit::CmdLimit;
use super::cnvstr::Cnv;
use super::dataset::*;
use super::dataset_search::*;
use super::reform_d::mk_reform_target;
use super::tables::{CmdType, ConfigVarType, OptType};

// CmdTrSpEcID (JlsNameSpace.hpp 577)
pub const TRSPEC_NONE: i32 = 0;
pub const TRSPEC_OFF: i32 = 1;
pub const TRSPEC_TR: i32 = 2;
pub const TRSPEC_SP: i32 = 3;
pub const TRSPEC_EC: i32 = 4;
pub const TRSPEC_LG: i32 = 5;

// SearchDirType (JlsNameSpace.hpp 148)
pub const SEARCH_DIR_PREV: i32 = 0;
pub const SEARCH_DIR_NEXT: i32 = 1;

/// isScpArTypeBorder
pub fn is_scp_ar_type_border(ar: i32) -> bool {
    (SCP_AR_B_UNIT..=SCP_AR_B_OTHER).contains(&ar)
}

/// edgeFromNrf
fn edge_from_nrf(nrf: i32) -> i32 {
    nrf % 2
}

/// getAutoParam
fn get_auto_param(arg: &mut AutoArg, tp: ParamAuto) -> i32 {
    arg.get_param(tp)
}

/// JlsAutoScript::getTermNext (endfix defaulting)
pub fn term_next_a(ds: &Dataset, arg: &mut AutoArg, term: &mut Term) -> bool {
    if !term.endfix {
        // エッジ情報未設定時
        let noedge = if get_auto_param(arg, ParamAuto::C_noedge) != 0 {
            SCP_END_NOEDGE
        } else {
            SCP_END_EDGEIN
        };
        Dataset::set_term_endtype(term, noedge);
    }
    ds.term_next(term)
}

/// getElgNextKeep
fn elg_next_keep(ds: &Dataset, elg: &mut ElgCurrent) -> bool {
    let elgbak = *elg;
    let ret = ds.elg_pt_next(elg);
    if !ret {
        *elg = elgbak;
    }
    ret
}

/// exeCmdParam
pub fn exe_cmd_param(cmdarg: &CmdArg, arg: &mut AutoArg) -> CmdAutoType {
    // --- コマンドから種類を選択 ---
    let mut cmdtype = CmdAutoType::None;
    match cmdarg.cmdsel {
        CmdType::AutoCut => {
            if cmdarg.select_auto_sub == TRSPEC_TR {
                cmdtype = CmdAutoType::CutTR;
            } else if cmdarg.select_auto_sub == TRSPEC_EC {
                cmdtype = CmdAutoType::CutEC;
            } else {
                eprintln!("error: AutoCut is need TR/EC");
            }
        }
        CmdType::AutoAdd => {
            if cmdarg.select_auto_sub == TRSPEC_TR {
                cmdtype = CmdAutoType::AddTR;
            } else if cmdarg.select_auto_sub == TRSPEC_EC {
                cmdtype = CmdAutoType::AddEC;
            } else if cmdarg.select_auto_sub == TRSPEC_SP {
                cmdtype = CmdAutoType::AddSP;
            } else {
                eprintln!("error: AutoAdd is need TR/EC/SP");
            }
        }
        CmdType::AutoEdge => cmdtype = CmdAutoType::Edge,
        CmdType::AutoCM => cmdtype = CmdAutoType::AtCM,
        CmdType::AutoBorder => cmdtype = CmdAutoType::AtBorder,
        CmdType::AutoIClear => cmdtype = CmdAutoType::AtIClear,
        CmdType::AutoUp => cmdtype = CmdAutoType::AtUP,
        CmdType::AutoIns => cmdtype = CmdAutoType::Ins,
        CmdType::AutoDel => cmdtype = CmdAutoType::Del,
        _ => {
            if cmdarg.get_opt(OptType::FlagAutoChg) > 0 {
                cmdtype = CmdAutoType::AtChg;
            } else {
                eprintln!(
                    "error:internal setting at autoCmd(Command: {})",
                    cmdtype as i32
                );
            }
        }
    }

    // --- パラメータ格納 ---
    if !matches!(cmdtype, CmdAutoType::None) && cmdarg.cmdsel != CmdType::AutoCM {
        // AutoCMは別途実行時に別クラスで設定
        arg.set_param(cmdarg, cmdtype);
    }
    cmdtype
}

/// startAutoUp
pub fn start_auto_up(ds: &mut Dataset, cnv: &Cnv, arg: &mut AutoArg) -> bool {
    // --- 実行有無確認 ---
    let prm_c_exe = get_auto_param(arg, ParamAuto::C_exe);
    if prm_c_exe == 0 {
        return false;
    }
    // --- コマンド実行 ---
    let restruct = get_auto_param(arg, ParamAuto::C_restruct);
    let mut nlg_fall = 0i32;
    let mut nsc_lastfall = 0i32;
    let mut cont_bk = true;
    loop {
        let num_scpos = ds.size_scp();
        // --- ロゴ位置を取得して次のロゴ位置に移動 ---
        let (ret_nlg, msec_rise, msec_fall, cont_next) = ds.result_logo_next(cnv, nlg_fall);
        nlg_fall = ret_nlg;
        // --- ロゴ期間がある場合 ---
        if nlg_fall >= 0 {
            // --- ロゴ番号取得。ない時に挿入によるシーンチェンジ番号(nsc)変更あり ---
            let nsc_rise = ds.nsc_force_msec(cnv, msec_rise, EDGE_RISE);
            let nsc_fall = ds.nsc_force_msec(cnv, msec_fall, EDGE_FALL);
            // --- 前立ち下がりと次立ち上がり位置の間をCM化 ---
            if nsc_lastfall < nsc_rise {
                mk_reform_target(ds, cnv, nsc_lastfall, nsc_rise, false, restruct);
                // ロゴ無
            }
            // --- ロゴ期間をロゴ有に設定 ---
            if nsc_rise < nsc_fall {
                mk_reform_target(ds, cnv, nsc_rise, nsc_fall, true, restruct); // ロゴ有
            }
            // --- 前立ち下がりと次立ち上がりが同一位置で連続扱いでない場合は切れ目設定 ---
            if nsc_lastfall == nsc_rise && !cont_bk && nsc_lastfall > 0 {
                ds.set_scp_chap(nsc_lastfall, SCP_CHAP_DUNIT);
            }
            nsc_lastfall = nsc_fall;
            cont_bk = cont_next; // 次ロゴとの間がロゴが切れ目なしの連続か
        }
        // --- 最後のロゴ終了から最終位置までの設定 ---
        else if nsc_lastfall >= 0 && nsc_lastfall < num_scpos - 1 {
            mk_reform_target(ds, cnv, nsc_lastfall, num_scpos - 1, false, restruct);
            // ロゴ無
        }
        if nlg_fall < 0 {
            break;
        }
    }
    true
}

/// startAutoBorder
pub fn start_auto_border(
    ds: &mut Dataset,
    cnv: &Cnv,
    arg: &mut AutoArg,
    autoscope: (i32, i32),
) -> bool {
    let prm_c_exe = get_auto_param(arg, ParamAuto::C_exe);
    if prm_c_exe == 0 {
        return false;
    }
    let mut exeflag = false;
    {
        let mut term = Term::default();
        let mut cont = true;
        while cont {
            cont = term_next_a(ds, arg, &mut term);
            if cont && autoscope.0 <= term.msec_st && term.msec_ed <= autoscope.1 {
                let arstat_term = ds.term_arstat(&term);
                if is_scp_ar_type_border(arstat_term) {
                    let restruct = get_auto_param(arg, ParamAuto::C_restruct);
                    let arstat = if restruct != 0 {
                        if arstat_term == SCP_AR_B_UNIT {
                            SCP_AR_L_UNIT
                        } else {
                            SCP_AR_L_OTHER
                        }
                    } else if arstat_term == SCP_AR_B_UNIT {
                        SCP_AR_N_UNIT
                    } else {
                        SCP_AR_N_OTHER
                    };
                    exeflag = true;
                    ds.set_term_arstat(&term, arstat);
                }
            }
        }
    }
    let _ = cnv;
    exeflag
}

/// startAutoIClear
pub fn start_auto_iclear(
    ds: &mut Dataset,
    _cnv: &Cnv,
    arg: &mut AutoArg,
    autoscope: (i32, i32),
) -> bool {
    let prm_c_exe = get_auto_param(arg, ParamAuto::C_exe);
    if prm_c_exe == 0 {
        return false;
    }
    let mut exeflag = false;
    {
        let mut term = Term::default();
        let mut cont = true;
        while cont {
            cont = term_next_a(ds, arg, &mut term);
            if cont && autoscope.0 <= term.msec_st && term.msec_ed <= autoscope.1 {
                ds.set_term_arext(&term, SCP_AREXT_NONE);
                exeflag = true;
            }
        }
        // --- 検索位置の初期化 ---
        ds.rec_hold.msec_tr_point = -1; // CutTRコマンドの設定位置
    }
    exeflag
}

/// getNscElgFromNrf
pub fn get_nsc_elg_from_nrf(ds: &Dataset, nrf_base: i32) -> i32 {
    let edgelogo = edge_from_nrf(nrf_base);
    let msec_base_nrf = ds.msec_logo_nrf(nrf_base);

    // --- 構成上の対応するロゴ端を取得 ---
    let mut nsc_base_elg = -1;
    let mut flag_cont = true;
    let mut elg = ElgCurrent::default();
    if nrf_base >= 0 {
        // --- ロゴに対応する構成区切り位置取得 ---
        while elg_next_keep(ds, &mut elg) && flag_cont {
            if edgelogo == EDGE_RISE
                && (elg.msec_last_fall < msec_base_nrf || elg.msec_last_fall <= 0)
                && msec_base_nrf < elg.msec_fall
            {
                nsc_base_elg = elg.nsc_rise;
                flag_cont = false;
            }
            if edgelogo == EDGE_FALL
                && elg.msec_last_rise < msec_base_nrf
                && msec_base_nrf < elg.msec_rise
            {
                nsc_base_elg = elg.nsc_last_fall;
                flag_cont = false;
            }
        }
        // --- 最後のロゴ立ち下がり検出 ---
        if flag_cont && edgelogo == EDGE_FALL && elg.msec_last_rise <= msec_base_nrf {
            nsc_base_elg = elg.nsc_last_fall;
        }
    }
    // --- 有効な位置関係か確認 ---
    if nsc_base_elg >= 0 {
        let msec_base_elg = ds.msec_scp_edge(nsc_base_elg, edgelogo);
        // --- 基準位置とロゴ扱い位置が離れていたら無効化する処理 ---
        if (msec_base_nrf - msec_base_elg).abs() > 31 * 1000 {
            // 31秒以上
            nsc_base_elg = -1;
        }
    }
    nsc_base_elg
}

/// startAutoChg
pub fn start_auto_chg(ds: &mut Dataset, cnv: &Cnv, _arg: &mut AutoArg, lim: &CmdLimit) -> bool {
    let mut exeflag = false;
    // --- 対象の無音SCを設定（データ挿入によるシーンチェンジ番号(nsc)変更あり） ---
    let nrf_base = lim.get_logo_base_nrf();
    let mut edgelogo = EDGE_RISE;
    if nrf_base >= 0 {
        edgelogo = edge_from_nrf(nrf_base);
    }
    let mut nsc_target = lim.get_result_target_sel();
    let msec_target = lim.get_target_range_force();
    if nsc_target < 0 && msec_target >= 0 {
        nsc_target = ds.nsc_force_msec(cnv, msec_target, edgelogo);
    }
    // --- ロゴ基準ではない場合(NextTail)の処理 ---
    if nrf_base < 0 {
        if nsc_target > 0 {
            exeflag = true;
            let nsc_arstat = ds.nsc_next_scp_decide(nsc_target - 1, SCP_END_EDGEIN);
            if nsc_arstat != nsc_target {
                // 新規推測構成区切りなら推測設定
                let logoon = ds.is_elg_in_scp(nsc_arstat);
                mk_reform_target(ds, cnv, -1, nsc_target, logoon, 0);
            }
        }
        return exeflag; // ロゴ基準ではない場合(NextTail)は終了
    }
    // --- 基準位置を取得 ---
    let nsc_base = get_nsc_elg_from_nrf(ds, nrf_base);
    // --- 有効位置であれば実行 ---
    if nsc_target != nsc_base && nsc_base >= 0 && nsc_target >= 0 {
        exeflag = true;
        if edgelogo == EDGE_RISE {
            if nsc_base < nsc_target {
                mk_reform_target(ds, cnv, nsc_base, nsc_target, false, 1); // ロゴ無
            } else {
                mk_reform_target(ds, cnv, nsc_target, nsc_base, true, 1); // ロゴ有
            }
        }
        if edgelogo == EDGE_FALL {
            if nsc_base < nsc_target {
                mk_reform_target(ds, cnv, nsc_base, nsc_target, true, 1); // ロゴ有
            } else {
                mk_reform_target(ds, cnv, nsc_target, nsc_base, false, 1); // ロゴ無
            }
        }
    }
    exeflag
}

/// subInsDelGetBase
fn sub_ins_del_get_base(ds: &Dataset, lim: &CmdLimit) -> i32 {
    let mut nsc_base = -1;
    let limit_nscend = lim.get_result_target_end();
    let limit_nscbase = lim.get_logo_base_nsc();
    let limit_nrfbase = lim.get_logo_base_nrf();

    if limit_nscend >= 0 {
        nsc_base = limit_nscend;
    } else if limit_nscbase >= 0 {
        nsc_base = limit_nscbase;
    } else if limit_nrfbase >= 0 {
        nsc_base = ds.nsc_from_nrf(limit_nrfbase, ds.msec_val_lap2, SCP_CHAP_DECIDE, false);
    }
    nsc_base
}

/// subInsDelGetRange; returns (nsc(target,base), msec(target,base)) and base existence
fn sub_ins_del_get_range(
    ds: &mut Dataset,
    cnv: &Cnv,
    lim: &CmdLimit,
) -> ((i32, i32), (i32, i32), bool) {
    let edge = lim.get_logo_base_edge();
    // --- 基準位置取得 ---
    let mut nsc_base = sub_ins_del_get_base(ds, lim);
    let msec_base = ds.msec_scp_edge(nsc_base, edge);
    // --- ターゲット位置取得 ---
    let mut nsc_target = lim.get_result_target_sel();
    let mut msec_target = lim.get_target_range_force();
    if nsc_target < 0 {
        if msec_target >= 0 {
            nsc_target = ds.nsc_force_msec(cnv, msec_target, edge);
        }
    } else {
        msec_target = ds.msec_scp(nsc_target);
    }
    // --- 基準位置の更新（ターゲット位置が新規追加の時変わるため） ---
    if nsc_base >= 0 {
        nsc_base = ds.nsc_from_msec_all_edgein(msec_base);
    }
    // --- 結果代入 ---
    let ok = !(nsc_target < 0 || nsc_base < 0);
    ((nsc_target, nsc_base), (msec_target, msec_base), ok)
}

/// subInsDelAddUnit
fn sub_ins_del_add_unit(ds: &mut Dataset, arg: &mut AutoArg, rmsec: (i32, i32), flag_add: bool) {
    // --- -unitオプション指定時のみ動作 ---
    let n_unit = get_auto_param(arg, ParamAuto::C_unit);
    if n_unit > 0 {
        // --- 位置情報取得 ---
        let mut nsc_from = ds.nsc_from_msec_all_edgein(rmsec.0);
        let nsc_to = ds.nsc_from_msec_all_edgein(rmsec.1);
        // --- 設定内容（AutoInsは独立追加／AutoDelは独立解除） ---
        let chap_type = if flag_add {
            SCP_CHAP_DUNIT
        } else {
            SCP_CHAP_DFORCE
        };
        // --- 実施 ---
        if nsc_from > 0 {
            ds.set_scp_chap(nsc_from, chap_type);
        }
        if nsc_to > 0 {
            ds.set_scp_chap(nsc_to, chap_type);
        }
        let _ = &mut nsc_from;
    }
}

/// subInsDelChangeArExt
fn sub_ins_del_change_ar_ext(
    ds: &mut Dataset,
    arg: &mut AutoArg,
    rmsec: (i32, i32),
    flag_add: bool,
) {
    // --- 追加の更新有無 ---
    let id_opt = get_auto_param(arg, ParamAuto::V_info);
    if id_opt == TRSPEC_NONE {
        return;
    }
    // --- -info設定に対応する属性設定値 ---
    let ar_ext = if flag_add {
        match id_opt {
            TRSPEC_TR => AREXT_L_TRKEEP,
            TRSPEC_SP => AREXT_L_SP,
            TRSPEC_EC => AREXT_L_EC,
            TRSPEC_LG => AREXT_L_LGADD,
            _ => SCP_AREXT_NONE,
        }
    } else {
        match id_opt {
            TRSPEC_TR => AREXT_L_TRCUT,
            TRSPEC_SP => AREXT_L_SP,
            TRSPEC_EC => AREXT_L_ECCUT,
            TRSPEC_LG => AREXT_L_LGCUT,
            _ => SCP_AREXT_NONE,
        }
    };
    // --- 更新 ---
    {
        // --- 位置情報取得 ---
        let mut nsc_from = ds.nsc_from_msec_all_edgein(rmsec.0);
        let mut nsc_to = ds.nsc_from_msec_all_edgein(rmsec.1);
        // --- fromを前側、toを後側 ---
        if nsc_from > nsc_to {
            std::mem::swap(&mut nsc_from, &mut nsc_to);
        }
        // --- 片側でもマイナスの時は設定なし ---
        if nsc_from < 0 {
            return;
        }
        // --- 処理開始 ---
        let mut term = Term::default();
        let mut cont = true;
        while cont {
            cont = term_next_a(ds, arg, &mut term);
            if cont && nsc_from <= term.nsc_st && term.nsc_ed <= nsc_to {
                ds.set_term_arext(&term, ar_ext);
            }
        }
    }
}

/// startAutoIns
pub fn start_auto_ins(ds: &mut Dataset, cnv: &Cnv, arg: &mut AutoArg, lim: &CmdLimit) -> bool {
    let mut exeflag = false;
    // --- 実行有無確認 ---
    let prm_c_exe = get_auto_param(arg, ParamAuto::C_exe);
    if prm_c_exe == 0 {
        return false;
    }
    // --- コマンド実行 ---
    let restruct = get_auto_param(arg, ParamAuto::C_restruct);
    let ((nsc_st, nsc_ed), (msec_st, msec_ed), ok) = sub_ins_del_get_range(ds, cnv, lim);
    if ok {
        // --- 構成情報更新 ---
        mk_reform_target(ds, cnv, nsc_st, nsc_ed, true, restruct); // ロゴ有
        exeflag = true;
        // --- 属性情報の追加変更 ---
        let flag_add = true; // 追加
        sub_ins_del_add_unit(ds, arg, (msec_st, msec_ed), flag_add);
        sub_ins_del_change_ar_ext(ds, arg, (msec_st, msec_ed), flag_add);
    }
    exeflag
}

/// startAutoDel
pub fn start_auto_del(ds: &mut Dataset, cnv: &Cnv, arg: &mut AutoArg, lim: &CmdLimit) -> bool {
    let mut exeflag = false;
    // --- 実行有無確認 ---
    let prm_c_exe = get_auto_param(arg, ParamAuto::C_exe);
    if prm_c_exe == 0 {
        return false;
    }
    // --- コマンド実行 ---
    let restruct = get_auto_param(arg, ParamAuto::C_restruct);
    let ((nsc_st, nsc_ed), (msec_st, msec_ed), ok) = sub_ins_del_get_range(ds, cnv, lim);
    if ok {
        // --- 構成情報更新 ---
        mk_reform_target(ds, cnv, nsc_st, nsc_ed, false, restruct); // ロゴ無
        exeflag = true;
        // --- 属性情報の追加変更 ---
        let flag_add = false; // 追加ではなく削除
        sub_ins_del_add_unit(ds, arg, (msec_st, msec_ed), flag_add);
        sub_ins_del_change_ar_ext(ds, arg, (msec_st, msec_ed), flag_add);
    }
    exeflag
}

/// startAutoCutTR
pub fn start_auto_cut_tr(
    ds: &mut Dataset,
    cnv: &Cnv,
    arg: &mut AutoArg,
    autoscope: (i32, i32),
) -> bool {
    // --- 初期状態設定 ---
    let msec_spc = ds.msec_val_spc;
    // --- 予告設定位置（CM構成内部分割を自動で行う位置） ---
    ds.rec_hold.msec_tr_point = autoscope.0;
    // --- 実行有無確認 ---
    let prm_c_exe = get_auto_param(arg, ParamAuto::C_exe);
    if prm_c_exe == 0 {
        return false;
    }

    // --- 予告開始位置を取得 ---
    let nsc_trstart = sub_cut_tr_get_loc_st(ds, cnv, arg, autoscope);
    if nsc_trstart < 0 {
        // 予告候補が見つからない場合
        return false;
    }

    // --- パラメータ取得 ---
    let prm_msec_wlogo_trmax = ds.get_config(ConfigVarType::msecWLogoTRMax);
    let prm_msec_wcomp_trmax = ds.get_config(ConfigVarType::msecWCompTRMax);
    let prm_limit = get_auto_param(arg, ParamAuto::V_limit);
    let prm_trsumprd = get_auto_param(arg, ParamAuto::V_trsumprd);
    let prm_c_cutst = get_auto_param(arg, ParamAuto::C_cutst);

    // --- 予告位置を設定 ---
    let mut ret = false;
    {
        let mut state_cut = 0;
        let mut ncut_rest = prm_limit;
        let mut nlg = 0i32;
        let mut nlg_start = 0i32;
        let mut sec_tr_total = 0i32;
        let mut elg = ElgCurrent::default();
        let mut det_logo = elg_next_keep(ds, &mut elg);
        let mut term = Term {
            ini: nsc_trstart,
            ..Default::default()
        };
        let mut cont = term_next_a(ds, arg, &mut term);
        while cont {
            let arstat_term = ds.term_arstat(&term);
            let arext_term = ds.term_arext(&term);
            let flag_scope = ds.check_scope_term(&term, autoscope.0, autoscope.1);
            if flag_scope
                && (is_scp_ar_type_logo_border(arstat_term) || term.nsc_st == nsc_trstart)
                && arext_term != AREXT_L_LGCUT
                && arext_term != AREXT_L_LGADD
            {
                // --- シーンチェンジに対応するロゴ位置取得 ---
                while term.nsc_ed > elg.nsc_fall && det_logo {
                    nlg += 1;
                    det_logo = elg_next_keep(ds, &mut elg);
                }
                // --- 前フレームからの期間 ---
                let msec_dif_term = term.msec_ed - term.msec_st;
                let sec_dif_term = cnv.tc.get_sec_from_msec(msec_dif_term as i64) as i32;
                // --- 初回チェック ---
                if state_cut == 0 {
                    nlg_start = nlg;
                    state_cut = 1;
                }
                // --- カット開始条件判断 ---
                let mut flag_cut1st = false;
                if state_cut == 1
                    && ncut_rest == 0
                    && sec_tr_total >= prm_trsumprd
                    && prm_c_cutst != 3
                    && (((sec_dif_term % 15 == 0) && prm_c_cutst == 0)
                        || ((sec_dif_term >= 15) && prm_c_cutst == 1)
                        || prm_c_cutst == 2)
                {
                    state_cut = 2;
                    flag_cut1st = true;
                }
                // --- カット開始後の処理 ---
                if state_cut == 2 {
                    if (elg.msec_fall - elg.msec_rise <= prm_msec_wlogo_trmax + msec_spc
                        || nlg == nlg_start)
                        // ロゴが２分以内か開始ロゴ
                        && elg.msec_fall - term.msec_ed <= prm_msec_wlogo_trmax + msec_spc
                        // ロゴ終了まで２分以内
                        && msec_dif_term <= prm_msec_wcomp_trmax + msec_spc
                        // 構成が60秒以内
                        && elg.msec_fall <= autoscope.1 + msec_spc
                    // ロゴ終了地点が範囲内
                    {
                        if is_scp_ar_type_border(arstat_term) && flag_cut1st {
                            // カット開始直後のBorder
                            ds.set_term_arext(&term, AREXT_L_TRRAW);
                        } else {
                            sub_cut_tr_set_cut(ds, cnv, arg, &mut term);
                        }
                        ret = true;
                    } else {
                        state_cut = 3; // 完全終了
                    }
                }
                // --- カット開始まで残す処理 ---
                if state_cut == 1 {
                    if ncut_rest > 0 {
                        ds.set_term_arext(&term, AREXT_L_TRKEEP);
                        ncut_rest -= 1;
                        sec_tr_total += sec_dif_term;
                    } else {
                        // エンドカード判断待ち
                        ds.set_term_arext(&term, AREXT_L_TRRAW);
                    }
                }
            } else if flag_scope && arstat_term == SCP_AR_N_OTHER {
                // ロゴなし不明構成
                if state_cut == 1 || state_cut == 2 {
                    // カット期間中
                    ds.set_term_arext(&term, AREXT_N_TRCUT);
                }
            }
            // --- 次の位置を設定 ---
            cont = term_next_a(ds, arg, &mut term);
        }
    }
    ret
}

/// subCutTRGetLocSt
fn sub_cut_tr_get_loc_st(ds: &Dataset, cnv: &Cnv, arg: &mut AutoArg, autoscope: (i32, i32)) -> i32 {
    let mut nsc_cand = -1;
    let mut flag_cand = false;
    let mut det_logo = true;
    let mut elg = ElgCurrent::default();
    while det_logo {
        det_logo = elg_next_keep(ds, &mut elg);
        if det_logo {
            sub_cut_tr_get_loc_st_sub(ds, cnv, arg, &mut nsc_cand, &mut flag_cand, autoscope, elg);
        }
    }
    nsc_cand
}

/// subCutTRGetLocStSub
fn sub_cut_tr_get_loc_st_sub(
    ds: &Dataset,
    cnv: &Cnv,
    arg: &mut AutoArg,
    r_nsc_cand: &mut i32,
    r_flag_cand: &mut bool,
    autoscope: (i32, i32),
    elg: ElgCurrent,
) {
    // --- 設定値 ---
    let msec_spc = ds.msec_val_spc;
    let prm_wcomp_spmin = ds.get_config(ConfigVarType::secWCompSPMin);
    let prm_wcomp_spmax = ds.get_config(ConfigVarType::secWCompSPMax);
    let prm_msec_wlogo_trmax = ds.get_config(ConfigVarType::msecWLogoTRMax);
    let prm_msec_wcomp_trmax = ds.get_config(ConfigVarType::msecWCompTRMax);

    // --- 使用パラメータ ---
    let prm_c_from = get_auto_param(arg, ParamAuto::C_from);
    let prm_c_lgpre = get_auto_param(arg, ParamAuto::C_lgpre);
    let prm_trscope = get_auto_param(arg, ParamAuto::V_trscope);
    let prm_tr1stprd = get_auto_param(arg, ParamAuto::V_tr1stprd);

    // --- ロゴ終了が期間内 ---
    if elg.msec_fall <= autoscope.0 + msec_spc || elg.msec_fall >= autoscope.1 + msec_spc {
        return;
    }

    // --- 前回状態を取り込み ---
    let mut nsc_cand = *r_nsc_cand;
    let mut flag_cand = *r_flag_cand;

    // --- ロゴ期間が予告用の長さか確認 ---
    let mut longlogo = false;
    {
        if elg.msec_rise < autoscope.0 - msec_spc
            || elg.msec_fall - elg.msec_rise > prm_msec_wlogo_trmax + msec_spc
        {
            longlogo = true;
        }
    }
    // --- ロゴ位置設定（ロゴエッジ処理している所は除く） ---
    let mut rnsc_logo = (elg.nsc_rise, elg.nsc_fall);
    {
        let mut term = Term {
            ini: rnsc_logo.1, // 開始位置
            ..Default::default()
        };
        let mut cont = term_next_a_prev(ds, arg, &mut term);
        while cont {
            if term.nsc_st < rnsc_logo.0 {
                cont = false;
            } else {
                let arext_term = ds.term_arext(&term);
                if arext_term == AREXT_L_LGCUT || arext_term == AREXT_L_LGADD {
                    rnsc_logo.1 = term.nsc_st; // エッジ処理している所は除く
                } else {
                    cont = false;
                }
            }
            if cont {
                cont = term_next_a_prev(ds, arg, &mut term);
            }
        }
    }
    let msec_dst_fall = ds.msec_scp(rnsc_logo.1);

    // --- ロゴ開始直後にすぐ終わるかチェック ---
    let mut shortlogo = false;
    {
        let mut term = Term {
            ini: rnsc_logo.0, // 開始位置
            ..Default::default()
        };
        let cont = term_next_a(ds, arg, &mut term);
        if term.nsc_ed != rnsc_logo.1 || !cont {
            // no-op
        } else if cnv
            .tc
            .get_sec_from_msec((msec_dst_fall - elg.msec_rise) as i64)
            < 15
        {
            let mut next_elg = elg;
            let valid_next = elg_next_keep(ds, &mut next_elg);
            // --- 次のロゴがないか終了地点よりも後まで続く場合、単体短時間フラグ ---
            if !valid_next || next_elg.msec_fall >= autoscope.1 - msec_spc {
                shortlogo = true;
            }
        }
    }
    // --- ロゴ手前でも位置確定がまだで予告候補があればチェック ---
    if rnsc_logo.0 < rnsc_logo.1 && !flag_cand && (prm_c_lgpre == 1 || prm_c_lgpre == 2) {
        // --- １つ前の構成を取得 ---
        let mut term1 = Term {
            ini: rnsc_logo.0, // 開始位置
            ..Default::default()
        };
        let cont1 = term_next_a_prev(ds, arg, &mut term1);
        // --- ロゴ手前がCM15秒単位検出ではない時 ---
        if cont1 && !ds.is_term_cm_unit(&term1) {
            // --- ２つ前の構成を取得 ---
            let mut term2 = term1;
            let mut cont2 = term_next_a_prev(ds, arg, &mut term2);
            if term2.nsc_st <= elg.nsc_last_fall {
                cont2 = false;
            }
            // --- ３つ前の構成を取得 ---
            // 原典では term3 に copy した後 getTermPrev(term2) を呼ぶため
            // term2 がもう一つ戻る（term3 は２つ前の位置のまま）
            let term3 = term2;
            let mut cont3 = term_next_a_prev(ds, arg, &mut term2);
            if term3.nsc_st <= elg.nsc_last_fall || !cont2 {
                cont3 = false;
            }
            // --- 共通の構成期間制限を確認 ---
            let mut flag_common_cond = false;
            if !longlogo && (prm_c_lgpre == 2 || shortlogo) {
                flag_common_cond = true;
            }
            // --- ２つ前はCM15秒単位だった場合 ---
            if cont2 && ds.is_term_cm_unit(&term2) {
                // １つ前の構成が番組提供の構成外であれば予告に追加
                let sec_dif_term1 = cnv
                    .tc
                    .get_sec_from_msec((term1.msec_ed - term1.msec_st) as i64)
                    as i32;
                if (sec_dif_term1 < prm_wcomp_spmin || sec_dif_term1 > prm_wcomp_spmax)
                    && flag_common_cond
                    && term1.msec_st >= autoscope.0
                    && (sec_dif_term1 <= prm_tr1stprd || prm_tr1stprd == 0)
                {
                    nsc_cand = term1.nsc_st;
                    flag_cand = true;
                }
            } else if cont3 && ds.is_term_cm_unit(&term3) {
                // ２つ前もCM15秒単位検出ではなく、３つ前がCM15秒単位だった場合
                // １つ前の構成が番組提供の構成外であれば予告に追加
                let sec_dif_term2 = cnv
                    .tc
                    .get_sec_from_msec((term2.msec_ed - term2.msec_st) as i64)
                    as i32;
                if (sec_dif_term2 < prm_wcomp_spmin || sec_dif_term2 > prm_wcomp_spmax)
                    && flag_common_cond
                    && term1.msec_st >= autoscope.0
                    && (sec_dif_term2 <= prm_tr1stprd || prm_tr1stprd == 0)
                {
                    nsc_cand = term2.nsc_st;
                    flag_cand = true;
                }
            }
        }
    }
    // --- 自動判別でCM明けのロゴがあれば優先させる場合のため ---
    if rnsc_logo.0 < rnsc_logo.1 && !flag_cand && nsc_cand >= 0 {
        if nsc_cand >= 0 {
            // --- 短い単一構成以外のロゴは新候補にするため過去候補をクリア ---
            let mut term = Term {
                ini: rnsc_logo.0,
                ..Default::default()
            };
            if term_next_a_prev(ds, arg, &mut term) {
                // --- ロゴ手前がCM検出ではない時はCM明けから ---
                if !ds.is_term_cm_unit(&term) || !shortlogo {
                    nsc_cand = -1;
                }
            }
        }
        if nsc_cand >= 0 {
            // --- ロゴ期間終了位置から次の構成位置を取得 ---
            let mut term = Term {
                ini: rnsc_logo.1,
                ..Default::default()
            };
            if term_next_a(ds, arg, &mut term) {
                // --- ロゴ直後がCM検出ではない時はCM明けから ---
                if !ds.is_term_cm_unit(&term) {
                    nsc_cand = -1;
                }
            }
        }
    }
    // --- ロゴ期間内のシーンチェンジ確認 ---
    if rnsc_logo.0 < rnsc_logo.1 {
        let mut term = Term {
            ini: rnsc_logo.0,
            ..Default::default()
        };
        let mut cont = term_next_a(ds, arg, &mut term);
        while cont {
            let arstat_term = ds.term_arstat(&term);
            let arext_term = ds.term_arext(&term);
            if ds.check_scope_term(&term, autoscope.0, autoscope.1)
                && term.nsc_st >= rnsc_logo.0
                && term.nsc_ed <= rnsc_logo.1
                && is_scp_ar_type_logo(arstat_term)
                && arext_term != AREXT_L_LGCUT
                && arext_term != AREXT_L_LGADD
            {
                // --- 前回からの期間を取得 ---
                let msec_logo_ed = ds.msec_scp(rnsc_logo.1);
                let msec_dif_term = term.msec_ed - term.msec_st;
                let sec_dif_term = cnv.tc.get_sec_from_msec(msec_dif_term as i64) as i32;
                let sec_dif_lged = cnv
                    .tc
                    .get_sec_from_msec((msec_logo_ed - term.msec_ed) as i64)
                    as i32;
                // --- 構成が予告期間以上あったら本編と認識 ---
                if msec_dif_term > prm_msec_wcomp_trmax + msec_spc {
                    longlogo = true;
                    nsc_cand = -1;
                } else if nsc_cand < 0 {
                    // 構成期間が所定以下なら設定
                    if sec_dif_term <= prm_tr1stprd || prm_tr1stprd == 0 {
                        nsc_cand = term.nsc_st;
                    }
                }
                // --- 本編後の同ロゴ内位置だった場合条件を満たすか確認 ---
                if longlogo {
                    // --- CM明けから数える場合は本編ロゴ中は無効化 ---
                    if prm_c_from == 1
                        || ((sec_dif_term % 15 != 0) && prm_c_from == 5)
                        || ((sec_dif_term % 5 != 0) && prm_c_from == 4)
                        || sec_dif_lged > prm_trscope
                    {
                        nsc_cand = -1;
                    }
                }
            }
            // --- 確定するロゴ期間の場合はフラグ設定 ---
            if longlogo && prm_c_from == 2 {
                flag_cand = false;
            } else if nsc_cand >= 0 {
                flag_cand = true;
            }
            // --- 次の位置を設定 ---
            cont = term_next_a(ds, arg, &mut term);
        }
    }

    *r_nsc_cand = nsc_cand;
    *r_flag_cand = flag_cand;
}

/// getTermPrev wrapper
fn term_next_a_prev(ds: &Dataset, arg: &mut AutoArg, term: &mut Term) -> bool {
    if !term.endfix {
        let noedge = if get_auto_param(arg, ParamAuto::C_noedge) != 0 {
            SCP_END_NOEDGE
        } else {
            SCP_END_EDGEIN
        };
        Dataset::set_term_endtype(term, noedge);
    }
    ds.term_prev(term)
}

/// subCutTRSetCut
fn sub_cut_tr_set_cut(ds: &mut Dataset, cnv: &Cnv, arg: &mut AutoArg, term: &mut Term) {
    // --- 期間によりカット状態設定 ---
    let msec_dif_term = term.msec_ed - term.msec_st;
    let sec_dif_term = cnv.tc.get_sec_from_msec(msec_dif_term as i64) as i32;
    if sec_dif_term >= 14 {
        ds.set_term_arext(term, AREXT_L_TRCUT);
    } else {
        ds.set_term_arext(term, AREXT_L_ECCUT);
    }
    // --- 設定値読み込み ---
    let prm_msec_wcomp_trmax = ds.get_config(ConfigVarType::msecWCompTRMax);
    let mgn_cm_detect = ds.get_config(ConfigVarType::msecMgnCmDetect);
    // --- 前構成と15秒単位の結合処理 ---
    {
        let mut preterm = *term;
        let mut nloop = 0;
        let mut cont = true;
        while cont && nloop <= 2 {
            // --- 結合はカット状態同士のみ。それ以外では終了 ---
            let arext_term = ds.term_arext(&preterm);
            if arext_term != AREXT_L_TRCUT && arext_term != AREXT_L_ECCUT {
                cont = false;
            }
            if cont {
                // --- 15秒単位からずれがない所を探す ---
                let msec_dif_target = term.msec_ed - preterm.msec_st;
                let msec_gap = ((msec_dif_target + 7500) % 15000) - 7500;
                if msec_dif_target > prm_msec_wcomp_trmax + mgn_cm_detect {
                    // 60秒を超えたら終了
                    cont = false;
                } else if msec_gap <= mgn_cm_detect {
                    if nloop > 0 {
                        // --- 15秒単位の所があれば途中を結合 ---
                        let mut tmpterm = preterm;
                        for _i in 0..nloop {
                            ds.set_scp_chap(tmpterm.nsc_ed, SCP_CHAP_CPOSIT);
                            term_next_a(ds, arg, &mut tmpterm);
                        }
                        // --- termを設定し直し ---
                        let nsc_end = term.nsc_ed;
                        *term = Term::default();
                        term.ini = preterm.nsc_st;
                        let mut tmpcont = term_next_a(ds, arg, term);
                        while term.nsc_ed < nsc_end && tmpcont {
                            // 念のため確認
                            tmpcont = term_next_a(ds, arg, term);
                        }
                        ds.set_term_arext(term, AREXT_L_TRCUT);
                    }
                    cont = false;
                }
            }
            if cont {
                cont = term_next_a_prev(ds, arg, &mut preterm);
                nloop += 1;
            }
        }
    }
}

/// startAutoCutEC
pub fn start_auto_cut_ec(
    ds: &mut Dataset,
    cnv: &Cnv,
    arg: &mut AutoArg,
    autoscope: (i32, i32),
) -> bool {
    // --- 使用パラメータ ---
    let prm_limit = get_auto_param(arg, ParamAuto::V_limit);

    // --- 初期状態設定 ---
    let mut ret = false;

    // --- 位置情報を取得 ---
    let mut local_cntcut = vec![0i32; ds.size_scp() as usize];
    let mut ovw_force = 0i32;
    let n_cutdst =
        sub_cut_ec_get_loc_st(ds, cnv, arg, &mut local_cntcut, &mut ovw_force, autoscope);

    // --- 確認した位置でカット処理 ---
    {
        let mut state_cut = 0;
        let mut term = Term::default();
        let mut cont = term_next_a(ds, arg, &mut term);
        while cont {
            let arext_term = ds.term_arext(&term);
            let cntcut_term = local_cntcut[term.nsc_ed as usize];
            // --- 範囲内でロゴ有の配置位置だった場合のみ実行 ---
            if ds.check_scope_term(&term, autoscope.0, autoscope.1) {
                if state_cut == 0 {
                    if cntcut_term == n_cutdst {
                        state_cut = 1;
                    } else if cntcut_term > 0 && prm_limit > 0 {
                        ds.set_term_arext(&term, AREXT_L_EC);
                    }
                }
                if state_cut == 1 {
                    if cntcut_term > 0 {
                        if (arext_term != AREXT_L_EC && arext_term != AREXT_L_SP)
                            || (ovw_force > 0 && ovw_force <= cntcut_term)
                        {
                            ds.set_term_arext(&term, AREXT_L_ECCUT);
                            ret = true;
                        }
                    } else if arext_term == AREXT_L_TRCUT || arext_term == AREXT_L_ECCUT {
                        state_cut = 2;
                        cont = false;
                    }
                }
            } else if term.msec_ed > autoscope.1 {
                cont = false;
            }
            // --- 次位置を設定 ---
            if cont {
                cont = term_next_a(ds, arg, &mut term);
            }
        }
    }
    ret
}

/// subCutECCheckScope
fn sub_cut_ec_check_scope(sec_dif: i32, prm_c_sel: i32, prm_period: i32, prm_maxprd: i32) -> bool {
    (sec_dif < 15 && prm_c_sel == 1)
        || (sec_dif <= prm_period && prm_c_sel == 3)
        || (sec_dif >= prm_period && sec_dif < 15 && prm_c_sel == 4)
        || (sec_dif >= prm_period && sec_dif < prm_maxprd && prm_c_sel == 5)
        || (sec_dif == prm_period && prm_c_sel == 2)
}

// 原典 JlsAutoScript.cpp の msec.ed > msec.ed 自己比較（常に偽）を検証するため維持
#[allow(clippy::eq_op)]
/// subCutECGetLocSt
fn sub_cut_ec_get_loc_st(
    ds: &Dataset,
    cnv: &Cnv,
    arg: &mut AutoArg,
    local_cntcut: &mut [i32],
    r_ovw_force: &mut i32,
    autoscope: (i32, i32),
) -> i32 {
    // --- 使用パラメータ ---
    let mut prm_limit = get_auto_param(arg, ParamAuto::V_limit);
    let prm_period = get_auto_param(arg, ParamAuto::V_period);
    let prm_maxprd = get_auto_param(arg, ParamAuto::V_maxprd);
    let prm_c_sel = get_auto_param(arg, ParamAuto::C_sel);
    let prm_c_cutsp = get_auto_param(arg, ParamAuto::C_cutsp);
    let prm_c_cutla = get_auto_param(arg, ParamAuto::C_cutla);
    let prm_c_cutlp = get_auto_param(arg, ParamAuto::C_cutlp);
    let prm_c_cut30 = get_auto_param(arg, ParamAuto::C_cut30);

    // --- 初期状態設定 ---
    for v in local_cntcut.iter_mut() {
        *v = 0;
    }
    let mut prmflag_cutlast = 0;
    if prm_limit < 0 || prm_c_cutla > 0 || prm_c_cutlp > 0 {
        prmflag_cutlast = 1;
    }

    // --- 先頭位置を検索 ---
    let mut nsc_cand = -1;
    {
        let mut flag_cand = false;
        let mut flag_existec = false;
        let mut flag_end = false;
        let mut term = Term::default();
        let mut cont = term_next_a(ds, arg, &mut term);
        while cont && !flag_end {
            let arstat_term = ds.term_arstat(&term);
            let arext_term = ds.term_arext(&term);
            // --- 範囲内でロゴ有の配置位置だった場合のみ実行 ---
            if ds.check_scope_term(&term, autoscope.0, autoscope.1) {
                let sec_dif_term = cnv
                    .tc
                    .get_sec_from_msec((term.msec_ed - term.msec_st) as i64)
                    as i32;
                // --- エンドカードとして有効な所を検索する ---
                if arext_term == AREXT_L_TRRAW {
                    if !flag_cand {
                        nsc_cand = term.nsc_st;
                        flag_cand = true;
                    }
                } else if arext_term == AREXT_L_EC {
                    if !flag_cand || !flag_existec {
                        nsc_cand = term.nsc_st;
                        flag_cand = true;
                        flag_existec = true;
                    }
                } else if arext_term == AREXT_L_SP {
                    if prmflag_cutlast > 0 || prm_c_cutsp > 0 {
                        // カット対象とする場合
                        if nsc_cand < 0 {
                            nsc_cand = term.nsc_st;
                        }
                    } else {
                        nsc_cand = -1;
                        flag_cand = false;
                    }
                } else if arext_term == AREXT_L_TRCUT || arext_term == AREXT_L_ECCUT {
                    flag_end = true;
                } else if arext_term == AREXT_L_TRKEEP {
                    nsc_cand = -1;
                    flag_cand = false;
                    if prm_c_cut30 > 0 && sec_dif_term == 30 {
                        prm_limit = 0; // エンドカードなし
                    }
                } else if is_scp_ar_type_logo(arstat_term)
                    && arext_term != AREXT_L_LGCUT
                    && arext_term != AREXT_L_LGADD
                    && !flag_cand
                {
                    // 検索範囲内かチェック
                    if sub_cut_ec_check_scope(sec_dif_term, prm_c_sel, prm_period, prm_maxprd) {
                        if prmflag_cutlast > 0 && nsc_cand < 0 {
                            nsc_cand = term.nsc_st;
                        }
                    } else if !flag_cand && nsc_cand >= 0 {
                        nsc_cand = -1;
                    }
                }
            } else if term.msec_ed > autoscope.1 {
                flag_end = true;
            }
            // --- 次位置を設定 ---
            cont = term_next_a(ds, arg, &mut term);
        }
    }

    // --- 各シーンチェンジ位置順番付け ---
    let mut n_cutmax = 0;
    let mut sec_dif_last = 0;
    if nsc_cand >= 0 {
        let mut flag_end = false;
        let mut flag_cont = false;
        let mut term = Term {
            ini: nsc_cand,
            ..Default::default()
        };
        let mut cont = term_next_a(ds, arg, &mut term);
        while cont && !flag_end {
            let arstat_term = ds.term_arstat(&term);
            let arext_term = ds.term_arext(&term);
            let sec_dif_term =
                cnv.tc
                    .get_sec_from_msec((term.msec_ed - term.msec_st) as i64) as i32;
            // --- 範囲内でロゴ有の配置位置だった場合のみ実行 ---
            if ds.check_scope_term(&term, autoscope.0, autoscope.1) {
                let mut flag_valid = false;
                // --- エンドカードとして有効な所を検索する ---
                if arext_term == AREXT_L_TRRAW || arext_term == AREXT_L_EC {
                    flag_valid = true;
                } else if arext_term == AREXT_L_SP {
                    flag_cont = true; // 継続フラグを立てる
                    if prmflag_cutlast > 0 || prm_c_cutsp > 0 {
                        // カット対象とする場合
                        flag_valid = true;
                    }
                } else if arext_term == AREXT_L_TRCUT || arext_term == AREXT_L_ECCUT {
                    flag_end = true; // 既存カットがあれば終了
                } else if is_scp_ar_type_logo(arstat_term)
                    && arext_term != AREXT_L_LGCUT
                    && arext_term != AREXT_L_LGADD
                {
                    // 検索範囲内かチェック
                    if sub_cut_ec_check_scope(sec_dif_term, prm_c_sel, prm_period, prm_maxprd) {
                        if prmflag_cutlast > 0 || flag_cont {
                            flag_valid = true;
                        }
                    } else {
                        flag_cont = false; // 継続は終了
                    }
                }
                if flag_valid {
                    // --- 開始位置取得 ---
                    if sub_cut_ec_check_scope(sec_dif_term, prm_c_sel, prm_period, prm_maxprd) {
                        n_cutmax += 1;
                        local_cntcut[term.nsc_ed as usize] = n_cutmax;
                        sec_dif_last = sec_dif_term; // 最後の期間を記憶
                    }
                }
            } else {
                // 原典 quirk: term.msec.ed > term.msec.ed は常に偽
                let x = term.msec_ed;
                if x > x {
                    flag_end = true;
                }
            }
            // --- 次の位置 ---
            cont = term_next_a(ds, arg, &mut term);
        }
    }
    // --- カット位置の番号を決定 ---
    let mut n_cutdst;
    let mut ovw_force = 0;
    if prm_limit >= 0 {
        // 先頭から数える場合
        n_cutdst = prm_limit + 1;
        if n_cutmax > 0 && prmflag_cutlast > 0 {
            // 最後をカットする場合
            if prm_c_cutla > 0
                || (prm_c_cutlp > 0
                    && sub_cut_ec_check_scope(sec_dif_last, 2, prm_period, prm_maxprd))
            {
                ovw_force = n_cutmax;
                if n_cutdst > n_cutmax {
                    n_cutdst = n_cutmax;
                }
            }
        }
    } else {
        // 最後から数える場合
        n_cutdst = n_cutmax + prm_limit + 1;
        ovw_force = n_cutdst;
        if prm_c_cutlp > 0 {
            // 期間条件がある場合
            if !sub_cut_ec_check_scope(sec_dif_last, 2, prm_period, prm_maxprd) {
                n_cutdst = n_cutmax + 100; // カット無効化
                ovw_force = 0;
            }
        }
    }
    *r_ovw_force = ovw_force;
    n_cutdst
}

/// calcDifGap
pub fn calc_dif_gap(msec1: i32, msec2: i32) -> i32 {
    let msec_dif = (msec1 - msec2).abs();
    let msec_gap_pre = msec_dif - (msec_dif / 1000 * 1000);
    if msec_gap_pre > 500 {
        1000 - msec_gap_pre
    } else {
        msec_gap_pre
    }
}

#[derive(Clone, Copy, Default)]
pub struct AddLocInfo {
    pub type_logo: i32,
    pub type_tr: i32,
    pub type_sp: i32,
    pub type_ec: i32,
    pub sec_dif_logo: i32,
    pub sec_dif_tr: i32,
    pub sec_dif_sp: i32,
    pub sec_dif_ec: i32,
    pub type_nolast: i32,
    pub type_endlogo: i32,
    pub sec_dif_sc: i32,
}

#[derive(Clone, Copy, Default)]
pub struct AddExistInfo {
    pub trailer: bool,
    pub sponsor: bool,
    pub sec_tr: i32,
    pub divide_cm: bool,
}

/// startAutoAddSP
pub fn start_auto_add_sp(
    ds: &mut Dataset,
    cnv: &Cnv,
    arg: &mut AutoArg,
    autoscope: (i32, i32),
) -> bool {
    // --- 使用パラメータ ---
    let prm_c_exe = get_auto_param(arg, ParamAuto::C_exe);
    let prm_limit = get_auto_param(arg, ParamAuto::V_limit);
    let prm_c_limtrsum = get_auto_param(arg, ParamAuto::C_limtrsum);
    let prm_trsumprd = get_auto_param(arg, ParamAuto::V_trsumprd);
    let prm_c_in1 = get_auto_param(arg, ParamAuto::C_in1);

    // --- 実行有無確認 ---
    if prm_c_exe == 0 {
        return false;
    }

    // --- limit確認、予告・番組提供有無確認 ---
    let mut exist_info = AddExistInfo::default();
    let cnt_limit = sub_add_get_limit(ds, cnv, arg, &mut exist_info, CmdAutoType::AddSP, autoscope);
    // --- limit上限確認、上限以上であれば何もせず終了 ---
    if prm_limit != 0 && cnt_limit >= prm_limit {
        return false;
    }
    // --- 予告秒数が指定以上あれば何もせず終了 ---
    if prm_c_limtrsum > 0 && exist_info.sec_tr >= prm_trsumprd {
        return false;
    }
    // --- 既存の番組提供情報は検索位置に使わない ---
    exist_info.sponsor = false;

    // --- 優先順位最大のシーンチェンジ位置確認（prm_limit == 0の時は書き換えも実行） ---
    let mut nsc_prior = sub_add_search(ds, cnv, arg, CmdAutoType::AddSP, exist_info, autoscope);
    if nsc_prior < 0 {
        return false;
    }

    // --- 一番優先順位の高い候補を番組提供とする ---
    if nsc_prior > 0 && prm_limit > 0 {
        // --- 予告と認識した最初の内容を番組提供部分とする場合 ---
        if ds.scp_arext(nsc_prior) == AREXT_L_TRKEEP {
            let mut flag_detcand = false;
            let mut term = Term {
                ini: nsc_prior,
                ..Default::default()
            };
            let mut cont = term_next_a(ds, arg, &mut term);
            loop {
                if cont {
                    // --- 検索範囲を超えたら終了 ---
                    if term.msec_ed > autoscope.1 {
                        cont = false;
                    } else {
                        // --- 候補の確認 ---
                        let arstat_term = ds.term_arstat(&term);
                        let arext_term = ds.term_arext(&term);
                        if is_scp_ar_type_logo_border(arstat_term) && arext_term != AREXT_L_TRKEEP {
                            flag_detcand = true;
                        }
                    }
                }
                if flag_detcand || !cont {
                    break;
                }
            }
            // 候補がある場合は予告認識を１つ後にずらす
            if cont {
                ds.set_term_arext(&term, AREXT_L_TRKEEP);
            }
            // 候補がない場合は強制設定時以外は中止
            else if prm_c_in1 < 2 {
                nsc_prior = -1;
            }
        }
        // --- 番組提供部分を設定 ---
        if nsc_prior > 0 {
            // 候補位置がカット状態だった場合は次以降のカット位置を確認
            if ds.scp_arext(nsc_prior) == AREXT_L_TRCUT {
                let mut term = Term {
                    ini: nsc_prior,
                    ..Default::default()
                };
                let mut cont = term_next_a(ds, arg, &mut term);
                while cont {
                    let arext_term = ds.term_arext(&term);
                    // エンドカードカットだった場合は判断前に戻す
                    if arext_term == AREXT_L_ECCUT {
                        ds.set_term_arext(&term, AREXT_L_TRRAW);
                    }
                    cont = term_next_a(ds, arg, &mut term);
                }
            }
            sub_add_revise_unit_cm(ds, cnv, arg, nsc_prior); // 合併１５秒単位CM処理
            ds.set_scp_arext(nsc_prior, AREXT_L_SP);
        }
    }
    true
}

/// startAutoAddEC
pub fn start_auto_add_ec(
    ds: &mut Dataset,
    cnv: &Cnv,
    arg: &mut AutoArg,
    autoscope: (i32, i32),
) -> bool {
    let prm_c_exe = get_auto_param(arg, ParamAuto::C_exe);
    let prm_limit = get_auto_param(arg, ParamAuto::V_limit);
    let prm_c_limtrsum = get_auto_param(arg, ParamAuto::C_limtrsum);
    let prm_trsumprd = get_auto_param(arg, ParamAuto::V_trsumprd);

    if prm_c_exe == 0 {
        return false;
    }

    let mut exist_info = AddExistInfo::default();
    let cnt_limit = sub_add_get_limit(ds, cnv, arg, &mut exist_info, CmdAutoType::AddEC, autoscope);
    if prm_limit != 0 && cnt_limit >= prm_limit {
        return false;
    }
    if prm_c_limtrsum > 0 && exist_info.sec_tr >= prm_trsumprd {
        return false;
    }

    let nsc_prior = sub_add_search(ds, cnv, arg, CmdAutoType::AddEC, exist_info, autoscope);
    if nsc_prior < 0 {
        return false;
    }

    // --- 一番優先順位の高い候補をエンドカードとする ---
    if nsc_prior > 0 && prm_limit > 0 {
        // エンドカードを設定
        sub_add_revise_unit_cm(ds, cnv, arg, nsc_prior); // 合併１５秒単位CM処理
        ds.set_scp_arext(nsc_prior, AREXT_L_EC);
    }
    true
}

/// startAutoAddTR
pub fn start_auto_add_tr(
    ds: &mut Dataset,
    cnv: &Cnv,
    arg: &mut AutoArg,
    autoscope: (i32, i32),
) -> bool {
    let prm_c_exe = get_auto_param(arg, ParamAuto::C_exe);
    let prm_limit = get_auto_param(arg, ParamAuto::V_limit);
    let prm_c_limtrsum = get_auto_param(arg, ParamAuto::C_limtrsum);
    let prm_trsumprd = get_auto_param(arg, ParamAuto::V_trsumprd);

    if prm_c_exe == 0 {
        return false;
    }

    let mut exist_info = AddExistInfo::default();
    let cnt_limit = sub_add_get_limit(ds, cnv, arg, &mut exist_info, CmdAutoType::AddTR, autoscope);
    if prm_limit != 0 && cnt_limit >= prm_limit {
        return false;
    }
    if prm_c_limtrsum > 0 && exist_info.sec_tr >= prm_trsumprd {
        return false;
    }

    let nsc_prior = sub_add_search(ds, cnv, arg, CmdAutoType::AddTR, exist_info, autoscope);
    if nsc_prior < 0 {
        return false;
    }
    // --- 予告検出なしの検出 ---
    if nsc_prior <= 0 && prm_limit > 0 {
        return false;
    }

    // --- 一番優先順位の高い候補を予告とする ---
    if nsc_prior > 0 && prm_limit > 0 {
        // 予告を設定
        sub_add_revise_unit_cm(ds, cnv, arg, nsc_prior); // 合併１５秒単位CM処理
        ds.set_scp_arext(nsc_prior, AREXT_L_TRKEEP);
    }
    true
}

/// subAddGetLimit
fn sub_add_get_limit(
    ds: &Dataset,
    cnv: &Cnv,
    arg: &mut AutoArg,
    exist_info: &mut AddExistInfo,
    cmdtype: CmdAutoType,
    autoscope: (i32, i32),
) -> i32 {
    // --- コマンドに対応する構成取得 ---
    let arext_dst = if cmdtype == CmdAutoType::AddSP {
        AREXT_L_SP // 番組提供
    } else if cmdtype == CmdAutoType::AddEC {
        AREXT_L_EC // エンドカード
    } else {
        AREXT_L_TRKEEP
    };

    // --- limit確認、予告・番組提供有無確認 ---
    let mut cnt_limit = 0;
    let mut flag_trailer = false;
    let mut flag_sponsor = false;
    let mut totalsec_tr = 0;
    {
        let mut term = Term::default();
        let mut cont = true;
        while cont {
            cont = term_next_a(ds, arg, &mut term);
            if cont {
                let arext_term = ds.term_arext(&term);
                if ds.check_scope_term(&term, autoscope.0, autoscope.1) {
                    if arext_term == arext_dst {
                        // 目的存在
                        cnt_limit += 1;
                    }
                    if arext_term == AREXT_L_TRKEEP {
                        // 予告存在
                        flag_trailer = true;
                        // 予告秒数取得
                        let tmpsec = cnv
                            .tc
                            .get_sec_from_msec((term.msec_ed - term.msec_st) as i64)
                            as i32;
                        totalsec_tr += tmpsec;
                    }
                    if arext_term == AREXT_L_SP {
                        // 番組提供存在
                        flag_sponsor = true;
                    }
                }
            }
        }
    }

    // --- 予告検出なしの検出後は番組提供について合併15秒構成を分解して認識させる ---
    let mut divide_cm = false;
    {
        if ds.rec_hold.msec_tr_point == autoscope.0
            && autoscope.0 > 0
            && cmdtype == CmdAutoType::AddSP
        {
            divide_cm = true;
        }
        if !flag_trailer && cmdtype == CmdAutoType::AddTR {
            divide_cm = true;
        }
    }

    exist_info.trailer = flag_trailer;
    exist_info.sponsor = flag_sponsor;
    exist_info.sec_tr = totalsec_tr;
    exist_info.divide_cm = divide_cm;

    cnt_limit
}

/// subAddSearch
fn sub_add_search(
    ds: &mut Dataset,
    cnv: &Cnv,
    arg: &mut AutoArg,
    cmdtype: CmdAutoType,
    exist_info: AddExistInfo,
    autoscope: (i32, i32),
) -> i32 {
    // --- 使用パラメータ ---
    let prm_secprev = get_auto_param(arg, ParamAuto::V_secprev);
    let prm_secnext = get_auto_param(arg, ParamAuto::V_secnext);
    let prm_limit = get_auto_param(arg, ParamAuto::V_limit);
    let prm_c_limloc = get_auto_param(arg, ParamAuto::C_limloc);
    let prm_c_cutskip = get_auto_param(arg, ParamAuto::C_cutskip);
    let prm_c_wmin = get_auto_param(arg, ParamAuto::C_wmin);
    let prm_c_wmax = get_auto_param(arg, ParamAuto::C_wmax);
    let prm_c_w15 = get_auto_param(arg, ParamAuto::C_w15);
    let prm_c_wdefmin = get_auto_param(arg, ParamAuto::C_wdefmin);
    let prm_c_wdefmax = get_auto_param(arg, ParamAuto::C_wdefmax);

    // --- コマンドに対応する構成取得 ---
    let arext_dst = match cmdtype {
        CmdAutoType::AddSP => AREXT_L_SP,
        CmdAutoType::AddEC => AREXT_L_EC,
        _ => AREXT_L_TRKEEP,
    };

    // --- 各シーンチェンジ位置確認 ---
    let mut nsc_prior = -1;
    let mut nsc_sub_prior = -1;
    {
        let mut total_prior = 0;
        let mut total_sub_prior = 0;
        let mut term = Term::default();
        let mut cont = term_next_a(ds, arg, &mut term);
        while cont {
            let arstat_term = ds.term_arstat(&term);
            let arext_term = ds.term_arext(&term);
            // --- 範囲内の場合 ---
            if ds.check_scope_term(&term, autoscope.0, autoscope.1) {
                let mut locinfo = AddLocInfo::default();
                // --- 指定位置の前後状態を取得 ---
                sub_add_get_loc_info(ds, cnv, arg, &mut locinfo, term, autoscope);
                // --- 状態からの優先順位設定 ---
                let mut prior1 = sub_add_get_priority(
                    arg,
                    &mut locinfo,
                    cmdtype,
                    arstat_term,
                    arext_term,
                    exist_info,
                );
                // --- 次の構成時間に制約ある場合のチェック ---
                if prm_secnext > 0 && !sub_add_check_sec(ds, cnv, arg, term.nsc_ed, prm_secnext) {
                    prior1 = 0;
                }
                // --- 前の構成時間に制約ある場合のチェック ---
                if prm_secprev > 0 && !sub_add_check_sec(ds, cnv, arg, term.nsc_st, -prm_secprev) {
                    prior1 = 0;
                }
                // --- 候補限定用 ---
                let mut sub_prior1 = prior1;
                // --- 内容期間による選択 ---
                let secdif_sc = locinfo.sec_dif_sc;
                if (secdif_sc >= prm_c_wmin && secdif_sc <= prm_c_wmax)
                    || (secdif_sc == 15 && prm_c_w15 > 0)
                {
                    // 指定範囲内
                } else if secdif_sc >= prm_c_wdefmin && secdif_sc <= prm_c_wdefmax {
                    prior1 = 0; // 指定範囲外であるが標準範囲内
                } else {
                    // 期間が範囲外
                    prior1 = 0;
                    sub_prior1 = 0;
                }
                // --- 最優先位置の更新 ---
                if total_prior < prior1 {
                    total_prior = prior1;
                    nsc_prior = term.nsc_ed;
                }
                if total_sub_prior < sub_prior1 {
                    total_sub_prior = sub_prior1;
                    nsc_sub_prior = term.nsc_ed;
                }
                // --- limitない場合は毎回書き換え ---
                if prior1 > 0 && prm_limit == 0 {
                    sub_add_revise_unit_cm(ds, cnv, arg, term.nsc_ed); // 合併１５秒単位CM処理
                    ds.set_scp_arext(term.nsc_ed, arext_dst);
                }
            }
            cont = term_next_a(ds, arg, &mut term);
        }
    }
    // --- 標準期間の候補位置のみに候補を限定する処理 ---
    if prm_c_limloc > 0 && nsc_prior >= 0 && nsc_sub_prior != nsc_prior {
        nsc_prior = -1;
    }
    // --- 追加位置より前の予告等カットは無効化する処理 ---
    if nsc_prior > 0 && prm_c_cutskip > 0 {
        sub_add_cancel_cut(ds, cnv, arg, nsc_prior, autoscope);
    }
    nsc_prior
}

/// subAddGetLocInfo
fn sub_add_get_loc_info(
    ds: &Dataset,
    cnv: &Cnv,
    arg: &mut AutoArg,
    locinfo: &mut AddLocInfo,
    target: Term,
    autoscope: (i32, i32),
) {
    // --- 設定値 ---
    let prm_msec_wcomp_trmax = ds.get_config(ConfigVarType::msecWCompTRMax);

    // --- 使用パラメータ ---
    let prm_c_cutskip = get_auto_param(arg, ParamAuto::C_cutskip);
    let prm_c_lgprev = get_auto_param(arg, ParamAuto::C_lgprev);
    let prm_c_lgpost = get_auto_param(arg, ParamAuto::C_lgpost);

    // --- ロゴ位置を取得 ---
    let mut logomode = false; // ターゲット位置のロゴ状態
    let mut rnsc_logo = (-1, -1); // ターゲット位置前後のロゴ位置
    let mut nsc_endrise = -1; // 範囲内に全体が含まれる最後のロゴ開始位置
    {
        let mut elg = ElgCurrent::default();
        let mut cont = true;
        let mut want_logo = true;
        while cont {
            cont = elg_next_keep(ds, &mut elg);
            // --- 対象ロゴに一番近い立ち上がり／立ち下がり位置を取得 ---
            if want_logo {
                if elg.nsc_last_fall <= target.nsc_st && target.nsc_st < elg.nsc_rise {
                    want_logo = false;
                    logomode = false;
                    rnsc_logo = (elg.nsc_last_fall, elg.nsc_rise);
                    if elg.nsc_last_fall == 0 {
                        rnsc_logo.0 = -1;
                    }
                } else if elg.nsc_rise <= target.nsc_st && target.nsc_st < elg.nsc_fall {
                    want_logo = false;
                    logomode = true;
                    rnsc_logo = (elg.nsc_rise, elg.nsc_fall);
                } else if !cont {
                    want_logo = false;
                    logomode = false;
                    rnsc_logo = (elg.nsc_fall, -1);
                }
            }
            // --- 範囲内に全体が含まれる最後のロゴ位置を取得 ---
            if ds.check_scope_range(elg.msec_rise, elg.msec_fall, autoscope.0, autoscope.1) && cont
            {
                let mut term = Term {
                    ini: elg.nsc_rise,
                    ..Default::default()
                };
                let mut nextflag = term_next_a(ds, arg, &mut term);
                while nextflag {
                    if term.nsc_ed > elg.nsc_fall {
                        nextflag = false;
                    } else {
                        let arext_term = ds.term_arext(&term);
                        if arext_term != AREXT_L_LGCUT {
                            nextflag = false;
                            nsc_endrise = term.nsc_st; // 設定位置
                        }
                    }
                    if nextflag {
                        nextflag = term_next_a(ds, arg, &mut term);
                    }
                }
            }
        }
    }

    // --- 構成位置を取得 ---
    let mut rnsc_tr_1st = (-1, -1);
    let mut rnsc_tr_2nd = (-1, -1);
    let mut rnsc_tr_stop = (-1, -1);
    let mut rnsc_tr_near = (-1, -1);
    let mut rnsc_sp = (-1, -1);
    let mut rnsc_ec = (-1, -1);
    let mut type_nolast = 0;
    let mut flag_invalid_all = false;
    {
        let mut term = Term::default();
        let mut msec_difmin_tr = 0;
        let mut arext_hold = SCP_AREXT_NONE;
        let mut cont = term_next_a(ds, arg, &mut term);
        while cont {
            // --- 検索範囲内の処理 ---
            if ds.check_scope_term(&term, autoscope.0, autoscope.1) {
                let arstat_term = ds.term_arstat(&term);
                let mut arext_term = ds.term_arext(&term);
                // --- 対象位置との距離 ---
                let mut msec_dif_target = 0;
                if term.nsc_ed <= target.nsc_st {
                    msec_dif_target = (target.msec_st - term.msec_ed).abs();
                } else if term.nsc_st >= target.nsc_ed {
                    msec_dif_target = (term.msec_st - target.msec_ed).abs();
                }
                // --- 境界部分だった場合の補正 ---
                if arstat_term == SCP_AR_B_OTHER {
                    if arext_hold == AREXT_L_TRKEEP {
                        arext_term = arext_hold;
                    }
                } else {
                    arext_hold = arext_term;
                }
                // --- 予告部分 ---
                if arext_term == AREXT_L_TRKEEP {
                    if rnsc_tr_1st.0 < 0 {
                        // 一番最初
                        rnsc_tr_1st = (term.nsc_st, term.nsc_ed);
                    }
                    if msec_difmin_tr > msec_dif_target || rnsc_tr_near.0 < 0 {
                        // 一番近い
                        rnsc_tr_near = (term.nsc_st, term.nsc_ed);
                        msec_difmin_tr = msec_dif_target;
                    }
                    if rnsc_tr_stop.0 >= 0 && term.nsc_ed <= target.nsc_st {
                        // 予告終了位置変更
                        rnsc_tr_stop = (-1, -1);
                        rnsc_tr_2nd = (-1, -1);
                    }
                    if rnsc_tr_stop.0 >= 0 && rnsc_tr_2nd.0 < 0 {
                        // 一度予告終了後再開
                        rnsc_tr_2nd = (term.nsc_st, term.nsc_ed);
                    }
                } else if rnsc_tr_1st.0 >= 0 && rnsc_tr_stop.0 < 0 {
                    // 予告終了位置
                    rnsc_tr_stop = (term.nsc_st, term.nsc_ed);
                }
                // --- 番組提供部分 ---
                if arext_term == AREXT_L_SP && rnsc_sp.0 < 0 {
                    // 一番最初
                    rnsc_sp = (term.nsc_st, term.nsc_ed);
                }
                // --- エンドカード部分 ---
                if arext_term == AREXT_L_EC && rnsc_ec.0 < 0 {
                    // 一番最初
                    rnsc_ec = (term.nsc_st, term.nsc_ed);
                }
                // --- 予告・番組提供有効範囲チェック ---
                if term.msec_ed - term.msec_st > prm_msec_wcomp_trmax + ds.msec_val_spc
                    && term.nsc_st >= target.nsc_ed
                {
                    // 現在地点より後に予告を超える構成
                    if is_scp_ar_type_logo(arstat_term) {
                        // ロゴ内
                        if arext_term != AREXT_L_LGCUT {
                            // ロゴカット部分以外
                            type_nolast = 1; // 本編がまだ後に存在
                        }
                    }
                }
                // --- 予告・番組提供無効化チェック ---
                if (arext_term == AREXT_L_TRCUT || arext_term == AREXT_L_ECCUT)
                    && term.nsc_ed <= target.nsc_st
                {
                    // 予告カットが手前に存在
                    if prm_c_cutskip == 0 {
                        // カット以降は無効の時
                        flag_invalid_all = true;
                    }
                }
            }
            cont = term_next_a(ds, arg, &mut term);
        }
    }

    // --- ロゴ隣接状態検出 ---
    let mut sec_dif_logo = 0;
    let mut type_logo = 0; // ロゴ検出なし
    if !flag_invalid_all {
        let mut msec_dif_tmp1 = 600 * 1000; // 未検出時の最大フレーム数を設定
        let mut msec_dif_tmp2 = 600 * 1000; // 未検出時の最大フレーム数を設定
        if rnsc_logo.0 >= 0 {
            // ロゴ切り替わり前側からの距離
            // ロゴ開始側・終了側を候補から外す時の処理
            if (prm_c_lgprev == 0 && prm_c_lgpost != 0 && logomode)
                || (prm_c_lgprev != 0 && prm_c_lgpost == 0 && !logomode)
            {
                // skip
            } else {
                msec_dif_tmp1 = target.msec_st - ds.msec_scp(rnsc_logo.0);
            }
        }
        if rnsc_logo.1 >= 0 {
            // ロゴ切り替わり後側からの距離
            // ロゴ開始側・終了側を候補から外す時の処理
            if (prm_c_lgprev != 0 && prm_c_lgpost == 0 && logomode)
                || (prm_c_lgprev == 0 && prm_c_lgpost != 0 && !logomode)
            {
                // skip
            } else {
                msec_dif_tmp2 = ds.msec_scp(rnsc_logo.1) - target.msec_ed;
            }
        }
        let msec_dif_tmp;
        let mut type_logo_add = 0;
        let mut type_logo_pside = 0;
        if msec_dif_tmp1 <= msec_dif_tmp2 {
            // 前側が現位置に近い
            msec_dif_tmp = msec_dif_tmp1;
            if logomode {
                // 開始時ロゴあり
                type_logo_add = 100;
            }
        } else {
            // 後側が現位置に近い
            type_logo_pside = 1000;
            msec_dif_tmp = msec_dif_tmp2;
            if !logomode {
                // 開始時ロゴなし＝終了側がロゴあり
                type_logo_add = 100;
            }
        }
        sec_dif_logo = cnv.tc.get_sec_from_msec(msec_dif_tmp as i64) as i32; // 秒数に変換
        if logomode {
            // ロゴ内部
            if sec_dif_logo >= 300 {
                type_logo = 10; // ロゴ内部で５分以上離れている
            } else if sec_dif_logo >= 31 {
                type_logo = 12; // ロゴ内部で31秒以上離れている
            } else if sec_dif_logo >= 15 {
                type_logo = 13; // ロゴ内部で15秒以上離れている
            } else if sec_dif_logo > 1 {
                type_logo = 15; // ロゴ内部で近いが隣接ではない
            } else {
                type_logo = 16; // ロゴ内部で隣接
            }
        } else {
            // ロゴ外部
            if sec_dif_logo >= 300 {
                type_logo = 0; // ロゴ外部で５分以上離れている
            } else if sec_dif_logo >= 15 {
                type_logo = 3; // ロゴ外部で15秒以上離れている
            } else if sec_dif_logo > 1 {
                type_logo = 4; // ロゴ外部で近いが隣接ではない
            } else {
                type_logo = 7; // ロゴ外部で隣接
            }
        }
        type_logo += type_logo_add;
        type_logo += type_logo_pside;
    }

    // --- 予告からの時間 ---
    let sec_dif_tr;
    let type_tr;
    {
        if rnsc_tr_near.0 < 0 {
            // 予告が見つからない場合
            sec_dif_tr = 0;
            type_tr = 0;
        } else if rnsc_tr_1st.0 == target.nsc_ed {
            // 現位置が予告開始に隣接
            sec_dif_tr = 0;
            type_tr = 2;
        } else if rnsc_tr_1st.1 == target.nsc_ed {
            // 現位置が予告開始の最初の位置
            sec_dif_tr = 0;
            type_tr = 3;
        } else if rnsc_tr_2nd.0 >= 0 && rnsc_tr_stop.0 >= 0 && rnsc_tr_stop.1 == target.nsc_ed {
            // 現位置が予告隙間
            sec_dif_tr = 0;
            type_tr = 4;
        } else if rnsc_tr_near.0 >= target.nsc_ed {
            // 現位置が予告より前
            sec_dif_tr = cnv
                .tc
                .get_sec_from_msec((ds.msec_scp(rnsc_tr_near.0) - target.msec_ed) as i64)
                as i32;
            type_tr = 2;
        } else {
            // 現位置が予告後
            sec_dif_tr = cnv
                .tc
                .get_sec_from_msec((target.msec_st - ds.msec_scp(rnsc_tr_near.1)) as i64)
                as i32;
            type_tr = 1;
        }
    }
    // --- 番組提供からの時間 ---
    let sec_dif_sp;
    let type_sp;
    {
        if rnsc_sp.0 < 0 {
            sec_dif_sp = 0;
            type_sp = 0;
        } else if rnsc_sp.0 < target.nsc_ed && rnsc_sp.1 >= target.nsc_ed {
            // 現位置が番組提供と同一
            sec_dif_sp = 0;
            type_sp = 3;
        } else if rnsc_sp.0 >= target.nsc_ed {
            // 現位置が番組提供手前
            sec_dif_sp = cnv
                .tc
                .get_sec_from_msec((ds.msec_scp(rnsc_sp.0) - target.msec_ed) as i64)
                as i32;
            type_sp = 2;
        } else {
            // 現位置が番組提供後
            sec_dif_sp = cnv
                .tc
                .get_sec_from_msec((target.msec_st - ds.msec_scp(rnsc_sp.1)) as i64)
                as i32;
            type_sp = 1;
        }
    }
    // --- エンドカード手前の時間 ---
    let sec_dif_ec;
    let type_ec;
    {
        if rnsc_ec.0 < 0 {
            sec_dif_ec = 0;
            type_ec = 0;
        } else if rnsc_ec.0 >= target.nsc_ed {
            // 現位置がエンドカード手前
            sec_dif_ec = cnv
                .tc
                .get_sec_from_msec((ds.msec_scp(rnsc_ec.0) - target.msec_ed) as i64)
                as i32;
            type_ec = 2;
        } else {
            // 現位置がエンドカード後
            sec_dif_ec = 0; // 時間チェックはしていない
            type_ec = 1;
        }
    }
    // --- 単位間隔 ---
    let sec_dif_sc = cnv
        .tc
        .get_sec_from_msec((target.msec_ed - target.msec_st) as i64) as i32;

    // --- 最後のロゴからの状態 ---
    let type_endlogo;
    {
        if nsc_endrise < 0 {
            type_endlogo = 0;
        } else if target.msec_st < nsc_endrise {
            type_endlogo = 2;
        } else {
            type_endlogo = 1;
        }
    }

    // --- 返り値代入 ---
    locinfo.sec_dif_logo = sec_dif_logo;
    locinfo.type_logo = type_logo;
    locinfo.sec_dif_tr = sec_dif_tr;
    locinfo.type_tr = type_tr;
    locinfo.sec_dif_sp = sec_dif_sp;
    locinfo.type_sp = type_sp;
    locinfo.sec_dif_ec = sec_dif_ec;
    locinfo.type_ec = type_ec;
    locinfo.sec_dif_sc = sec_dif_sc;
    locinfo.type_nolast = type_nolast;
    locinfo.type_endlogo = type_endlogo;
}

/// subAddGetPriority
fn sub_add_get_priority(
    arg: &mut AutoArg,
    locinfo: &mut AddLocInfo,
    cmdtype: CmdAutoType,
    arstat_cur: i32,
    arext_cur: i32,
    exist_info: AddExistInfo,
) -> i32 {
    // --- 使用パラメータ ---
    let prm_c_lgprev = get_auto_param(arg, ParamAuto::C_lgprev);
    let prm_c_lgpost = get_auto_param(arg, ParamAuto::C_lgpost);
    let prm_c_lgintr = get_auto_param(arg, ParamAuto::C_lgintr);
    let prm_c_lgy = get_auto_param(arg, ParamAuto::C_lgy);
    let prm_c_lgn = get_auto_param(arg, ParamAuto::C_lgn);
    let prm_c_lgbn = get_auto_param(arg, ParamAuto::C_lgbn);
    let prm_c_search = get_auto_param(arg, ParamAuto::C_search);
    let prm_scope = get_auto_param(arg, ParamAuto::V_scope);
    let prm_scopen = get_auto_param(arg, ParamAuto::V_scopen);
    let prm_c_lgsp = get_auto_param(arg, ParamAuto::C_lgsp);
    let prm_c_in1 = get_auto_param(arg, ParamAuto::C_in1);
    let prm_c_chklast = get_auto_param(arg, ParamAuto::C_chklast);
    let prm_c_unitcmoff = get_auto_param(arg, ParamAuto::C_unitcmoff);
    let prm_c_unitcmon = get_auto_param(arg, ParamAuto::C_unitcmon);

    // --- type_logoを分類 ---
    let type_logo_prior = locinfo.type_logo % 10; // 優先順位
    let type_logo_in = (locinfo.type_logo / 10) % 10; // ロゴ内部
    let type_logo_prev = (locinfo.type_logo / 100) % 10; // ロゴ手前
    let type_logo_pside = (locinfo.type_logo / 1000) % 10; // ロゴ前側
                                                           // 両隣も含めロゴなし
    let type_logo_nonbr = if (type_logo_in == 0) && (type_logo_prior <= 5) {
        1
    } else {
        0
    };

    // --- 予告と番組提供の位置関係から無効箇所を検出 ---
    let mut invalid_pos = false;
    let mut invalid_sp = false;
    let mut invalid_tr = false;
    let type_tr = locinfo.type_tr;
    let type_sp = locinfo.type_sp;
    let type_ec = locinfo.type_ec;
    let secdif_tr = locinfo.sec_dif_tr;
    let secdif_sp = locinfo.sec_dif_sp;
    let secdif_ec = locinfo.sec_dif_ec;
    let secdif_logo = locinfo.sec_dif_logo;
    if exist_info.sponsor && exist_info.trailer {
        if prm_c_lgprev == 0 {
            // 前側を無効化
            if type_sp == 2 || type_tr == 2 {
                invalid_pos = true;
            }
        }
        if prm_c_lgpost == 0 {
            // 後側を無効化
            if type_sp == 1 || type_tr == 1 {
                invalid_pos = true;
            }
        }
        if prm_c_lgintr > 0 {
            // 中間を残す
            if type_tr == 1 && (type_sp == 2 || type_sp == 0)
                || type_tr == 2 && type_sp == 1
                || type_tr == 4
            {
                invalid_pos = false;
            }
        }
        if !invalid_pos {
            // 予告・番組提供ともに有効な場合
            if (type_sp == 1 && type_tr == 1) || (type_sp == 2 && type_tr == 2) {
                if secdif_sp > secdif_tr {
                    // 離れていたら近い側を残す
                    invalid_sp = true;
                } else if secdif_sp < secdif_tr {
                    // 離れていたら近い側を残す
                    invalid_tr = true;
                }
            }
        }
    } else if exist_info.sponsor {
        if prm_c_lgprev == 0 {
            // 前側を無効化
            if type_sp == 2 {
                invalid_pos = true;
            }
        }
        if prm_c_lgpost == 0 {
            // 後側を無効化
            if type_sp == 1 {
                invalid_pos = true;
            }
        }
    } else if exist_info.trailer {
        if prm_c_lgprev == 0 {
            // 前側を無効化
            if type_tr == 2 {
                invalid_pos = true;
            }
        }
        if prm_c_lgpost == 0 {
            // 後側を無効化
            if type_tr == 1 {
                invalid_pos = true;
            }
        }
    }
    // --- 検索範囲による無効設定 ---
    // ロゴなしのみ検索でロゴあり領域、またはその逆の場合は無効
    if (prm_c_lgy == 0 && type_logo_in != 0) || (prm_c_lgn == 0 && type_logo_in == 0) {
        invalid_pos = true;
    } else if prm_c_lgbn > 0 && type_logo_nonbr == 0 {
        // 両隣も含めてロゴなし検索で条件を満たさない場合は無効
        invalid_pos = true;
    }
    // --- 直後が番組提供・エンドカード時のみとする場合に条件を満たさない場合は無効 ---
    let is_next_sp = exist_info.sponsor && type_sp == 2 && secdif_sp == 0;
    let is_next_ec = type_ec == 2 && secdif_ec == 0;
    if prm_c_lgsp != 0 && !is_next_sp && !is_next_ec {
        invalid_pos = true;
    }

    // --- 検索範囲起点をロゴ検出にするか判断 ---
    let mut valid_onlylogo = false;
    if (4..=6).contains(&prm_c_search)
        || ((1..=3).contains(&prm_c_search)
            && !exist_info.trailer
            && (!exist_info.sponsor
                || cmdtype == CmdAutoType::AddTR
                || cmdtype == CmdAutoType::AddSP))
    {
        valid_onlylogo = true;
    }
    // --- 検索範囲による優先順位設定 ---
    let mut prior1 = 0;
    // --- ロゴからの距離判断 ---
    if !invalid_pos {
        // 前側検索なしで前側、後側検索なしで後側の場合は無効
        if (prm_c_lgprev == 0 && type_logo_prev != 0) || (prm_c_lgpost == 0 && type_logo_prev == 0)
        {
            prior1 = 0;
        } else if type_logo_prior > 0 {
            // ロゴ切り替わり位置から
            if (prm_c_search == 0)
                || ((prm_c_search == 1 || prm_c_search == 4) && (secdif_logo <= 1))
                || ((prm_c_search == 2 || prm_c_search == 5)
                    && (secdif_logo >= prm_scopen && secdif_logo <= prm_scope))
                || ((prm_c_search == 3 || prm_c_search == 6) && (secdif_logo == prm_scope))
            {
                if prm_c_search == 2 && type_logo_pside != 0 && secdif_logo != 0 {
                    // 手前側は隣接に限定する場合
                    prior1 = 0;
                } else {
                    prior1 = type_logo_prior;
                }
            }
        }
    }
    // --- 予告・番組提供・エンドカードからの距離判断 ---
    let mut valid_in1 = false; // 予告と認識した最初の内容を番組提供部分にする許可フラグ
    if !invalid_pos && !valid_onlylogo {
        // 予告・番組提供がある場合はロゴ端からの優先順位は（ロゴなしで隣接）以外無効にする
        let mut prior1bak = prior1;
        if prior1 > 0 && prior1 < 7 {
            prior1bak = 0;
        }
        // ロゴ優先順位は外し改めて優先順位を検索
        prior1 = 0;
        // 番組提供がある場合の処理
        if exist_info.sponsor {
            if type_sp == 0 {
                // no-op
            } else if invalid_pos {
                // 前側検索なしで前側、後側検索なしで後側の場合は無効
            } else if invalid_sp {
                // 番組提供は無効の場合
            } else if (prm_c_search == 0)
                || ((prm_c_search == 1) && (secdif_sp <= 1))
                || ((prm_c_search == 2) && (secdif_sp >= prm_scopen && secdif_sp <= prm_scope))
                || ((prm_c_search == 3) && (secdif_sp == prm_scope))
            {
                // 探索範囲内
                if type_sp == 2 {
                    // 番組提供より前
                    if secdif_sp <= 1 {
                        // 隣接のみ優先順位を上げる
                        prior1 += 20;
                    } else if prm_c_search == 2 {
                        // 手前側は隣接に限定する場合
                        prior1 = 0;
                    }
                } else if type_sp == 1 {
                    // 番組提供直後に隣接
                    if secdif_sp <= 1 {
                        prior1 += 130;
                    } else {
                        // 番組提供から指定時間以内
                        prior1 += 30;
                    }
                }
            }
        }
        if exist_info.trailer {
            if type_tr == 0 {
                // 予告が現地点に近くない場合は無効
            } else if invalid_pos {
                // 前側検索なしで前側、後側検索なしで後側の場合は無効
            } else if invalid_tr {
                // 予告は無効の場合
            } else if ((prm_c_search == 1) && (secdif_tr <= 1))
                || ((prm_c_search == 2) && (secdif_tr <= prm_scope))
                || ((prm_c_search == 3) && (secdif_tr == prm_scope))
            {
                // 探索範囲内
                if type_tr == 2 {
                    // 予告開始前
                    if secdif_tr == 0 {
                        // 隣接のみ優先順位を上げる
                        prior1 += 20;
                        // ロゴなしだった場合は優先度を上げる
                        if type_logo_in == 0 {
                            prior1 += 15;
                        }
                    } else if prm_c_search == 2 {
                        // 手前側は隣接に限定する場合
                        prior1 = 0;
                    }
                } else if type_tr == 1 {
                    // 予告直後に隣接
                    if secdif_tr <= 1 {
                        prior1 += 30;
                        // ロゴなしだった場合は優先度を上げる
                        if type_logo_in == 0 {
                            prior1 += 15;
                        }
                    } else {
                        // 予告から指定時間以内
                        prior1 += 20;
                    }
                } else if type_tr == 4 {
                    // 予告間隙間
                    prior1 += 40;
                } else if type_tr == 3 {
                    // 最初の予告位置
                    // type_tr == 3 は現状維持するが、番組提供に置換判断を許可
                    valid_in1 = true;
                }
            }
        }
        // 有効であれば残したロゴ優先順位も追加
        if prior1 > 0 {
            prior1 += prior1bak;
        }
    }
    // --- 本編内容前のロゴなし領域に予告・エンドカード位置がないか確認 ---
    if prm_c_chklast > 0 && locinfo.type_nolast > 0 && type_logo_in == 0 {
        prior1 = 0;
    }
    // --- 予告が見つからず、最後のロゴより前で前側は候補を外す場合の確認 ---
    if !exist_info.trailer && locinfo.type_endlogo == 2 && prm_c_lgprev == 0 {
        prior1 = 0;
    }
    // --- 現状態による選択 ---
    // --- 予告と認識した最初の内容を番組提供部分とする場合は特殊処理 ---
    if arext_cur == AREXT_L_TRKEEP && type_tr == 3 && prm_c_in1 > 0 && valid_in1 {
        if prm_c_in1 == 2 {
            // 優先度上げる
            prior1 += 500;
        }
        if prm_c_in1 == 1 {
            // 優先度上げる
            prior1 += 50;
        } else {
            // 優先度最小
            prior1 = 1;
        }
    } else if prior1 > 0 {
        if arext_cur == AREXT_L_TRKEEP
            || arext_cur == AREXT_L_EC
            || arext_cur == AREXT_L_SP
            || arext_cur == AREXT_L_LGCUT
            || arext_cur == AREXT_L_LGADD
            || arext_cur == AREXT_N_LGCUT
            || arext_cur == AREXT_N_LGADD
        {
            prior1 = 0;
        } else if arstat_cur == SCP_AR_N_AUNIT || arstat_cur == SCP_AR_N_BUNIT {
            // 合併１５秒単位CMの無効化処理
            if prm_c_unitcmoff > 0 {
                // 合併１５秒単位CMは強制的にCM以外とする
            } else if prm_c_unitcmon > 0 {
                // 合併１５秒単位CMは強制的にCMとする
                prior1 = 0;
            } else if type_tr != 1 {
                // 予告後の認識期間以外
                if !exist_info.divide_cm {
                    // 分割しない場合は無効でCMから変更しない
                    prior1 = 0;
                } else {
                    // 分割する場合は優先度最低で保持
                    prior1 = 1;
                }
            }
        } else if is_scp_ar_type_border(arstat_cur) && cmdtype == CmdAutoType::AddSP {
            // 番組提供のみ優先度アップ
            prior1 += 200;
        }
    }
    prior1
}

/// subAddCheckSec
fn sub_add_check_sec(
    ds: &Dataset,
    cnv: &Cnv,
    arg: &mut AutoArg,
    nsc_cur: i32,
    difsec: i32,
) -> bool {
    let mut ret = false;
    let next_dir = difsec >= 0;
    let mut term = Term {
        ini: nsc_cur,
        ..Default::default()
    };
    let mut cont = true;
    while cont {
        cont = if next_dir {
            term_next_a(ds, arg, &mut term)
        } else {
            term_next_a_prev(ds, arg, &mut term)
        };
        if cont {
            let sec_dif_term =
                cnv.tc
                    .get_sec_from_msec((term.msec_ed - term.msec_st) as i64) as i32;
            if sec_dif_term == difsec {
                ret = true;
            }
            if sec_dif_term >= difsec {
                cont = false;
            }
        }
    }
    ret
}

/// subAddCancelCut
fn sub_add_cancel_cut(
    ds: &mut Dataset,
    _cnv: &Cnv,
    arg: &mut AutoArg,
    nsc_prior: i32,
    autoscope: (i32, i32),
) {
    if nsc_prior > 0 {
        // --- 追加位置までの検索 ---
        let mut term = Term::default();
        let mut cont = term_next_a(ds, arg, &mut term);
        while cont && term.nsc_ed < nsc_prior {
            // --- 範囲内の場合 ---
            let arext_term = ds.term_arext(&term);
            if ds.check_scope_term(&term, autoscope.0, autoscope.1) {
                // --- カット処理を無効化 ---
                if arext_term == AREXT_L_TRCUT {
                    ds.set_term_arext(&term, AREXT_L_TRKEEP);
                } else if arext_term == AREXT_L_ECCUT {
                    ds.set_term_arext(&term, AREXT_L_TRRAW);
                }
            }
            cont = term_next_a(ds, arg, &mut term);
        }
    }
}

/// subAddReviseUnitCm
fn sub_add_revise_unit_cm(ds: &mut Dataset, cnv: &Cnv, arg: &mut AutoArg, nsc_target: i32) {
    let num_scpos = ds.size_scp();
    if nsc_target <= 0 || nsc_target >= num_scpos {
        return;
    }
    let arstat_target = ds.scp_arstat(nsc_target);
    let arstat_new = SCP_AR_N_OTHER;
    let mut arext_new = SCP_AREXT_NONE;

    // --- 合併１５秒単位CMだった場合の手前側処理 ---
    if (arstat_target == SCP_AR_N_AUNIT || arstat_target == SCP_AR_N_BUNIT) && nsc_target > 1 {
        // --- 手前側解除後の状態を設定 ---
        {
            // --- 手前が予告かチェック ---
            let mut term = Term {
                ini: nsc_target,
                ..Default::default()
            };
            let mut cont = term_next_a_prev(ds, arg, &mut term);
            let mut arstat_term = ds.term_arstat(&term);
            let mut flag_unitcont = true;
            // --- CM分割だった場合は分割の手前構成を取得 ---
            while cont
                && flag_unitcont
                && (arstat_term == SCP_AR_N_AUNIT || arstat_term == SCP_AR_N_BUNIT)
            {
                if arstat_term == SCP_AR_N_BUNIT {
                    // 分割先頭の次で終了
                    flag_unitcont = false;
                }
                cont = term_next_a_prev(ds, arg, &mut term);
                arstat_term = ds.term_arstat(&term);
            }
            if cont {
                let arext_term = ds.term_arext(&term);
                // --- 手前が予告の場合は予告状態を継続させる設定 ---
                if arext_term == AREXT_L_TRKEEP || arext_term == AREXT_L_TRRAW {
                    arext_new = AREXT_L_TRRAW; // 手前側解除後の状態を変更
                }
            }
        }
        // --- 解除処理 ---
        {
            let mut term = Term {
                ini: nsc_target,
                ..Default::default()
            };
            let mut cont = term_next_a_prev(ds, arg, &mut term);
            let mut arstat_term = ds.term_arstat(&term);
            // --- 中間位置を解除 ---
            while cont && arstat_term == SCP_AR_N_AUNIT {
                ds.set_term_arstat(&term, arstat_new);
                ds.set_term_arext(&term, arext_new);
                cont = term_next_a_prev(ds, arg, &mut term);
                arstat_term = ds.term_arstat(&term);
            }
            // --- 先頭位置を解除 ---
            if arstat_term == SCP_AR_N_BUNIT {
                ds.set_term_arstat(&term, arstat_new);
                ds.set_term_arext(&term, arext_new);
            }
        }
    }

    // 合併１５秒単位CMだった場合の後半部分処理
    if (arstat_target == SCP_AR_N_AUNIT || arstat_target == SCP_AR_N_BUNIT)
        && nsc_target < num_scpos - 1
    {
        let mut keep = 0;
        let mut msec_start = 0;
        let mut arext_keep = SCP_AREXT_NONE;
        let mut change = true;
        let mut term = Term {
            ini: nsc_target,
            ..Default::default()
        };
        let mut cont = term_next_a(ds, arg, &mut term);
        while cont && (0..=2).contains(&keep) {
            let arstat_term = ds.term_arstat(&term);
            let arext_term = ds.term_arext(&term);
            if arstat_term == SCP_AR_N_AUNIT
                || (arstat_term == SCP_AR_N_BUNIT && change)
                || (arstat_term == SCP_AR_N_OTHER && change)
            {
                keep += 1;
                if keep == 1 {
                    change = true; // 途中状態を設定
                    msec_start = term.msec_st; // 15秒単位開始位置
                    arext_keep = arext_term;
                } else if arext_term != arext_keep {
                    change = false; // 構成が変わったら分割設定しない
                }
                if keep > 1 && change {
                    // ２回目以降の場合
                    let sec_dif =
                        cnv.tc.get_sec_from_msec((term.msec_ed - msec_start) as i64) as i32;
                    // 新たに合併１５秒単位となる場合は設定
                    if (sec_dif % 15) == 0 && sec_dif <= 30 {
                        let mut nsc_lastwrite = -1;
                        let mut subterm = term;
                        let msec_prm_gap = ds.get_config(ConfigVarType::msecMgnCmDivide);
                        for _i in 0..keep - 1 {
                            let msec_sub_gapp = calc_dif_gap(subterm.msec_st, msec_start);
                            let msec_sub_gapn = calc_dif_gap(subterm.msec_st, term.msec_ed);
                            if msec_sub_gapp <= msec_prm_gap && msec_sub_gapn <= msec_prm_gap {
                                ds.set_term_arstat(&subterm, SCP_AR_N_AUNIT);
                                nsc_lastwrite = subterm.nsc_st;
                            } else {
                                ds.set_scp_chap(subterm.nsc_st, SCP_CHAP_NONE); // 構成結合
                                if nsc_lastwrite == subterm.nsc_ed {
                                    ds.set_term_arstat(&subterm, SCP_AR_N_AUNIT);
                                } else if nsc_lastwrite < 0 {
                                    ds.set_term_arstat(&subterm, SCP_AR_N_AUNIT);
                                    nsc_lastwrite = subterm.nsc_ed;
                                }
                            }
                            term_next_a_prev(ds, arg, &mut subterm);
                        }
                        if nsc_lastwrite == term.nsc_ed {
                            ds.set_scp_arstat(nsc_lastwrite, SCP_AR_N_UNIT);
                        } else if nsc_lastwrite > 0 {
                            ds.set_scp_arstat(nsc_lastwrite, SCP_AR_N_BUNIT);
                        }
                        keep = 0;
                        change = false;
                    } else if sec_dif > 30 {
                        change = false;
                    }
                }
                if keep > 0 {
                    ds.set_term_arstat(&term, SCP_AR_N_OTHER);
                }
            } else {
                cont = false;
            }
            // --- 次の位置設定 ---
            if cont {
                cont = term_next_a(ds, arg, &mut term);
            }
        }
    }
}

/// startAutoEdge
pub fn start_auto_edge(ds: &mut Dataset, cnv: &Cnv, arg: &mut AutoArg, lim: &CmdLimit) -> bool {
    let nsc_base = sub_ins_del_get_base(ds, lim);
    let mut flag_exec = false;
    if nsc_base >= 0 {
        let edge_base = lim.get_logo_base_edge();
        flag_exec = start_auto_edge_main(ds, cnv, arg, nsc_base, edge_base);
    }
    flag_exec
}

/// startAutoEdgeMain
fn start_auto_edge_main(
    ds: &mut Dataset,
    cnv: &Cnv,
    arg: &mut AutoArg,
    nsc_elg: i32,
    edge_elg: i32,
) -> bool {
    // --- 使用パラメータ ---
    let prm_c_exe = get_auto_param(arg, ParamAuto::C_exe);
    let prm_c_cmpart = get_auto_param(arg, ParamAuto::C_cmpart);

    // --- 実行有無確認 ---
    let mut flag_exec = false;
    if prm_c_exe == 0 {
        return false;
    }
    // --- ロゴエッジ種類を取得 ---
    if nsc_elg >= 0 && edge_elg == EDGE_RISE && prm_c_cmpart > 0 {
        // ロゴ開始前に追加
        flag_exec = sub_edge_exec(ds, cnv, arg, nsc_elg, EDGE_RISE, SEARCH_DIR_PREV);
    } else if nsc_elg >= 0 && edge_elg == EDGE_RISE && prm_c_cmpart == 0 {
        // ロゴ開始直後に追加
        flag_exec = sub_edge_exec(ds, cnv, arg, nsc_elg, EDGE_RISE, SEARCH_DIR_NEXT);
    } else if nsc_elg >= 0 && edge_elg == EDGE_FALL && prm_c_cmpart == 0 {
        // ロゴ終了直前に追加
        flag_exec = sub_edge_exec(ds, cnv, arg, nsc_elg, EDGE_FALL, SEARCH_DIR_PREV);
    } else if nsc_elg >= 0 && edge_elg == EDGE_FALL && prm_c_cmpart > 0 {
        // ロゴ終了直後に追加
        flag_exec = sub_edge_exec(ds, cnv, arg, nsc_elg, EDGE_FALL, SEARCH_DIR_NEXT);
    }
    flag_exec
}

/// subEdgeExec
fn sub_edge_exec(
    ds: &mut Dataset,
    cnv: &Cnv,
    arg: &mut AutoArg,
    nsc_logo: i32,
    edge_logo: i32,
    dr: i32,
) -> bool {
    // --- 使用パラメータ ---
    let prm_c_search = get_auto_param(arg, ParamAuto::C_search);
    let prm_scope = get_auto_param(arg, ParamAuto::V_scope);
    let prm_c_wmin = get_auto_param(arg, ParamAuto::C_wmin);
    let prm_c_wmax = get_auto_param(arg, ParamAuto::C_wmax);
    let prm_c_add = get_auto_param(arg, ParamAuto::C_add);
    let prm_c_allcom = get_auto_param(arg, ParamAuto::C_allcom);
    let mut flag_adapt = !(prm_c_search == 2 || prm_c_search == 3); // 既存カット適用後からの時1
    let flag_ovw = prm_c_search == 3;

    // --- 初期化 ---
    let mut flag_exec = false;
    // --- ロゴ内の変更か判断 ---
    let mut inlogo = false;
    if (edge_logo == EDGE_RISE && dr == SEARCH_DIR_NEXT)
        || (edge_logo == EDGE_FALL && dr == SEARCH_DIR_PREV)
    {
        inlogo = true;
    }
    // --- ロゴ終了位置を取得 ---
    let mut nsc_endlogo;
    if dr == SEARCH_DIR_PREV {
        nsc_endlogo = ds.nsc_prev_elg(nsc_logo, EDGE_BOTH);
        if nsc_endlogo < 0 {
            nsc_endlogo = 0; // 前がなければ先頭位置
        }
    } else {
        nsc_endlogo = ds.nsc_next_elg(nsc_logo, EDGE_BOTH);
        if nsc_endlogo < 0 {
            nsc_endlogo = ds.size_scp() - 1; // 次がなければ最終位置
        }
    }
    // --- ロゴ位置から順番に確認 ---
    {
        let mut nsc_base = nsc_logo;
        let msec_logo = ds.msec_scp(nsc_logo);
        let mut msec_base = ds.msec_scp(nsc_base);
        let mut term = Term {
            ini: nsc_logo,
            ..Default::default()
        };
        let mut cont = true;
        while cont {
            let mut nsc_dst;
            if dr == SEARCH_DIR_PREV {
                // 前側に検索時
                cont = term_next_a_prev(ds, arg, &mut term);
                nsc_dst = term.nsc_st;
                if nsc_dst < nsc_endlogo {
                    cont = false;
                }
            } else {
                // 後側に検索時
                cont = term_next_a(ds, arg, &mut term);
                nsc_dst = term.nsc_ed;
                if nsc_dst > nsc_endlogo {
                    cont = false;
                }
            }
            if cont {
                nsc_dst = if dr == SEARCH_DIR_PREV {
                    term.nsc_st
                } else {
                    term.nsc_ed
                };
                let msec_dst = ds.msec_scp(nsc_dst);
                let sec_dif_base = cnv
                    .tc
                    .get_sec_from_msec((msec_dst - msec_base).abs() as i64)
                    as i32;
                let sec_dif_logo = cnv
                    .tc
                    .get_sec_from_msec((msec_dst - msec_logo).abs() as i64)
                    as i32;
                let arext_term = ds.term_arext(&term);
                // --- 既存のカット（残留）状態を取得 ---
                let mut select_arext = false;
                if arext_term == AREXT_N_LGCUT
                    || arext_term == AREXT_N_LGADD
                    || arext_term == AREXT_L_LGCUT
                    || arext_term == AREXT_L_LGADD
                {
                    select_arext = true; // カット（残留）が既に存在
                }
                // --- 既存のカット（残留）でシフト許可状態の場合は基準位置を変更 ---
                if flag_adapt && select_arext {
                    nsc_base = nsc_dst; // 既存カット適用後の位置に基準を移動
                    msec_base = msec_dst;
                } else {
                    if flag_adapt {
                        flag_adapt = false; // 基準位置確定
                    }
                    // --- カット可能な最大秒数チェック ---
                    if sec_dif_logo > prm_scope || sec_dif_base > prm_c_wmax {
                        cont = false;
                    } else if select_arext && !flag_ovw {
                        // --- 既存カットが存在で書き換えしない ---
                        cont = false;
                    } else if sec_dif_base >= prm_c_wmin && sec_dif_base <= prm_c_wmax {
                        // --- カット適用範囲内の処理 ---
                        flag_exec = true;
                        if prm_c_allcom == 0 {
                            // 見つかったら終了する場合
                            cont = false;
                        }
                        let arext_add = if prm_c_add == 0 {
                            if inlogo {
                                AREXT_L_LGCUT
                            } else {
                                AREXT_N_LGCUT
                            }
                        } else if inlogo {
                            AREXT_L_LGADD
                        } else {
                            AREXT_N_LGADD
                        };
                        // --- 現在位置を設定し、複数構成だった場合は全構成で設定 ---
                        ds.set_term_arext(&term, arext_add); // 書き換え実行
                        let mut subterm = term;
                        let mut subcont = true;
                        while subcont {
                            if dr == SEARCH_DIR_PREV {
                                // 前側に検索時
                                subcont = term_next_a(ds, arg, &mut subterm);
                                if subterm.nsc_ed > nsc_base {
                                    subcont = false;
                                }
                            } else {
                                // 後側に検索時
                                subcont = term_next_a_prev(ds, arg, &mut subterm);
                                if subterm.nsc_st < nsc_base {
                                    subcont = false;
                                }
                            }
                            if subcont {
                                ds.set_term_arext(&subterm, arext_add); // 書き換え実行
                            }
                        }
                    }
                }
            }
        }
    }
    flag_exec
}

/// checkFirstAct
pub fn check_first_act(ds: &mut Dataset, cnv: &Cnv, cmdarg: &CmdArg) {
    // --- 初回のみ実行 ---
    if ds.is_auto_mode_initial() {
        // --- 推測構成を作成 ---
        ds.set_flag_auto_mode(2); // Auto系を有効
        super::reform_all::mk_reform_all(ds, cnv, cmdarg); // 基本構成推測
    }
}

/// exeCmdMain
pub fn exe_cmd_main(ds: &mut Dataset, cnv: &Cnv, cmdarg: &mut CmdArg, lim: &CmdLimit) -> bool {
    // --- パラメータとコマンド種類を取得 ---
    let mut a = AutoArg::new();
    let cmdtype = exe_cmd_param(cmdarg, &mut a);

    // --- 範囲設定 ---
    let mut autoscope = lim.get_frame_range();
    if autoscope.0 < 0 {
        autoscope.0 = lim.get_head();
    }
    if autoscope.1 < 0 {
        autoscope.1 = lim.get_tail();
    }

    let mut exeflag = false;
    match cmdtype {
        CmdAutoType::CutTR => exeflag = start_auto_cut_tr(ds, cnv, &mut a, autoscope),
        CmdAutoType::CutEC => exeflag = start_auto_cut_ec(ds, cnv, &mut a, autoscope),
        CmdAutoType::AddTR => exeflag = start_auto_add_tr(ds, cnv, &mut a, autoscope),
        CmdAutoType::AddSP => exeflag = start_auto_add_sp(ds, cnv, &mut a, autoscope),
        CmdAutoType::AddEC => exeflag = start_auto_add_ec(ds, cnv, &mut a, autoscope),
        CmdAutoType::Edge => exeflag = start_auto_edge(ds, cnv, &mut a, lim),
        CmdAutoType::AtCM => exeflag = super::reform_all::start_auto_cm(ds, cnv, cmdarg),
        CmdAutoType::AtUP => exeflag = start_auto_up(ds, cnv, &mut a),
        CmdAutoType::AtBorder => exeflag = start_auto_border(ds, cnv, &mut a, autoscope),
        CmdAutoType::AtIClear => exeflag = start_auto_iclear(ds, cnv, &mut a, autoscope),
        CmdAutoType::Ins => exeflag = start_auto_ins(ds, cnv, &mut a, lim),
        CmdAutoType::Del => exeflag = start_auto_del(ds, cnv, &mut a, lim),
        CmdAutoType::AtChg => exeflag = start_auto_chg(ds, cnv, &mut a, lim),
        _ => {
            eprintln!("error:internal setting (AutoType: {})", cmdtype as i32);
        }
    }
    exeflag
}

/// startCmd
pub fn start_cmd(
    ds: &mut Dataset,
    cnv: &Cnv,
    cmdarg: &mut CmdArg,
    lim: &CmdLimit,
    setup_only: bool,
) -> bool {
    check_first_act(ds, cnv, cmdarg);
    if !setup_only {
        return exe_cmd_main(ds, cnv, cmdarg, lim);
    }
    true
}
