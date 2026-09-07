//! Infrastructure: adapters implementing the domain ports against the real
//! world (filesystem, network, database).

pub mod anki_connect;
pub mod card_pg;
pub mod fsrs;
pub mod index_repo;
pub mod pg;
pub mod queue_pg;
pub mod session_pg;
pub mod settings_pg;
pub mod specta;
pub mod vault_fs;
