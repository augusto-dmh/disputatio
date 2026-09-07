//! Entities and ports. Pure: no framework imports, no IO (architecture.md
//! layering rule 1) — `infrastructure` implements these ports.

pub mod anki;
pub mod chunk;
pub mod queue;
pub mod repo;
pub mod review;
pub mod session;
pub mod settings;
pub mod source;
pub mod vault;
