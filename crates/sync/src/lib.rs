//! Contextful — local-first company brain.
//!
//! This crate is a library of subsystems behind a thin CLI ([`bin/sync`]):
//!
//! | Subsystem | Module | Spec |
//! |---|---|---|
//! | Capability access control | [`access`] | 03 |
//! | Identity / principals | [`identity`] | 03 |
//! | Brain & memory + MCP | [`brain`] | 02, 06 |
//! | Connectors & ETL | [`connectors`], [`cron`] | 05 |
//! | Rooms & sync relay | [`sync`] | 01 |
//! | Sandbox & agents | [`sandbox`], [`agent`] | 04 |
//! | Control plane | [`controlplane`], [`config`] | 07 |
//! | Demo scenario | [`scenario`] | 00 |

pub mod access;
pub mod agent;
pub mod brain;
pub mod config;
pub mod connectors;
pub mod controlplane;
pub mod cron;
pub mod identity;
pub mod sandbox;
pub mod scenario;
pub mod store;
pub mod sync;
