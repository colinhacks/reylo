//! What every database's analysis engine shares. A host hands in one JSON request and gets one
//! JSON response back ([`api::call`]); everything that touches a database happens on the host's
//! side, so the engine is pure, and a host that holds no connection of its own answers its needs
//! through the clients `reylo-engine` carries (`engine/facade`, `engine/native`).
//!
//! Each database is a crate of its own over this one (`reylo-engine-postgres`,
//! `reylo-engine-sqlite`, `reylo-engine-mysql`), which implements [`database::Database`], and each
//! is built to a WebAssembly component of its own, with the program that connects it, by
//! `engine/wasip2`. A native host links the database crates it needs and calls their own entries.
//! Core never names a database's parser: placeholders are lexed here for every dialect, and
//! everything that reads a statement's structure is its database's.

pub mod analysis;
pub mod api;
pub mod config;
pub mod database;
pub mod dialect;
pub mod errors;
pub mod fingerprint;
pub mod fresh;
pub mod host;
pub mod lex;
pub mod replay;
pub mod schema;
pub mod schema_type;
pub mod shape;
pub mod stack;
pub mod structured;
pub mod types;
