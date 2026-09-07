//! Entities and ports. Pure: no framework imports, no IO (architecture.md
//! layering rule 1) — `infrastructure` implements these ports.

pub mod anki;
pub mod chunk;
pub mod repo;
pub mod settings;
pub mod source;
pub mod vault;
