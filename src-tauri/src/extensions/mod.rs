//! Desktop services for the extensions workspace (Skills / MCP).
//!
//! [`store`] owns persistence, [`paths`] resolves client paths from the
//! environment, [`discovery`] observes what already exists, [`sources`]
//! resolves import candidates, [`secrets`] talks to the system credential
//! store, and [`checks`] verifies MCP definitions. The Tauri commands in
//! `commands::extensions` orchestrate these services; real client files are
//! only ever written through `asb_switch::extensions`.

pub mod checks;
pub mod discovery;
pub mod mcp_registry;
pub mod migrate;
pub mod paths;
pub mod secrets;
pub mod skill_catalog;
pub mod sources;
pub mod store;
