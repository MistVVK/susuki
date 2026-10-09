//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Port of REF-JLS `JlsDataset` storage layer (fixed commit d1a24d7):
//! config, scp/logo record storage, nrf mapping accessors, backup.
//! Search/term/output algorithms are separate modules coming with the
//! executor stage. Enum discriminants follow JlsNameSpace.hpp.

use super::cmdarg::WideMsec;
use super::tables::ConfigVarType;

pub const SIZE_CONFIG_VAR: usize = (ConfigVarType::MAXSIZE as u32) as usize;

// ScpChapType
pub const SCP_CHAP_DUPE: i32 = -1;
pub const SCP_CHAP_NONE: i32 = 0;
pub const SCP_CHAP_DECIDE: i32 = 4; // SCP_CHAP_DINT
                                    // ScpPriorType / LogoPriorType
pub const PRIOR_DUPE: i32 = -1;
pub const PRIOR_NONE: i32 = 0;
// LogoUnitType / LogoResultType
pub const LOGO_UNIT_NORMAL: i32 = 0;
pub const LOGO_RESULT_NONE: i32 = 0;
pub const LOGO_RESULT_DECIDE: i32 = 1;
pub const LOGO_RESULT_ABORT: i32 = 2;
// ScpArType / ScpArExtType defaults
pub const SCP_AR_UNKNOWN: i32 = 0;
pub const SCP_AREXT_NONE: i32 = 0;
// LogoEdgeType
pub const EDGE_RISE: i32 = 0;
pub const EDGE_FALL: i32 = 1;
pub const EDGE_BOTH: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScpRecord {
    pub msec: i32,
    pub msbk: i32,
    pub msmute_s: i32,
    pub msmute_e: i32,
    pub still: bool,
    pub statpos: i32,
    pub score: i32,
    pub chap: i32,
    pub arstat: i32,
    pub arext: i32,
}

impl Default for ScpRecord {
    /// clearRecordScp
    fn default() -> Self {
        ScpRecord {
            msec: 0,
            msbk: 0,
            msmute_s: -1,
            msmute_e: -1,
            still: false,
            statpos: PRIOR_NONE,
            score: 0,
            chap: SCP_CHAP_NONE,
            arstat: SCP_AR_UNKNOWN,
            arext: SCP_AREXT_NONE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LogoRecord {
    pub rise: i32,
    pub fall: i32,
    pub rise_l: i32,
    pub rise_r: i32,
    pub fall_l: i32,
    pub fall_r: i32,
    pub fade_rise: i32,
    pub fade_fall: i32,
    pub intl_rise: i32,
    pub intl_fall: i32,
    pub org_rise: i32,
    pub org_fall: i32,
    pub org_rise_l: i32,
    pub org_rise_r: i32,
    pub org_fall_l: i32,
    pub org_fall_r: i32,
    pub stat_rise: i32,
    pub stat_fall: i32,
    pub unit: i32,
    pub outtype_rise: i32,
    pub outtype_fall: i32,
    pub result_rise: i32,
    pub result_fall: i32,
}

#[derive(Debug, Clone)]
pub struct ExtOpt {
    pub verbose: i32,
    pub frm_lastcut: i32,
    pub msec_cut_in: i32,
    pub msec_cut_out: i32,
    pub wide_cut_in: i32,
    pub wide_cut_out: i32,
    pub fix_cut_in: i32,
    pub fix_cut_out: i32,
    pub fix_wid_cut_i: i32,
    pub fix_wid_cut_o: i32,
    pub flag_no_logo: i32,
    pub old_adjust: i32,
    pub fix_v_line: i32,
    pub fix_f_direct: i32,
    pub fix_nl_g_exact: i32,
    pub fix_sub_list: i32,
    pub fix_sub_path: i32,
    pub v_line: i32,
    pub flag_direct: i32,
    pub nl_g_exact: i32,
    pub sub_list: Vec<u8>,
    pub sub_path: Vec<u8>,
}

impl Default for ExtOpt {
    fn default() -> Self {
        ExtOpt {
            verbose: 0,
            frm_lastcut: 0,
            msec_cut_in: 0,
            msec_cut_out: 0,
            wide_cut_in: 0,
            wide_cut_out: 0,
            fix_cut_in: 0,
            fix_cut_out: 0,
            fix_wid_cut_i: 0,
            fix_wid_cut_o: 0,
            flag_no_logo: 0,
            old_adjust: 0,
            fix_v_line: 0,
            fix_f_direct: 0,
            fix_nl_g_exact: 0,
            fix_sub_list: 0,
            fix_sub_path: 0,
            v_line: 0,
            flag_direct: 0,
            nl_g_exact: 0,
            sub_list: b"common".to_vec(), // 初期検索フォルダ設定
            sub_path: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RecHold {
    pub msec_select1st: i32,
    pub msec_tr_point: i32,
    pub rmsec_head: i32,
    pub rmsec_tail: i32,
}

impl Default for RecHold {
    fn default() -> Self {
        RecHold {
            msec_select1st: -1,
            msec_tr_point: -1,
            rmsec_head: -1,
            rmsec_tail: -1,
        }
    }
}

#[derive(Debug, Clone)]
struct Backup {
    bak_scp: Vec<ScpRecord>,
    bak_logo: Vec<LogoRecord>,
    bak_msec_total_max: i32,
    bak_ext_opt: ExtOpt,
}

#[derive(Debug, Clone)]
pub struct Dataset {
    pub(crate) scp: Vec<ScpRecord>,
    pub(crate) logo: Vec<LogoRecord>,
    pub config: [i32; SIZE_CONFIG_VAR],
    pub ext_opt: ExtOpt,
    pub rec_hold: RecHold,
    pub result_trim: Vec<i32>,
    pub div_file: Vec<i32>,
    pub msec_total_max: i32,
    pub(crate) level_use_logo: i32,
    pub(crate) flag_setup_adj: bool,
    pub(crate) flag_setup_auto: i32,
    pub(crate) nsc_out_detail: i32,
    backup: Backup,
    // initData 固定値 (JlsDataset.cpp; header comments differ, source wins)
    pub msec_val_exact: i32,
    pub msec_val_near1: i32,
    pub msec_val_near2: i32,
    pub msec_val_near3: i32,
    pub msec_val_lap1: i32,
    pub msec_val_lap2: i32,
    pub msec_val_spc: i32,
}

impl Default for Dataset {
    fn default() -> Self {
        Self::new()
    }
}

impl Dataset {
    /// JlsDataset() + initData()
    pub fn new() -> Self {
        let mut d = Dataset {
            scp: Vec::new(),
            logo: Vec::new(),
            config: [0; SIZE_CONFIG_VAR],
            ext_opt: ExtOpt::default(),
            rec_hold: RecHold::default(),
            result_trim: Vec::new(),
            div_file: Vec::new(),
            msec_total_max: 0,
            level_use_logo: 0,
            flag_setup_adj: false,
            flag_setup_auto: 0,
            nsc_out_detail: 0,
            backup: Backup {
                bak_scp: Vec::new(),
                bak_logo: Vec::new(),
                bak_msec_total_max: 0,
                bak_ext_opt: ExtOpt::default(),
            },
            msec_val_exact: 100,
            msec_val_near1: 200,
            msec_val_near2: 400,
            msec_val_near3: 1200,
            msec_val_lap1: 700,
            msec_val_lap2: 2500,
            msec_val_spc: 1200,
        };
        let defaults: [(ConfigVarType, i32); 27] = [
            (ConfigVarType::msecWLogoTRMax, 120 * 1000),
            (ConfigVarType::msecWCompTRMax, 60 * 1000),
            (ConfigVarType::msecWLogoSftMrg, 4200),
            (ConfigVarType::msecWCompFirst, 0),
            (ConfigVarType::msecWCompLast, 0),
            (ConfigVarType::msecWLogoSumMin, 20 * 1000),
            (ConfigVarType::msecWLogoLgMin, 4500),
            (ConfigVarType::msecWLogoCmMin, 40000),
            (ConfigVarType::msecWLogoRevMin, 185 * 1000),
            (ConfigVarType::msecMgnCmDetect, 1500),
            (ConfigVarType::msecMgnCmDivide, 500),
            (ConfigVarType::secWCompSPMin, 6),
            (ConfigVarType::secWCompSPMax, 13),
            (ConfigVarType::flagCutTR, 1),
            (ConfigVarType::flagCutSP, 0),
            (ConfigVarType::flagAddLogo, 1),
            (ConfigVarType::flagAddUC, 0),
            (ConfigVarType::typeNoSc, 0),
            (ConfigVarType::cancelCntSc, 0),
            (ConfigVarType::LogoLevel, 0),
            (ConfigVarType::LogoRevise, 0),
            (ConfigVarType::AutoCmSub, 0),
            (ConfigVarType::msecPosFirst, -1),
            (ConfigVarType::msecLgCutFirst, -1),
            (ConfigVarType::msecZoneFirst, -1),
            (ConfigVarType::msecZoneLast, -1),
            (ConfigVarType::priorityPosFirst, 0),
        ];
        for (tp, v) in defaults {
            d.set_config(tp, v);
        }
        d.backup_logoset_save();
        d
    }

    pub fn set_config(&mut self, tp: ConfigVarType, val: i32) {
        self.config[tp as usize] = val;
    }
    pub fn get_config(&self, tp: ConfigVarType) -> i32 {
        self.config[tp as usize]
    }
    pub fn config_raw(&self, n: usize) -> i32 {
        self.config[n]
    }
    pub fn set_config_raw(&mut self, n: usize, val: i32) {
        self.config[n] = val;
    }

    /// ConfigActType (JlsDataset.cpp getConfigAction)
    pub fn get_config_action(&self, act: i32) -> i32 {
        match act {
            0 => {
                // LogoDelEdge
                let val = self.get_config(ConfigVarType::LogoRevise);
                val % 10
            }
            1 => {
                // LogoDelMid
                let val = self.get_config(ConfigVarType::LogoRevise);
                if ((val / 10 % 10) & 0x1) != 0 {
                    1
                } else {
                    0
                }
            }
            2 => {
                // LogoDelWide
                let val = self.get_config(ConfigVarType::LogoRevise);
                let mut ret = if ((val / 10 % 10) & 0x2) != 0 { 1 } else { 0 };
                let tmp = self.get_config(ConfigVarType::flagAddUC);
                if (tmp % 10) & 0x1 != 0 {
                    ret = 0;
                }
                ret
            }
            3 => {
                // LogoUCRemain
                self.get_config(ConfigVarType::flagAddUC) % 10
            }
            4 => {
                // LogoUCGapCm
                let val = self.get_config(ConfigVarType::flagAddUC);
                let mut ret = if ((val / 10 % 10) & 0x1) != 0 { 1 } else { 0 };
                if self.is_unuse_level_logo() {
                    ret = 1;
                }
                ret
            }
            5 => {
                // MuteNoSc
                let val = self.get_config(ConfigVarType::typeNoSc);
                if val == 0 {
                    if self.is_unuse_level_logo() {
                        1
                    } else {
                        2
                    }
                } else {
                    val
                }
            }
            _ => 0,
        }
    }

    pub fn size_scp(&self) -> i32 {
        self.scp.len() as i32
    }
    pub fn size_logo(&self) -> i32 {
        self.logo.len() as i32
    }
    pub fn empty_logo(&self) -> bool {
        self.logo.is_empty()
    }
    pub fn clear_data_logo_all(&mut self) {
        self.logo.clear();
    }

    pub fn push_scp(&mut self, dt: ScpRecord) {
        self.scp.push(dt);
    }
    pub fn push_logo(&mut self, dt: LogoRecord) {
        self.logo.push(dt);
    }
    pub fn pop_logo(&mut self) {
        self.logo.pop();
    }
    pub fn insert_logo(&mut self, dt: LogoRecord, nlg: i32) {
        if nlg >= 0 && nlg <= self.size_logo() {
            self.logo.insert(nlg as usize, dt);
        }
    }
    pub fn insert_scp(&mut self, dt: ScpRecord, nsc: i32) {
        if nsc >= 0 && nsc <= self.size_scp() {
            self.scp.insert(nsc as usize, dt);
        }
    }
    pub fn get_logo(&self, nlg: i32) -> Option<LogoRecord> {
        if nlg >= 0 && nlg < self.size_logo() {
            Some(self.logo[nlg as usize])
        } else {
            None
        }
    }
    pub fn set_logo(&mut self, dt: LogoRecord, nlg: i32) {
        if nlg >= 0 && nlg < self.size_logo() {
            self.logo[nlg as usize] = dt;
        }
    }
    pub fn get_scp(&self, nsc: i32) -> Option<ScpRecord> {
        if nsc >= 0 && nsc < self.size_scp() {
            Some(self.scp[nsc as usize])
        } else {
            None
        }
    }
    pub fn set_scp(&mut self, dt: ScpRecord, nsc: i32) {
        if nsc >= 0 && nsc < self.size_scp() {
            self.scp[nsc as usize] = dt;
        }
    }

    //--- nrf mapping (nlg = nrf/2, even=rise, odd=fall) ---
    /// flagSetupAuto accessor (used by priorScp rule)
    pub fn flag_setup_adj_val(&self) -> i32 {
        self.flag_setup_adj as i32
    }
    pub fn get_flag_setup_auto_for_dump(&self) -> i32 {
        self.flag_setup_auto
    }
    pub fn get_flag_setup_auto(&self) -> i32 {
        self.flag_setup_auto
    }
    pub fn msec_logo_rise(&self, nlg: i32) -> i32 {
        self.get_logo(nlg).map(|l| l.rise).unwrap_or(0)
    }
    pub fn msec_logo_fall(&self, nlg: i32) -> i32 {
        self.get_logo(nlg).map(|l| l.fall).unwrap_or(0)
    }
    pub fn msec_logo_nrf(&self, nrf: i32) -> i32 {
        let n = nrf / 2;
        match self.get_logo(n) {
            Some(l) => {
                if nrf % 2 == 0 {
                    l.rise
                } else {
                    l.fall
                }
            }
            None => 0,
        }
    }
    pub fn wide_msec_logo_nrf(&self, nrf: i32) -> WideMsec {
        let n = nrf / 2;
        match self.get_logo(n) {
            Some(l) => {
                if nrf % 2 == 0 {
                    WideMsec {
                        just: l.rise,
                        early: l.rise_l,
                        late: l.rise_r,
                    }
                } else {
                    WideMsec {
                        just: l.fall,
                        early: l.fall_l,
                        late: l.fall_r,
                    }
                }
            }
            None => WideMsec::default(),
        }
    }
    pub fn set_msec_logo_nrf(&mut self, nrf: i32, val: i32) {
        let n = (nrf / 2) as usize;
        if n < self.logo.len() {
            let l = &mut self.logo[n];
            if nrf % 2 == 0 {
                l.rise = val;
                if l.rise_l > val {
                    l.rise_l = val;
                }
                if l.rise_r < val {
                    l.rise_r = val;
                }
            } else {
                l.fall = val;
                if l.fall_l > val {
                    l.fall_l = val;
                }
                if l.fall_r < val {
                    l.fall_r = val;
                }
            }
        }
    }

    pub fn msec_scp(&self, nsc: i32) -> i32 {
        self.get_scp(nsc).map(|s| s.msec).unwrap_or(0)
    }
    pub fn msec_scp_bk(&self, nsc: i32) -> i32 {
        self.get_scp(nsc).map(|s| s.msbk).unwrap_or(0)
    }
    pub fn msec_scp_edge(&self, nsc: i32, edge: i32) -> i32 {
        if edge == EDGE_FALL {
            self.msec_scp_bk(nsc)
        } else {
            self.msec_scp(nsc)
        }
    }

    //--- state / setters used by executor ---
    pub fn set_level_use_logo(&mut self, v: i32) {
        self.level_use_logo = v;
    }
    pub fn level_use_logo(&self) -> i32 {
        self.level_use_logo
    }
    pub fn set_flag_setup_adj(&mut self, f: bool) {
        self.flag_setup_adj = f;
    }
    pub fn set_flag_auto_mode(&mut self, v: i32) {
        self.flag_setup_auto = v;
    }
    pub fn flag_auto_mode(&self) -> i32 {
        self.flag_setup_auto
    }
    /// isExistLogo: false only when flagNoLogo > 0 (reference semantics)
    pub fn is_exist_logo(&self) -> bool {
        self.ext_opt.flag_no_logo <= 0
    }
    /// isUnuseLogo: level == CONFIG_LOGO_LEVEL_UNUSE_ALL (1)
    pub fn is_unuse_logo(&self) -> bool {
        self.level_use_logo == 1
    }
    /// isUnuseLevelLogo: level < CONFIG_LOGO_LEVEL_USE_LOW (4)
    pub fn is_unuse_level_logo(&self) -> bool {
        self.level_use_logo < 4
    }
    /// isAutoModeUse: flag == 2
    pub fn is_auto_mode_use(&self) -> bool {
        self.flag_setup_auto == 2
    }
    /// isAutoLogoOnly
    pub fn is_auto_logo_only(&self) -> bool {
        self.flag_setup_auto > 0 && self.is_unuse_level_logo()
    }
    pub fn is_setup_adj_initial(&self) -> bool {
        !self.flag_setup_adj
    }
    /// isValidLogoRise (JlsDataset.cpp:1500)
    pub fn is_valid_logo_rise(&self, nlg: i32) -> bool {
        if nlg >= 0 && nlg < self.size_logo() {
            let t = self.logo[nlg as usize].outtype_rise;
            return t == LOGO_RESULT_NONE || t == LOGO_RESULT_DECIDE;
        }
        false
    }
    /// isValidLogoFall
    pub fn is_valid_logo_fall(&self, nlg: i32) -> bool {
        if nlg >= 0 && nlg < self.size_logo() {
            let t = self.logo[nlg as usize].outtype_fall;
            return t == LOGO_RESULT_NONE || t == LOGO_RESULT_DECIDE;
        }
        false
    }
    /// getNrfDirLogo (JlsDataset.cpp:724)
    pub fn get_nrf_dir_logo(&self, nrf: i32, dir_next: bool, edge: i32, select: i32) -> i32 {
        let size_logo = self.size_logo();
        let mut r = -1i32;
        let check_rise = edge == EDGE_RISE || edge == EDGE_BOTH;
        let check_fall = edge == EDGE_FALL || edge == EDGE_BOTH;
        let mut i = nrf;
        let step = if dir_next { 1 } else { -1 };
        loop {
            i += step;
            if i >= 0 && i < size_logo * 2 {
                let nlg_i = i / 2;
                let edge_i = i % 2; // edgeFromNrf
                if edge_i == EDGE_RISE && check_rise {
                    if self.is_valid_logo_rise(nlg_i) || select == 0 {
                        r = i;
                        break;
                    }
                } else if edge_i == EDGE_FALL
                    && check_fall
                    && (self.is_valid_logo_fall(nlg_i) || select == 0)
                {
                    r = i;
                    break;
                }
            } else {
                break;
            }
        }
        r
    }
    /// getNrfNextLogo
    pub fn get_nrf_next_logo(&self, nrf: i32, edge: i32, select: i32) -> i32 {
        self.get_nrf_dir_logo(nrf, true, edge, select)
    }

    pub fn is_auto_mode_initial(&self) -> bool {
        self.flag_setup_auto == 0
    }

    /// backupLogosetSave / Load (Load restores only the documented subset)
    pub fn backup_logoset_save(&mut self) {
        self.backup.bak_scp = self.scp.clone();
        self.backup.bak_logo = self.logo.clone();
        self.backup.bak_msec_total_max = self.msec_total_max;
        self.backup.bak_ext_opt = self.ext_opt.clone();
    }
    pub fn backup_logoset_load(&mut self) {
        self.scp = self.backup.bak_scp.clone();
        self.logo = self.backup.bak_logo.clone();
        self.ext_opt.flag_no_logo = self.backup.bak_ext_opt.flag_no_logo;
        self.ext_opt.fix_f_direct = self.backup.bak_ext_opt.fix_f_direct;
        self.ext_opt.flag_direct = self.backup.bak_ext_opt.flag_direct;
        self.rec_hold = RecHold {
            msec_select1st: -1,
            msec_tr_point: -1,
            rmsec_head: self.rec_hold.rmsec_head,
            rmsec_tail: self.rec_hold.rmsec_tail,
        };
        self.msec_total_max = self.backup.bak_msec_total_max;
        self.level_use_logo = 0;
        self.flag_setup_adj = false;
        self.flag_setup_auto = 0;
        self.nsc_out_detail = 0;
    }
}
