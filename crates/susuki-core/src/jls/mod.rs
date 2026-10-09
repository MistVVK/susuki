//! Design 7 / Appendix B: JLS script engine.
//! tables.rs is generated from the REF-JLS fixed commit (see header comment);
//! evaluation/executing code is added per design 7.2-7.6 in subsequent work.

pub mod autoarg;
pub mod autoexec;
pub mod cmdarg;
pub mod cmdlimit;
pub mod cnvstr;
pub mod dataset;
pub mod dataset_output;
pub mod dataset_search;
pub mod decode;
pub mod exec;
pub mod forbid;
pub mod gstate;
pub mod input;
pub mod limit;
pub mod memstore;
pub mod reform_all;
pub mod reform_base;
pub mod reform_cm;
pub mod reform_d;
pub mod reform_data;
pub mod scrreg;
pub mod state;
pub mod tables;
pub mod types;
pub mod vars;
