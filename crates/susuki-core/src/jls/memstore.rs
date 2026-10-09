//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Port of JlsScrMem.{hpp,cpp}: storage of delayed-execution command lines.

use std::collections::{HashMap, VecDeque};

use super::types::LazyType;

const SIZE_MEMVARNUM_MAX: usize = 2048; // JlsNameSpace.hpp

// MemSpecialID index: DUMMY, LAZY_FULL, LAZY_S, LAZY_A, LAZY_E, NoData
const MEM_SPECIAL_STRINGS: [&[u8]; 6] = [b"DUMMY", b"LAZY", b"LAZY_S", b"LAZY_A", b"LAZY_E", b""];
const SIZE_MEM_SPECIAL_ID: usize = 6;
const SCR_MEM_STR_LAZY: &[u8] = b"__LAZY__";

fn icmp_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.eq_ignore_ascii_case(y))
}

fn lazy_string(type_lazy: LazyType) -> &'static str {
    match type_lazy {
        LazyType::LazyS => "LAZY_S",
        LazyType::LazyA => "LAZY_A",
        LazyType::LazyE => "LAZY_E",
        LazyType::Full => "LAZY",
        LazyType::None => "",
    }
}

#[derive(Default)]
pub struct MemArg {
    flag_dummy: bool,
    list_name: Vec<Vec<u8>>,
}

impl MemArg {
    pub fn new() -> Self {
        let mut a = MemArg {
            flag_dummy: false,
            list_name: Vec::new(),
        };
        a.clear_arg();
        a
    }

    fn clear_arg(&mut self) {
        self.flag_dummy = false;
        self.list_name.clear();
        self.list_name.push(Vec::new()); // first entry is the base name
    }

    /// setNameByStr
    pub fn set_name_by_str(&mut self, str_name: &[u8]) {
        self.clear_arg();
        let mut base = true;
        if let Some(id) = Self::find_special_name(str_name) {
            match id {
                0 => {
                    self.flag_dummy = true;
                    base = false;
                }
                1 => {
                    self.set_map_name_to_ext(b""); // Lazy body carries no ext ids
                    base = false;
                }
                5 => {
                    base = false;
                }
                _ => {}
            }
        } else {
            self.set_map_name_to_ext(str_name);
        }
        if base {
            self.set_map_name_to_base(str_name);
        }
    }

    /// setNameByLazy
    pub fn set_name_by_lazy(&mut self, type_lazy: LazyType) {
        self.clear_arg();
        let name = lazy_string(type_lazy);
        if !name.is_empty() {
            self.set_name_by_str(name.as_bytes());
        }
    }

    pub fn is_exist_base_name(&self) -> bool {
        !self.list_name[0].is_empty()
    }
    pub fn is_exist_ext_name(&self) -> bool {
        let n = self.list_name.len() as i32;
        n > 1 && n <= SIZE_MEM_SPECIAL_ID as i32
    }
    pub fn is_name_dummy(&self) -> bool {
        self.flag_dummy
    }
    pub fn get_base_name(&self) -> Vec<u8> {
        self.list_name[0].clone()
    }
    pub fn get_name_list(&self) -> Vec<Vec<u8>> {
        self.list_name.clone()
    }

    fn set_map_name_to_base(&mut self, str_name: &[u8]) {
        self.list_name[0] = str_name.to_vec();
    }

    fn set_map_name_to_ext(&mut self, str_name: &[u8]) {
        for (i, sp) in MEM_SPECIAL_STRINGS
            .iter()
            .enumerate()
            .take(SIZE_MEM_SPECIAL_ID)
        {
            if i == 2 || i == 3 || i == 4 {
                let mut var_str = sp.to_vec();
                if !str_name.is_empty() {
                    var_str = str_name
                        .iter()
                        .chain(SCR_MEM_STR_LAZY)
                        .chain(*sp)
                        .copied()
                        .collect();
                }
                self.list_name.push(var_str);
            }
        }
    }

    fn find_special_name(str_name: &[u8]) -> Option<usize> {
        MEM_SPECIAL_STRINGS
            .iter()
            .take(SIZE_MEM_SPECIAL_ID)
            .position(|sp| icmp_eq(str_name, sp))
    }
}

#[derive(Clone, Copy)]
pub struct CopyFlag {
    pub add: bool,
    pub mv: bool,
}

pub struct ScrMem {
    map_var: HashMap<Vec<u8>, VecDeque<Vec<u8>>>,
}

impl ScrMem {
    pub fn new() -> Self {
        ScrMem {
            map_var: HashMap::new(),
        }
    }

    pub fn is_lazy_exist(&mut self, type_lazy: LazyType) -> bool {
        let mut marg = MemArg::new();
        marg.set_name_by_lazy(type_lazy);
        let slist = marg.get_name_list();
        let mut exist = false;
        for name in &slist {
            if self.mem_is_exist(name) {
                exist = true;
            }
        }
        exist
    }

    pub fn push_str_by_name(&mut self, str_name: &[u8], str_buf: &[u8]) -> bool {
        let mut marg = MemArg::new();
        marg.set_name_by_str(str_name);
        self.exe_cmd_push_str(&marg, str_buf)
    }
    pub fn push_str_by_lazy(&mut self, type_lazy: LazyType, str_buf: &[u8]) -> bool {
        let mut marg = MemArg::new();
        marg.set_name_by_lazy(type_lazy);
        self.exe_cmd_push_str(&marg, str_buf)
    }
    pub fn get_list_by_name(&mut self, que: &mut VecDeque<Vec<u8>>, str_name: &[u8]) -> bool {
        let mut marg = MemArg::new();
        marg.set_name_by_str(str_name);
        self.exe_cmd_get_list(
            que,
            &marg,
            CopyFlag {
                add: true,
                mv: false,
            },
        )
    }
    pub fn pop_list_by_name(&mut self, que: &mut VecDeque<Vec<u8>>, str_name: &[u8]) -> bool {
        let mut marg = MemArg::new();
        marg.set_name_by_str(str_name);
        self.exe_cmd_get_list(
            que,
            &marg,
            CopyFlag {
                add: true,
                mv: true,
            },
        )
    }
    pub fn pop_list_by_lazy(&mut self, que: &mut VecDeque<Vec<u8>>, type_lazy: LazyType) -> bool {
        let mut marg = MemArg::new();
        marg.set_name_by_lazy(type_lazy);
        self.exe_cmd_get_list(
            que,
            &marg,
            CopyFlag {
                add: true,
                mv: true,
            },
        )
    }
    pub fn erase_mem_by_name(&mut self, str_name: &[u8]) -> bool {
        let mut marg = MemArg::new();
        marg.set_name_by_str(str_name);
        self.exe_cmd_erase_mem(&marg)
    }
    pub fn copy_mem_by_name(&mut self, src: &[u8], dst: &[u8]) -> bool {
        let (sarg, darg) = self.make_pair(src, dst);
        self.exe_cmd_copy_mem(
            &sarg,
            &darg,
            CopyFlag {
                add: false,
                mv: false,
            },
        )
    }
    pub fn move_mem_by_name(&mut self, src: &[u8], dst: &[u8]) -> bool {
        let (sarg, darg) = self.make_pair(src, dst);
        self.exe_cmd_copy_mem(
            &sarg,
            &darg,
            CopyFlag {
                add: false,
                mv: true,
            },
        )
    }
    pub fn append_mem_by_name(&mut self, src: &[u8], dst: &[u8]) -> bool {
        let (sarg, darg) = self.make_pair(src, dst);
        self.exe_cmd_copy_mem(
            &sarg,
            &darg,
            CopyFlag {
                add: true,
                mv: false,
            },
        )
    }

    fn make_pair(&self, src: &[u8], dst: &[u8]) -> (MemArg, MemArg) {
        let mut sarg = MemArg::new();
        let mut darg = MemArg::new();
        sarg.set_name_by_str(src);
        darg.set_name_by_str(dst);
        (sarg, darg)
    }

    fn exe_cmd_push_str(&mut self, arg_dst: &MemArg, str_buf: &[u8]) -> bool {
        if arg_dst.is_exist_base_name() {
            let name = arg_dst.get_base_name();
            self.mem_push_str(&name, str_buf)
        } else {
            arg_dst.is_name_dummy()
        }
    }

    fn exe_cmd_get_list(
        &mut self,
        que: &mut VecDeque<Vec<u8>>,
        arg_src: &MemArg,
        flags: CopyFlag,
    ) -> bool {
        let mut success;
        if arg_src.is_exist_base_name() {
            let name = arg_src.get_base_name();
            success = self.mem_get_list(que, &name, flags);
        } else {
            if !flags.add {
                que.clear();
            }
            success = arg_src.is_name_dummy();
        }
        if arg_src.is_exist_ext_name() {
            let mut arg_dst = MemArg::new();
            arg_dst.set_name_by_lazy(LazyType::Full); // Lazy body
            let flags2 = CopyFlag {
                add: true,
                mv: flags.mv,
            };
            if self.exe_cmd_copy_mem(arg_src, &arg_dst, flags2) {
                success = true;
            }
        }
        success
    }

    fn exe_cmd_erase_mem(&mut self, arg_dst: &MemArg) -> bool {
        let mut success = false;
        for name in arg_dst.get_name_list() {
            success |= self.mem_erase(&name);
        }
        if arg_dst.is_name_dummy() {
            success = true;
        }
        success
    }

    fn exe_cmd_copy_mem(&mut self, arg_src: &MemArg, arg_dst: &MemArg, flags: CopyFlag) -> bool {
        let slist = arg_src.get_name_list();
        let dlist = arg_dst.get_name_list();
        let smax = slist.len();
        let dmax = dlist.len();
        let mut success = false;
        let loop_max = smax.max(dmax);
        for i in 0..loop_max {
            let si;
            let mut exist_src = i < smax;
            let mut exist_dst = i < dmax;
            if exist_src {
                exist_src = !slist[i].is_empty();
            }
            if exist_dst {
                exist_dst = !dlist[i].is_empty();
            }
            if exist_src && exist_dst {
                si = self.mem_copy(&slist[i], &dlist[i], flags);
            } else {
                si = exist_src && arg_dst.is_name_dummy();
                if exist_src && flags.mv {
                    self.mem_erase(&slist[i]);
                }
                if exist_dst && !flags.add {
                    self.mem_erase(&dlist[i]);
                }
            }
            success |= si;
        }
        success
    }

    fn mem_push_str(&mut self, str_name: &[u8], str_buf: &[u8]) -> bool {
        if self.map_var.len() >= SIZE_MEMVARNUM_MAX {
            return false;
        }
        if str_name.is_empty() {
            return false;
        }
        self.map_var
            .entry(str_name.to_vec())
            .or_default()
            .push_back(str_buf.to_vec());
        true
    }

    fn mem_get_list(
        &mut self,
        que: &mut VecDeque<Vec<u8>>,
        str_name: &[u8],
        flags: CopyFlag,
    ) -> bool {
        if !self.mem_is_exist(str_name) {
            let q: VecDeque<Vec<u8>> = VecDeque::new();
            Self::set_queue(que, q, flags);
            return false;
        }
        let key = str_name.to_vec();
        let src = self.map_var.entry(key).or_default().clone();
        Self::set_queue(que, src, flags);
        if flags.mv {
            if let Some(q) = self.map_var.get_mut(str_name) {
                q.clear();
            }
        }
        true
    }

    fn mem_erase(&mut self, str_name: &[u8]) -> bool {
        if !self.mem_is_name_exist(str_name) {
            return false;
        }
        self.map_var.remove(str_name);
        true
    }

    fn mem_copy(&mut self, str_src: &[u8], str_dst: &[u8], flags: CopyFlag) -> bool {
        if !self.mem_is_exist(str_src) {
            if !flags.add {
                self.mem_erase(str_dst);
            }
            return false;
        }
        let src = self.map_var.get(str_src).cloned().unwrap_or_default();
        let dst_key = str_dst.to_vec();
        let dst = self.map_var.entry(dst_key).or_default();
        let mut src2 = src.clone();
        Self::set_queue(dst, src2.clone(), flags);
        if flags.mv {
            src2.clear();
            if let Some(s) = self.map_var.get_mut(str_src) {
                s.clear();
            }
        }
        let _ = src2;
        true
    }

    fn mem_is_exist(&mut self, str_name: &[u8]) -> bool {
        if str_name.is_empty() {
            return false;
        }
        match self.map_var.get_mut(str_name) {
            None => false,
            Some(q) => !q.is_empty(),
        }
    }
    fn mem_is_name_exist(&self, str_name: &[u8]) -> bool {
        if str_name.is_empty() {
            return false;
        }
        self.map_var.contains_key(str_name)
    }

    fn set_queue(dst: &mut VecDeque<Vec<u8>>, src: VecDeque<Vec<u8>>, flags: CopyFlag) {
        if flags.add {
            dst.extend(src);
        } else {
            *dst = src;
        }
    }
}

impl Default for ScrMem {
    fn default() -> Self {
        Self::new()
    }
}
