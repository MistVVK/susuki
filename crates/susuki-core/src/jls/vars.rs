//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Variable substitution engine: port of the register/variable part of
//! JlsScript.cpp (d1a24d75...) plus the list-string helpers.
//! All strings are CP932 byte vectors.

use std::cell::RefCell;
use std::rc::Rc;

use super::cnvstr::Cnv;
use super::dataset::Dataset;
use super::scrreg::ScrReg;

const COMMA_IN: &[u8] = b"@**!("; // DefStrCommaIn (JlsScript.hpp)
const COMMA_OUT: &[u8] = b")!**@"; // DefStrCommaOut

fn icmp_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.eq_ignore_ascii_case(y))
}

fn to_i32_string(v: i32) -> Vec<u8> {
    v.to_string().into_bytes()
}

// ===== list-string helpers (getListStr* / setListStr* in JlsScript.cpp) =====

/// getListStrCommaCheck: returns (flagComma, posSt, posEd)
pub fn list_str_comma_check(s: &[u8], pos: usize) -> (bool, i32, i32) {
    if let Some(pin) = find_sub(s, COMMA_IN, pos) {
        if let Some(pout) = find_sub(s, COMMA_OUT, pin) {
            return (true, pin as i32, (pout + COMMA_OUT.len() - 1) as i32);
        }
    }
    (false, -1, -1)
}

fn find_sub(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from > hay.len() {
        return None;
    }
    hay.windows(needle.len())
        .skip(from)
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// getListStrSize
pub fn list_str_size(s: &[u8]) -> i32 {
    let mut num_list = 0i32;
    if !s.is_empty() {
        let (mut flag_comma, mut pos_cmm_st, mut pos_cmm_ed) = list_str_comma_check(s, 0);
        let mut flag_no_detect = false;
        num_list = 1;
        let mut flag_first_char = true;
        for i in 0..s.len() as i32 {
            if flag_no_detect {
                if i == pos_cmm_ed {
                    flag_no_detect = false;
                }
            } else {
                if flag_comma {
                    if i == pos_cmm_st && flag_first_char {
                        flag_no_detect = true;
                    } else if i > pos_cmm_st {
                        let (f, st, ed) = list_str_comma_check(s, i as usize);
                        flag_comma = f;
                        pos_cmm_st = st;
                        pos_cmm_ed = ed;
                    }
                }
                if s[i as usize] == b',' {
                    num_list += 1;
                    flag_first_char = true;
                } else {
                    flag_first_char = false;
                }
            }
        }
    }
    num_list
}

/// getListStrBaseStore
pub fn list_str_base_store(raw: &[u8]) -> Vec<u8> {
    if raw.contains(&b',') {
        let mut v = COMMA_IN.to_vec();
        v.extend_from_slice(raw);
        v.extend_from_slice(COMMA_OUT);
        v
    } else {
        raw.to_vec()
    }
}

/// getListStrBaseLoad
pub fn list_str_base_load(store: &[u8]) -> Vec<u8> {
    if store.starts_with(COMMA_IN) {
        if let Some(pos) = find_sub(store, COMMA_OUT, 0) {
            let len = COMMA_IN.len();
            if pos >= len {
                return store[len..pos].to_vec();
            }
        }
    }
    store.to_vec()
}

/// getListStrBasePosItem (num>0 only)
pub fn list_str_base_pos_item(s: &[u8], num: i32) -> Option<(i32, i32)> {
    let (mut flag_comma, mut pos_cmm_st, mut pos_cmm_ed) = list_str_comma_check(s, 0);
    let mut n_cur = 0i32;
    let mut pos = -1i32;
    let mut pos_next = 0i32;
    while pos_next >= 0 && n_cur < num && num > 0 {
        pos = pos_next;
        if pos > pos_cmm_st && flag_comma {
            let (f, st, ed) = list_str_comma_check(s, pos.max(0) as usize);
            flag_comma = f;
            pos_cmm_st = st;
            pos_cmm_ed = ed;
        }
        let mut pos_send = pos;
        if pos == pos_cmm_st && flag_comma {
            pos_send = pos_cmm_ed;
        }
        match find_sub(s, b",", pos_send.max(0) as usize) {
            None => pos_next = -1,
            Some(p) => pos_next = p as i32 + 1,
        }
        n_cur += 1;
    }
    if pos >= 0 && n_cur == num {
        let len_item = if pos <= pos_next {
            pos_next - pos - 1
        } else {
            s.len() as i32 - pos
        };
        return Some((pos, len_item));
    }
    None
}

/// getListStrPosItem
pub fn list_str_pos_item(s: &[u8], num: i32, flag_ins: bool) -> Option<(i32, i32)> {
    let num_list = list_str_size(s);
    let mut num_abs = if num >= 0 { num } else { num_list + num + 1 };
    if flag_ins && num < 0 {
        num_abs += 1;
    }
    if num_abs > 0 && num_abs == num_list + 1 && flag_ins {
        return Some((s.len() as i32, 0));
    }
    if num_abs > num_list || num_abs == 0 {
        return None;
    }
    list_str_base_pos_item(s, num_abs)
}

/// getListStrElement
pub fn list_str_element(s: &[u8], num: i32) -> Option<Vec<u8>> {
    let (pos_item, len_item) = list_str_pos_item(s, num, false)?;
    let st = pos_item as usize;
    let en = (st + len_item as usize).min(s.len());
    Some(list_str_base_load(&s[st..en]))
}

/// setListStrIns
pub fn list_str_ins(s: &mut Vec<u8>, item: &[u8], num: i32) -> bool {
    let len_list = s.len() as i32;
    let loc_st = match list_str_pos_item(s, num, true) {
        Some((p, _)) => p,
        None => return false,
    };
    let store = list_str_base_store(item);
    if loc_st == 0 {
        if s.is_empty() {
            *s = store;
        } else {
            let mut v = store;
            v.push(b',');
            v.extend_from_slice(s);
            *s = v;
        }
    } else if loc_st == len_list {
        s.push(b',');
        s.extend_from_slice(&store);
    } else {
        let str_tmp = s[(loc_st - 1) as usize..].to_vec();
        let mut v: Vec<u8> = Vec::new();
        if loc_st == 1 {
            v.push(b',');
            v.extend_from_slice(&store);
        } else {
            v.extend_from_slice(&s[..(loc_st - 1) as usize]);
            v.push(b',');
            v.extend_from_slice(&store);
        }
        v.extend_from_slice(&str_tmp);
        *s = v;
    }
    true
}

/// setListStrDel
pub fn list_str_del(s: &mut Vec<u8>, num: i32) -> bool {
    let len_list = s.len() as i32;
    let (pos_item, len_item) = match list_str_pos_item(s, num, false) {
        Some(v) => v,
        None => return false,
    };
    if pos_item == 0 {
        if len_item + 1 >= len_list {
            s.clear();
        } else {
            let st = (len_item + 1) as usize;
            *s = if st <= s.len() {
                s[st..].to_vec()
            } else {
                Vec::new()
            };
        }
    } else if pos_item == 1 {
        let st = (len_item + 1) as usize;
        *s = if st <= s.len() {
            s[st..].to_vec()
        } else {
            Vec::new()
        };
    } else {
        let str_tmp = if (pos_item + len_item) < len_list {
            s[(pos_item + len_item) as usize..].to_vec()
        } else {
            Vec::new()
        };
        let mut v: Vec<u8> = Vec::new();
        let cut = (pos_item - 1) as usize;
        v.extend_from_slice(if cut <= s.len() { &s[..cut] } else { s });
        v.extend_from_slice(&str_tmp);
        *s = v;
    }
    true
}

/// setListStrRep
pub fn list_str_rep(s: &mut Vec<u8>, item: &[u8], num: i32) -> bool {
    if list_str_del(s, num) {
        return list_str_ins(s, item, num);
    }
    false
}

// ===== variable engine (JlsScript.cpp register access layer) =====

pub struct Vars {
    pub reg: ScrReg,
    pub cnv: Rc<RefCell<Cnv>>,
    /// shared error sink (same object as GState::msg in exec wiring)
    pub msg: Rc<RefCell<Vec<u8>>>,
    pub stop_err: Rc<RefCell<bool>>,
}

impl Vars {
    pub fn new(cnv: Rc<RefCell<Cnv>>) -> Self {
        Vars {
            reg: ScrReg::new(),
            cnv,
            msg: Rc::new(RefCell::new(Vec::new())),
            stop_err: Rc::new(RefCell::new(false)),
        }
    }

    pub fn add_msg_error(&mut self, msg: &[u8]) {
        if !*self.stop_err.borrow() {
            self.msg.borrow_mut().extend_from_slice(msg);
        }
    }
    pub fn pop_msg_error(&mut self) -> Option<Vec<u8>> {
        let mut b = self.msg.borrow_mut();
        if b.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut *b))
        }
    }
    pub fn stop_add_msg_error(&mut self, flag: bool) {
        *self.stop_err.borrow_mut() = flag;
    }

    /// setArgReg (JlsScript.cpp:85)
    pub fn set_arg_reg(&mut self, name: &[u8], val: &[u8]) -> bool {
        if name.is_empty() {
            return false;
        }
        if name.contains(&b'[') {
            let mut m = b"error: argument array ".to_vec();
            m.extend_from_slice(name);
            m.push(b'\n');
            self.add_msg_error(&m);
            return false;
        }
        self.reg.set_arg_reg(name, val)
    }

    /// setJlsRegVar
    pub fn set_jls_reg_var(
        &mut self,
        name: &[u8],
        val: &[u8],
        overwrite: bool,
        ds: &mut Dataset,
    ) -> bool {
        self.set_jls_reg_var_with_local(name, val, overwrite, false, ds)
    }
    pub fn set_jls_reg_var_local(
        &mut self,
        name: &[u8],
        val: &[u8],
        overwrite: bool,
        ds: &mut Dataset,
    ) -> bool {
        self.set_jls_reg_var_with_local(name, val, overwrite, true, ds)
    }
    /// setJlsRegVarWithLocal
    pub fn set_jls_reg_var_with_local(
        &mut self,
        name: &[u8],
        val: &[u8],
        overwrite: bool,
        flag_local: bool,
        ds: &mut Dataset,
    ) -> bool {
        if name.is_empty() {
            return false;
        }
        let mut name_w = name.to_vec();
        let mut val_w = val.to_vec();
        let mut ow = overwrite;
        let success = self.check_jls_reg_var_write(&mut name_w, &mut val_w, &mut ow);
        let success = if success {
            if flag_local {
                self.reg.set_local_reg_var(&name_w, &val_w, ow)
            } else {
                self.reg.set_reg_var(&name_w, &val_w, ow)
            }
        } else {
            false
        };
        if success {
            self.set_jls_reg_var_couple(&name_w, val, ds);
        }
        success
    }

    /// setJlsRegVarCouple: sync HEAD/TAIL frame<->time system variables.
    pub fn set_jls_reg_var_couple(&mut self, name: &[u8], val: &[u8], ds: &mut Dataset) {
        let (add_name, type_add): (&[u8], i32) = if icmp_eq(name, b"HEADFRAME") {
            (b"HEADTIME", 1)
        } else if icmp_eq(name, b"TAILFRAME") {
            (b"TAILTIME", 1)
        } else if icmp_eq(name, b"HEADTIME") {
            (b"HEADFRAME", 2)
        } else if icmp_eq(name, b"TAILTIME") {
            (b"TAILFRAME", 2)
        } else {
            return;
        };
        let (_p, v) = self.cnv.borrow().get_str_val_msec_m1(val, 0);
        if let Some(val_ms) = v {
            let add_val = if type_add == 2 {
                self.cnv.borrow().get_string_frame_msec_m1(val_ms)
            } else {
                self.cnv.borrow().get_string_time_msec_m1(val_ms)
            };
            self.reg.set_reg_var(add_name, &add_val, true);
        }
        // head/tail information update
        if let Some(sub) = self.get_jls_reg_var(b"HEADTIME", true).1 {
            if let Some(v) = self.cnv.borrow().get_str_val_msec_m1(&sub, 0).1 {
                ds.rec_hold.rmsec_head = v;
            }
        }
        if let Some(sub) = self.get_jls_reg_var(b"TAILTIME", true).1 {
            if let Some(v) = self.cnv.borrow().get_str_val_msec_m1(&sub, 0).1 {
                ds.rec_hold.rmsec_tail = v;
            }
        }
    }

    /// getJlsRegVar
    pub fn get_jls_reg_var(&mut self, cand: &[u8], exact: bool) -> (i32, Option<Vec<u8>>) {
        let mut name_part = cand.to_vec();
        let mut flag_num = false;
        if cand.first() == Some(&b'#') {
            flag_num = true;
            name_part = cand[1..].to_vec();
        }
        let (flag_list, num_item, len_full_var) = self.check_jls_reg_var_list(&mut name_part);
        let (mut len_var, val) = self.reg.get_reg_var(&name_part, exact);
        let mut val = val.unwrap_or_default();
        if flag_list && name_part.len() as i32 == len_var {
            let str_list = val.clone();
            match list_str_element(&str_list, num_item) {
                Some(item) => val = item,
                None => {
                    // getListStrElement clears its output before failing
                    val = Vec::new();
                    let mut m = b"error: out of range ".to_vec();
                    m.extend_from_slice(&str_list);
                    m.extend_from_slice(b"[");
                    m.extend_from_slice(&to_i32_string(num_item));
                    m.extend_from_slice(b"]\n");
                    self.add_msg_error(&m);
                }
            }
            len_var = len_full_var;
        }
        if flag_num {
            let num_list = list_str_size(&val);
            val = to_i32_string(num_list);
        }
        let out = if len_var > 0 { Some(val) } else { None };
        (len_var, out)
    }

    /// checkJlsRegVarList (name_part is in/out, trimmed to base name on hit)
    pub fn check_jls_reg_var_list(&mut self, name_part: &mut Vec<u8>) -> (bool, i32, i32) {
        let mut num_item = 0i32;
        let mut len_full_var = 0i32;
        let mut flag_list = false;
        let mut str_item: Vec<u8> = Vec::new();
        {
            let loc_st = find_sub(name_part, b"[", 0).map(|p| p as i32);
            if let Some(loc_st) = loc_st {
                if loc_st != 0 {
                    let after = loc_st as usize + 1;
                    let tail_hit = if after < name_part.len() {
                        find_sub(name_part, b"]", after).map(|p| p as i32)
                    } else {
                        None
                    };
                    if let Some(loc_ed0) = tail_hit {
                        if loc_ed0 != loc_st + 1 {
                            let loc_ct = find_sub(name_part, b"$", 0).map(|p| p as i32);
                            let conflict = matches!(loc_ct, Some(c) if c < loc_st);
                            if !conflict {
                                let mut dup = 0i32;
                                let mut ed = loc_st + 1;
                                loop {
                                    let c = name_part.get(ed as usize).copied().unwrap_or(0);
                                    if (c != b']' || dup > 0) && c != 0 {
                                        if c == b']' {
                                            dup -= 1;
                                        } else if c == b'[' {
                                            dup += 1;
                                        }
                                        ed += 1;
                                    } else {
                                        break;
                                    }
                                }
                                if name_part.get(ed as usize).copied().unwrap_or(0) == b']' {
                                    let ary_buf =
                                        name_part[(loc_st + 1) as usize..ed as usize].to_vec();
                                    let (item, _ok) = self.replace_buf_var(&ary_buf);
                                    str_item = item;
                                    *name_part = name_part[..loc_st as usize].to_vec();
                                    len_full_var = ed + 1;
                                    flag_list = true;
                                }
                            }
                        }
                    }
                }
            }
        }
        if flag_list {
            let (consumed, v) = self.cnv.borrow().get_str_val_num(&str_item, 0);
            match v {
                Some(x) if consumed >= 0 => num_item = x,
                _ => {
                    let mut m = b"error: [value] must be integer in ".to_vec();
                    m.extend_from_slice(name_part);
                    m.extend_from_slice(b"[");
                    m.extend_from_slice(&str_item);
                    m.extend_from_slice(b"]\n");
                    self.add_msg_error(&m);
                    flag_list = false;
                }
            }
        }
        (flag_list, num_item, len_full_var)
    }

    /// checkJlsRegVarWrite (all in/out per reference signature)
    pub fn check_jls_reg_var_write(
        &mut self,
        name_write: &mut Vec<u8>,
        val_write: &mut Vec<u8>,
        overwrite: &mut bool,
    ) -> bool {
        let mut success = true;
        let (flag_list, num_item, _len_full) = self.check_jls_reg_var_list(name_write);
        if flag_list {
            success = false;
            let (len, full) = self.get_jls_reg_var(name_write, true);
            if len > 0 {
                let mut str_list = full.unwrap();
                let num_list = list_str_size(&str_list);
                if (num_item > 0 && num_item <= num_list)
                    || (num_item < 0 && num_list + num_item >= 0)
                {
                    success = list_str_rep(&mut str_list, val_write, num_item);
                } else if num_list + 1 == num_item {
                    success = list_str_ins(&mut str_list, val_write, num_item);
                    *overwrite = true;
                }
                if success {
                    *val_write = str_list;
                } else {
                    let mut m = b"error: out of range ".to_vec();
                    m.extend_from_slice(name_write);
                    m.extend_from_slice(b"[");
                    m.extend_from_slice(&to_i32_string(num_item));
                    m.extend_from_slice(b"]\n");
                    self.add_msg_error(&m);
                }
            } else {
                if num_item == 1 {
                    let mut str_list: Vec<u8> = Vec::new();
                    success = list_str_ins(&mut str_list, val_write, num_item);
                    if success {
                        *val_write = str_list;
                    }
                }
            }
        }
        success
    }

    /// replaceBufVar
    pub fn replace_buf_var(&mut self, src: &[u8]) -> (Vec<u8>, bool) {
        let mut dst: Vec<u8> = Vec::new();
        let mut success = true;
        let pos_cmt = self.cnv.borrow().get_str_pos_comment(src, 0);
        let mut pos_base = 0i32;
        while pos_base >= 0 {
            let mut pos_var = find_sub(src, b"$", pos_base.max(0) as usize).map(|p| p as i32);
            if let Some(pv) = pos_var {
                let flag_cmt = pv >= pos_cmt && pos_cmt >= 0;
                if flag_cmt {
                    self.stop_add_msg_error(true);
                }
                if pv > pos_base {
                    dst.extend_from_slice(&src[pos_base as usize..pv as usize]);
                    pos_base = pv;
                }
                let (str_val, len_var) = self.replace_reg_var_in_buf(src, pos_base);
                if len_var > 0 {
                    dst.extend_from_slice(&str_val);
                    pos_base += len_var;
                } else {
                    if !flag_cmt {
                        success = false;
                        let mut m = b"error: not defined variable in ".to_vec();
                        m.extend_from_slice(src);
                        m.push(b'\n');
                        self.add_msg_error(&m);
                    }
                    pos_var = None;
                }
                if flag_cmt {
                    self.stop_add_msg_error(true);
                }
            }
            //--- 変数がなければ残りすべてコピー (also when replacement failed) ---
            if pos_var.is_none() {
                let st = pos_base.max(0) as usize;
                dst.extend_from_slice(if st <= src.len() { &src[st..] } else { &[] });
                pos_base = -1;
            }
        }
        (dst, success)
    }

    /// replaceRegVarInBuf: returns (value, consumed length)
    pub fn replace_reg_var_in_buf(&mut self, buf: &[u8], pos: i32) -> (Vec<u8>, i32) {
        let mut ret = 0i32;
        let mut str_val = Vec::new();
        let ch = |i: i32| buf.get(i.max(0) as usize).copied().unwrap_or(0);
        if ch(pos) == b'$' {
            let mut pos = pos + 1;
            let mut flag_num = false;
            if ch(pos) == b'#' {
                flag_num = true;
                pos += 1;
            }
            let exact;
            let var_st;
            if ch(pos) == b'{' {
                exact = true;
                pos += 1;
                var_st = pos;
                let mut dup = 0i32;
                loop {
                    let c = ch(pos);
                    if (c != b'}' || dup > 0) && c != 0 {
                        if c == b'}' {
                            dup -= 1;
                        } else if c == b'{' {
                            dup += 1;
                        }
                        pos += 1;
                    } else {
                        break;
                    }
                }
            } else {
                exact = false;
                var_st = pos;
                loop {
                    let c = ch(pos);
                    if c != b' ' && c != 0 {
                        pos += 1;
                    } else {
                        break;
                    }
                }
            }
            let mut var_ed = pos;
            if ch(pos) == b'}' || ch(pos) == b' ' {
                var_ed -= 1;
            }
            if var_st <= var_ed {
                let mut cand =
                    buf[var_st as usize..(var_ed + 1).min(buf.len() as i32) as usize].to_vec();
                if flag_num {
                    let mut v = b"#".to_vec();
                    v.extend_from_slice(&cand);
                    cand = v;
                }
                let (nmatch, val) = self.get_jls_reg_var(&cand, exact);
                if nmatch > 0 {
                    str_val = val.unwrap_or_default();
                    ret = nmatch + 1;
                    if flag_num {
                        ret += 1;
                    }
                    if exact {
                        ret += 2;
                    }
                }
            }
        }
        (str_val, ret)
    }
}
