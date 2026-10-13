//! nvmise: nvm's commands, with mise keeping the Node versions.
//!
//! Everything nvm does comes from the `nvmimic` library. This crate adds
//! [`NVMISE`], the name the program goes by.

pub mod identity;

pub use identity::NVMISE;
