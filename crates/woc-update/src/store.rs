use crate::UpdateError;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

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

pub struct HttpStore {
    pub base: String,
    agent: ureq::Agent,
}

impl HttpStore {
    pub fn new(base: String) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(3))
            .timeout_read(Duration::from_secs(120))
            .build();
        Self { base, agent }
    }

    pub fn agent() -> ureq::Agent {
        ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(3))
            .timeout_read(Duration::from_secs(120))
            .build()
    }
}

impl ArtifactStore for HttpStore {
    fn fetch(&self, name: &str) -> Result<Vec<u8>, UpdateError> {
        let url = if self.base.ends_with('/') {
            format!("{base}{name}", base = self.base)
        } else {
            format!("{}/{}", self.base, name)
        };
        let resp = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| UpdateError::Msg(e.to_string()))?;
        let mut body = Vec::new();
        resp.into_reader().read_to_end(&mut body)?;
        Ok(body)
    }
}

pub fn fetch_url(url: &str) -> Result<Vec<u8>, UpdateError> {
    let resp = HttpStore::agent()
        .get(url)
        .call()
        .map_err(|e| UpdateError::Msg(e.to_string()))?;
    let mut body = Vec::new();
    resp.into_reader().read_to_end(&mut body)?;
    Ok(body)
}

pub fn url_parent(url: &str) -> Option<String> {
    let (base, _) = url.rsplit_once('/')?;
    if base.is_empty() {
        None
    } else {
        Some(base.to_string())
    }
}
