//! Caiman Bridge: import and migration from foreign hypervisors.

pub mod acquire;
pub mod qcow2;

pub use acquire::{acquire_disk, AcquireError, SshAuth, SshTarget};
pub use qcow2::{Qcow2Error, Qcow2Reader};
