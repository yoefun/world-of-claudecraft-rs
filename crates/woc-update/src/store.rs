use crate::UpdateError;
use std::path::PathBuf;

pub trait ArtifactStore {
    fn fetch(&self, name: &str) -> Result<Vec<u8>, UpdateError>;
}

pub struct DirStore {
    pub root: PathBuf,
}

impl ArtifactStore for DirStore {
    fn fetch(&self, name: &str) -> Result<Vec<u8>, UpdateError> {
        std::fs::read(self.root.join(name)).map_err(UpdateError::from)
    }
}
