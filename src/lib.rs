//! nvmise: nvm's commands, with mise keeping the Node versions.
//!
//! Everything nvm does comes from the `nvmimic` library. This crate adds
//! three things to it: [`NVMISE`], the name the program goes by,
//! [`MiseBackend`], which tells the library where mise keeps Node and asks
//! mise to install and remove versions, and [`MiseSteps`], nvmise's own part
//! of setting itself up and removing itself.

pub mod backend;
pub mod identity;
pub mod setup;

pub use backend::MiseBackend;
pub use identity::NVMISE;
pub use setup::MiseSteps;
