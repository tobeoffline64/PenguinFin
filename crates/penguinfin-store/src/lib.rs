//! PenguinFin's storage: the SQLite database, its migrations, backups and exports.
pub mod db;
pub mod export;

// The engine's modules, so storage code can name them as `crate::model` etc.
use penguinfin_core::{debt, fred, model};
#[cfg(test)]
use penguinfin_core::sample;
