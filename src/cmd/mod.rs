//! The command handlers `main` dispatches to, one module per area.

pub(crate) mod checks;
pub(crate) mod clean;
pub(crate) mod health;
pub(crate) mod hooks;
pub(crate) mod json;
pub(crate) mod sbom;
pub(crate) mod security;
pub(crate) mod updates;
pub(crate) mod workflow;
