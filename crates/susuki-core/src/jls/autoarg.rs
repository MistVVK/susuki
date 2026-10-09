//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Port of JlsAutoArg + ParamAuto/CmdAutoType (JlsAutoScript.cpp:19-373,
//! JlsNameSpace.hpp CmdAutoType/ParamAuto).

use super::cmdarg::CmdArg;
use super::tables::OptType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum CmdAutoType {
    None = 0,
    CutTR = 1,
    CutEC = 2,
    AddTR = 3,
    AddSP = 4,
    AddEC = 5,
    Edge = 6,
    AtCM = 7,
    AtUP = 8,
    AtBorder = 9,
    AtIClear = 10,
    AtChg = 11,
    Ins = 12,
    Del = 13,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
// 原典 JlsNameSpace.hpp ParamAuto の識別子表（照合容易性のため命名を一致させる）
#[allow(non_camel_case_types)]
pub enum ParamAuto {
    // code parameters (order = JlsNameSpace.hpp ParamAuto)
    C_exe = 0,
    C_search = 1,
    C_wmin = 2,
    C_wmax = 3,
    C_w15 = 4,
    C_lgprev = 5,
    C_lgpost = 6,
    C_lgintr = 7,
    C_lgsp = 8,
    C_cutskip = 9,
    C_in1 = 10,
    C_chklast = 11,
    C_lgy = 12,
    C_lgn = 13,
    C_lgbn = 14,
    C_limloc = 15,
    C_limtrsum = 16,
    C_unitcmoff = 17,
    C_unitcmon = 18,
    C_wdefmin = 19,
    C_wdefmax = 20,
    // autocut
    C_from = 21,
    C_cutst = 22,
    C_lgpre = 23,
    C_sel = 24,
    C_cutla = 25,
    C_cutlp = 26,
    C_cut30 = 27,
    C_cutsp = 28,
    // edge
    C_cmpart = 29,
    C_add = 30,
    C_allcom = 31,
    C_noedge = 32,
    // autoins/autodel
    C_restruct = 33,
    C_unit = 34,
    // numeric
    V_limit = 35,
    V_scope = 36,
    V_scopen = 37,
    V_period = 38,
    V_maxprd = 39,
    V_trsumprd = 40,
    V_secprev = 41,
    V_secnext = 42,
    V_trscope = 43,
    V_tr1stprd = 44,
    V_info = 45,
    MAXSIZE = 46,
}

pub struct AutoArg {
    cmdtype: CmdAutoType,
    enable_prm: [i32; ParamAuto::MAXSIZE as usize],
    val_prm: [i32; ParamAuto::MAXSIZE as usize],
    /// reference writes to cerr on unset access; buffered for parity checks
    pub err_buf: Vec<u8>,
}

impl AutoArg {
    pub fn new() -> Self {
        AutoArg {
            cmdtype: CmdAutoType::None,
            enable_prm: [0; ParamAuto::MAXSIZE as usize],
            val_prm: [0; ParamAuto::MAXSIZE as usize],
            err_buf: Vec::new(),
        }
    }

    /// setParam
    pub fn set_param(&mut self, cmdarg: &CmdArg, cmdtype: CmdAutoType) {
        self.clear_all();
        self.cmdtype = cmdtype;
        match cmdtype {
            CmdAutoType::CutTR => self.set_param_cut_tr(cmdarg),
            CmdAutoType::CutEC => self.set_param_cut_ec(cmdarg),
            CmdAutoType::AddTR | CmdAutoType::AddSP | CmdAutoType::AddEC => {
                self.set_param_add(cmdarg)
            }
            CmdAutoType::Edge => self.set_param_edge(cmdarg),
            CmdAutoType::AtUP
            | CmdAutoType::AtBorder
            | CmdAutoType::AtIClear
            | CmdAutoType::Ins
            | CmdAutoType::Del => self.set_param_ins_del(cmdarg),
            _ => {}
        }
    }

    /// getParam
    pub fn get_param(&mut self, tp: ParamAuto) -> i32 {
        let num = tp as usize;
        if num < ParamAuto::MAXSIZE as usize && self.enable_prm[num] > 0 {
            return self.val_prm[num];
        }
        eprintln!(
            "unexpected error JlsAutoArg::GetParam cmdtype={} type={}",
            self.cmdtype as i32, num
        );
        0
    }

    fn clear_all(&mut self) {
        for i in 0..ParamAuto::MAXSIZE as usize {
            self.enable_prm[i] = 0;
            self.val_prm[i] = 0;
        }
    }

    fn set_val(&mut self, tp: ParamAuto, val: i32) {
        let num = tp as usize;
        if num < ParamAuto::MAXSIZE as usize {
            self.enable_prm[num] = 1;
            self.val_prm[num] = val;
        }
    }

    fn set_param_cut_tr(&mut self, cmdarg: &CmdArg) {
        let autop_code = cmdarg.get_opt(OptType::AutopCode);
        let tmp_trscope = cmdarg.get_opt(OptType::AutopTrScope);
        let tmp_trsumprd = cmdarg.get_opt(OptType::AutopTrSumPrd);
        let prm_v_limit = cmdarg.get_opt(OptType::AutopLimit);
        let prm_v_tr1stprd = cmdarg.get_opt(OptType::AutopTr1stPrd);
        let prm_c_from = autop_code % 10;
        let prm_c_cutst = (autop_code / 10) % 10;
        let prm_c_lgpre = (autop_code / 100) % 10;
        let prm_c_noedge = 1;
        let prm_c_exe = if prm_c_from != 0 { 1 } else { 0 };
        let prm_v_trscope = if tmp_trscope == 0 { 30 } else { tmp_trscope };
        let prm_v_trsumprd = if tmp_trsumprd == 0 { 3 } else { tmp_trsumprd };

        self.set_val(ParamAuto::V_limit, prm_v_limit);
        self.set_val(ParamAuto::V_tr1stprd, prm_v_tr1stprd);
        self.set_val(ParamAuto::C_from, prm_c_from);
        self.set_val(ParamAuto::C_cutst, prm_c_cutst);
        self.set_val(ParamAuto::C_lgpre, prm_c_lgpre);
        self.set_val(ParamAuto::C_exe, prm_c_exe);
        self.set_val(ParamAuto::V_trscope, prm_v_trscope);
        self.set_val(ParamAuto::V_trsumprd, prm_v_trsumprd);
        self.set_val(ParamAuto::C_noedge, prm_c_noedge);
    }

    fn set_param_cut_ec(&mut self, cmdarg: &CmdArg) {
        let autop_code = cmdarg.get_opt(OptType::AutopCode);
        let tmp_period = cmdarg.get_opt(OptType::AutopPeriod);
        let tmp_maxprd = cmdarg.get_opt(OptType::AutopMaxPrd);
        let prm_v_limit = cmdarg.get_opt(OptType::AutopLimit);
        let prm_c_sel = cmdarg.get_opt(OptType::AutopCode) % 10;
        let prm_c_cutla = (((autop_code / 10) % 10) & 0x1) != 0;
        let prm_c_cutlp = (((autop_code / 10) % 10) & 0x2) != 0;
        let prm_c_cut30 = 0;
        let prm_c_cutsp = (((autop_code / 100) % 10) & 0x1) != 0;
        let prm_c_noedge = 1;
        let prm_v_period = if tmp_period == 0 { 5 } else { tmp_period };
        let prm_v_maxprd = if tmp_maxprd == 0 { 13 } else { tmp_maxprd };

        self.set_val(ParamAuto::V_limit, prm_v_limit);
        self.set_val(ParamAuto::C_sel, prm_c_sel);
        self.set_val(ParamAuto::C_cutla, prm_c_cutla as i32);
        self.set_val(ParamAuto::C_cutlp, prm_c_cutlp as i32);
        self.set_val(ParamAuto::C_cut30, prm_c_cut30);
        self.set_val(ParamAuto::C_cutsp, prm_c_cutsp as i32);
        self.set_val(ParamAuto::V_period, prm_v_period);
        self.set_val(ParamAuto::V_maxprd, prm_v_maxprd);
        self.set_val(ParamAuto::C_noedge, prm_c_noedge);
    }

    fn set_param_add(&mut self, cmdarg: &CmdArg) {
        let (
            default_c_wmin,
            default_c_wmax,
            default_scope,
            default_period,
            default_search,
            enable_w15,
            enable_in1,
        ) = match self.cmdtype {
            CmdAutoType::AddSP => (6, 13, 90, 5, 1, 1, 1),
            CmdAutoType::AddEC => (1, 13, 90, 5, 1, 0, 0),
            _ => (1, 13, 90, 5, 1, 0, 0),
        };

        let autop_code = cmdarg.get_opt(OptType::AutopCode);
        let tmp_c_w = autop_code % 10;
        let tmp_c_sea = (autop_code / 10) % 10;
        let tmp_c_lg = (autop_code / 100) % 10;
        let tmp_c_p = (autop_code / 1000) % 10;
        let tmp_c_lim = (autop_code / 10000) % 10;
        let tmp_c_unit = (autop_code / 100000) % 10;
        let tmp_period = cmdarg.get_opt(OptType::AutopPeriod);
        let tmp_maxprd = cmdarg.get_opt(OptType::AutopMaxPrd);
        let tmp_scope = cmdarg.get_opt(OptType::AutopScope);
        let prm_v_scopen = cmdarg.get_opt(OptType::AutopScopeN);
        let prm_v_limit = cmdarg.get_opt(OptType::AutopLimit);
        let prm_v_secnext = cmdarg.get_opt(OptType::AutopSecNext);
        let prm_v_secprev = cmdarg.get_opt(OptType::AutopSecPrev);
        let prm_v_trsumprd = cmdarg.get_opt(OptType::AutopTrSumPrd);
        let prm_c_noedge = 1;
        let is_scope = cmdarg.is_set_opt(OptType::AutopScope);
        let is_period = cmdarg.is_set_opt(OptType::AutopPeriod);
        let is_maxprd = cmdarg.is_set_opt(OptType::AutopMaxPrd);

        let prm_c_exe = if tmp_c_w != 0 { 1 } else { 0 };
        let prm_v_scope = if !is_scope { default_scope } else { tmp_scope };
        let prm_v_period = if !is_period {
            default_period
        } else {
            tmp_period
        };
        let prm_v_maxprd = if !is_maxprd {
            default_c_wmax
        } else {
            tmp_maxprd
        };
        let prm_c_search = if tmp_c_sea == 0 {
            default_search
        } else {
            tmp_c_sea
        };
        let prm_c_lgy = i32::from((tmp_c_lg & 0x3) != 1);
        let prm_c_lgn = i32::from((tmp_c_lg & 0x3) != 2);
        let prm_c_lgbn = i32::from((tmp_c_lg & 0x3) == 3);
        let prm_c_cutskip = i32::from((tmp_c_lg & 0x4) != 0);
        let prm_c_lgprev =
            i32::from(!(tmp_c_p == 1 || tmp_c_p == 3 || (enable_in1 > 0 && tmp_c_p >= 4)));
        let prm_c_lgpost =
            i32::from(!(tmp_c_p == 2 || tmp_c_p == 3 || (enable_in1 > 0 && tmp_c_p >= 4)));
        let prm_c_lgintr = i32::from(tmp_c_p == 3);
        let prm_c_lgsp = i32::from(tmp_c_p == 4 && enable_in1 == 0);
        let prm_c_limloc = i32::from((tmp_c_lim & 0x1) > 0);
        let prm_c_limtrsum = i32::from((tmp_c_lim & 0x2) > 0);
        let prm_c_unitcmoff = i32::from((tmp_c_unit & 0x1) > 0);
        let prm_c_unitcmon = i32::from((tmp_c_unit & 0x2) > 0);
        let prm_c_wdefmin = default_c_wmin;
        let prm_c_wdefmax = default_c_wmax;

        self.set_val(ParamAuto::V_scopen, prm_v_scopen);
        self.set_val(ParamAuto::V_limit, prm_v_limit);
        self.set_val(ParamAuto::V_secnext, prm_v_secnext);
        self.set_val(ParamAuto::V_secprev, prm_v_secprev);
        self.set_val(ParamAuto::V_trsumprd, prm_v_trsumprd);
        self.set_val(ParamAuto::C_exe, prm_c_exe);
        self.set_val(ParamAuto::V_scope, prm_v_scope);
        self.set_val(ParamAuto::V_period, prm_v_period);
        self.set_val(ParamAuto::V_maxprd, prm_v_maxprd);
        self.set_val(ParamAuto::C_search, prm_c_search);
        self.set_val(ParamAuto::C_lgy, prm_c_lgy);
        self.set_val(ParamAuto::C_lgn, prm_c_lgn);
        self.set_val(ParamAuto::C_lgbn, prm_c_lgbn);
        self.set_val(ParamAuto::C_cutskip, prm_c_cutskip);
        self.set_val(ParamAuto::C_lgprev, prm_c_lgprev);
        self.set_val(ParamAuto::C_lgpost, prm_c_lgpost);
        self.set_val(ParamAuto::C_lgintr, prm_c_lgintr);
        self.set_val(ParamAuto::C_lgsp, prm_c_lgsp);
        self.set_val(ParamAuto::C_limloc, prm_c_limloc);
        self.set_val(ParamAuto::C_limtrsum, prm_c_limtrsum);
        self.set_val(ParamAuto::C_unitcmoff, prm_c_unitcmoff);
        self.set_val(ParamAuto::C_unitcmon, prm_c_unitcmon);
        self.set_val(ParamAuto::C_wdefmin, prm_c_wdefmin);
        self.set_val(ParamAuto::C_wdefmax, prm_c_wdefmax);
        self.set_val(ParamAuto::C_noedge, prm_c_noedge);

        // 15秒特殊設定
        let mut prm_c_w15 = 0;
        if enable_w15 > 0 {
            prm_c_w15 = i32::from(tmp_c_w == 6 || (tmp_c_w == 3 && prm_v_period % 15 == 0));
        }
        let mut prm_c_in1 = 0;
        let mut prm_c_chklast = 1;
        if enable_in1 > 0 {
            prm_c_in1 = if tmp_c_p >= 4 { tmp_c_p - 3 } else { 0 };
            prm_c_chklast = 0;
        }
        let mut prm_c_wmin = default_c_wmin;
        if tmp_c_w == 2 || tmp_c_w == 4 || tmp_c_w == 5 {
            prm_c_wmin = prm_v_period;
        } else if tmp_c_w == 6 && enable_w15 == 1 {
            prm_c_wmin = 15;
        }
        let mut prm_c_wmax = default_c_wmax;
        if tmp_c_w == 2 || tmp_c_w == 3 {
            prm_c_wmax = prm_v_period;
        } else if tmp_c_w == 5 {
            prm_c_wmax = prm_v_maxprd;
        } else if tmp_c_w == 6 && enable_w15 == 1 {
            prm_c_wmax = 15;
        }

        self.set_val(ParamAuto::C_w15, prm_c_w15);
        self.set_val(ParamAuto::C_in1, prm_c_in1);
        self.set_val(ParamAuto::C_chklast, prm_c_chklast);
        self.set_val(ParamAuto::C_wmin, prm_c_wmin);
        self.set_val(ParamAuto::C_wmax, prm_c_wmax);
    }

    fn set_param_edge(&mut self, cmdarg: &CmdArg) {
        let autop_code = cmdarg.get_opt(OptType::AutopCode);
        let tmp_scope = cmdarg.get_opt(OptType::AutopScope);
        let tmp_period = cmdarg.get_opt(OptType::AutopPeriod);
        let tmp_maxprd = cmdarg.get_opt(OptType::AutopMaxPrd);
        let tmp_c_w = autop_code % 10;
        let tmp_c_sea = (autop_code / 10) % 10;
        let prm_c_cmpart = i32::from((((autop_code / 100) % 10) & 0x1) == 1);
        let prm_c_add = i32::from((((autop_code / 100) % 10) & 0x2) != 0);
        let prm_c_allcom = i32::from((((autop_code / 1000) % 10) & 0x1) == 1);
        let prm_c_noedge = i32::from(!((((autop_code / 1000) % 10) & 0x2) != 0));
        let prm_c_exe = if tmp_c_w != 0 { 1 } else { 0 };
        let prm_v_scope = if tmp_scope == 0 { 90 } else { tmp_scope };
        let prm_v_period = if tmp_period == 0 && !cmdarg.is_set_opt(OptType::AutopPeriod) {
            5
        } else {
            tmp_period
        };
        let prm_v_maxprd = if tmp_maxprd == 0 { 10 } else { tmp_maxprd };
        let prm_c_search = if tmp_c_sea == 0 { 1 } else { tmp_c_sea };
        let mut prm_c_wmin = 3;
        if tmp_c_w == 2 || tmp_c_w == 4 || tmp_c_w == 5 {
            prm_c_wmin = prm_v_period;
        }
        let mut prm_c_wmax = 10;
        if tmp_c_w == 2 || tmp_c_w == 3 {
            prm_c_wmax = prm_v_period;
        } else if tmp_c_w == 5 {
            prm_c_wmax = prm_v_maxprd;
        }

        self.set_val(ParamAuto::C_cmpart, prm_c_cmpart);
        self.set_val(ParamAuto::C_add, prm_c_add);
        self.set_val(ParamAuto::C_allcom, prm_c_allcom);
        self.set_val(ParamAuto::C_noedge, prm_c_noedge);
        self.set_val(ParamAuto::C_exe, prm_c_exe);
        self.set_val(ParamAuto::V_scope, prm_v_scope);
        self.set_val(ParamAuto::V_period, prm_v_period);
        self.set_val(ParamAuto::V_maxprd, prm_v_maxprd);
        self.set_val(ParamAuto::C_search, prm_c_search);
        self.set_val(ParamAuto::C_wmin, prm_c_wmin);
        self.set_val(ParamAuto::C_wmax, prm_c_wmax);
    }

    fn set_param_ins_del(&mut self, cmdarg: &CmdArg) {
        let mut autop_code = cmdarg.get_opt(OptType::AutopCode);
        if !cmdarg.is_set_opt(OptType::AutopCode) {
            autop_code = 1;
        }
        let prm_c_unit = cmdarg.get_opt(OptType::FlagUnit);
        let prm_v_info = cmdarg.get_opt(OptType::AutopTrInfo);
        let tmp_c_w = autop_code % 10;
        let prm_c_exe = if tmp_c_w != 0 { 1 } else { 0 };
        let prm_c_restruct = if tmp_c_w == 2 { 1 } else { 0 };
        let prm_c_noedge = 1;

        self.set_val(ParamAuto::C_exe, prm_c_exe);
        self.set_val(ParamAuto::C_restruct, prm_c_restruct);
        self.set_val(ParamAuto::C_noedge, prm_c_noedge);
        self.set_val(ParamAuto::C_unit, prm_c_unit);
        self.set_val(ParamAuto::V_info, prm_v_info);
    }
}

impl Default for AutoArg {
    fn default() -> Self {
        Self::new()
    }
}
