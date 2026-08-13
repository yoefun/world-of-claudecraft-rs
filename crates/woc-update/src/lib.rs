mod apply;
mod delta;
mod error;
mod hash;
mod manifest;
mod pack;
mod plan;
mod sign;
mod store;

pub use apply::apply_update;
pub use delta::{apply_delta, pack_delta, DeltaMeta, PatchEntry};
pub use error::UpdateError;
pub use hash::sha256_hex;
pub use manifest::{Artifact, FileEntry, InstallState, Manifest};
pub use pack::{file_entry, pack_full, unpack_full};
pub use plan::{plan_fetch, FetchPlan};
pub use sign::{
    sign_manifest, signing_key_from_hex, verify_manifest, verifying_key_from_hex,
};
pub use store::{ArtifactStore, DirStore};
