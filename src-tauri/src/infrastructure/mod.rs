//! Infrastructure: adapters implementing the domain ports against the real
//! world (filesystem, network, database).

pub mod anki_connect;
pub mod index_repo;
pub mod pg;
pub mod settings_pg;
pub mod specta;
pub mod vault_fs;
