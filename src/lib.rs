//! nvmise: nvm's commands, with mise keeping the Node versions.
//!
//! Everything nvm does comes from the `nvmimic` library. This crate adds two
//! things to it: [`NVMISE`], the name the program goes by, and
//! [`MiseBackend`], which tells the library where mise keeps Node and asks
//! mise to install and remove versions.

pub mod backend;
pub mod identity;

pub use backend::MiseBackend;
pub use identity::NVMISE;
