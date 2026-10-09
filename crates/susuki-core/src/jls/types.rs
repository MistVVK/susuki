//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Execution-layer enums from JlsNameSpace.hpp (not part of the generated
//! tables; kept here so tables.rs stays generator-owned).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheExeType {
    None,
    LazyS,
    LazyA,
    LazyE,
    Mem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LazyType {
    None,
    Full,
    LazyS,
    LazyA,
    LazyE,
}
