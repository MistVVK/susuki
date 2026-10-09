//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Port of JlsScrGlobal.cpp (minus register access, which lives in
//! scrreg.rs/vars.rs): mem/lazy storage facade, error sink, flags.

use std::collections::VecDeque;

use super::memstore::ScrMem;
use super::types::LazyType;

pub struct GState {
    pub mem: ScrMem,
    /// shared error sink (Vars::msg points to the same cell when wired)
    pub msg: std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
    pub stop_err: std::rc::Rc<std::cell::RefCell<bool>>,
    exe1st: bool,
    exit_flag: bool,
    lazy_st_ini_auto: bool,
    path_name_jl: Vec<u8>,
}

impl GState {
    pub fn new() -> Self {
        GState {
            mem: ScrMem::new(),
            msg: std::rc::Rc::new(std::cell::RefCell::new(Vec::new())),
            stop_err: std::rc::Rc::new(std::cell::RefCell::new(false)),
            exe1st: true,
            exit_flag: false,
            lazy_st_ini_auto: false,
            path_name_jl: Vec::new(),
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

    pub fn set_exe1st(&mut self, flag: bool) {
        self.exe1st = flag;
    }
    pub fn is_exe1st(&self) -> bool {
        self.exe1st
    }
    pub fn set_cmd_exit(&mut self, flag: bool) {
        self.exit_flag = flag;
    }
    pub fn is_cmd_exit(&self) -> bool {
        self.exit_flag
    }
    pub fn set_lazy_state_ini_auto(&mut self, flag: bool) {
        self.lazy_st_ini_auto = flag;
    }
    pub fn is_lazy_state_ini_auto(&self) -> bool {
        self.lazy_st_ini_auto
    }
    pub fn set_path_name_jl(&mut self, msg: &[u8]) {
        self.path_name_jl = msg.to_vec();
    }
    pub fn get_path_name_jl(&self) -> &[u8] {
        &self.path_name_jl
    }

    pub fn is_lazy_exist(&mut self, type_lazy: LazyType) -> bool {
        self.mem.is_lazy_exist(type_lazy)
    }
    pub fn pop_list_by_lazy(&mut self, que: &mut VecDeque<Vec<u8>>, type_lazy: LazyType) -> bool {
        self.mem.pop_list_by_lazy(que, type_lazy)
    }
    pub fn get_list_by_name(&mut self, que: &mut VecDeque<Vec<u8>>, str_name: &[u8]) -> bool {
        self.mem.get_list_by_name(que, str_name)
    }

    /// setLazyStore: false = the current line should not execute now
    pub fn set_lazy_store(&mut self, type_lazy: LazyType, str_buf: &[u8]) -> bool {
        let mut enable_exe = true;
        if type_lazy != LazyType::None {
            let success = self.mem.push_str_by_lazy(type_lazy, str_buf);
            enable_exe = false;
            if !success {
                let mut m = b"error : failed Lazy push: ".to_vec();
                m.extend_from_slice(str_buf);
                m.push(b'\n');
                self.add_msg_error(&m);
            }
        }
        enable_exe
    }
    /// setMemStore: always false (store lines never execute in place)
    pub fn set_mem_store(&mut self, str_name: &[u8], str_buf: &[u8]) -> bool {
        let enable_exe = false;
        let success = self.mem.push_str_by_name(str_name, str_buf);
        if !success {
            let mut m = b"error : failed memory push: ".to_vec();
            m.extend_from_slice(str_buf);
            m.push(b'\n');
            self.add_msg_error(&m);
        }
        enable_exe
    }
    pub fn set_mem_erase(&mut self, str_name: &[u8]) -> bool {
        self.mem.erase_mem_by_name(str_name)
    }
    pub fn set_mem_copy(&mut self, src: &[u8], dst: &[u8]) -> bool {
        self.mem.copy_mem_by_name(src, dst)
    }
    pub fn set_mem_move(&mut self, src: &[u8], dst: &[u8]) -> bool {
        self.mem.move_mem_by_name(src, dst)
    }
    pub fn set_mem_append(&mut self, src: &[u8], dst: &[u8]) -> bool {
        self.mem.append_mem_by_name(src, dst)
    }
}

impl Default for GState {
    fn default() -> Self {
        Self::new()
    }
}
