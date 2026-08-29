use crate::{Artifact, InstallState, Manifest, UpdateError};

#[derive(Debug, Clone)]
pub enum FetchPlan {
    Nothing,
    Delta { from: String, artifact: Artifact },
    Full { artifact: Artifact },
}

pub fn plan_fetch(local: &InstallState, remote: &Manifest) -> Result<FetchPlan, UpdateError> {
    if local.target != remote.target {
        return Err(UpdateError::TargetMismatch);
    }
    if local.rewrite_version == remote.rewrite_version {
        return Ok(FetchPlan::Nothing);
    }
    if let Some(artifact) = remote.delta_from.get(&local.rewrite_version) {
        return Ok(FetchPlan::Delta {
            from: local.rewrite_version.clone(),
            artifact: artifact.clone(),
        });
    }
    Ok(FetchPlan::Full {
        artifact: remote.full.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn remote(delta_from: &str) -> Manifest {
        let mut d = BTreeMap::new();
        if !delta_from.is_empty() {
            d.insert(
                delta_from.into(),
                Artifact {
                    name: "d.wocdelta".into(),
                    sha256: "00".into(),
                    size: 1,
                },
            );
        }
        Manifest {
            rewrite_version: "1.5.0".into(),
            protocol_rev: 6,
            target: "x86_64-unknown-linux-gnu".into(),
            files: vec![],
            full: Artifact {
                name: "full.tar.zst".into(),
                sha256: "aa".into(),
                size: 2,
            },
            delta_from: d,
            sig: String::new(),
        }
    }

    #[test]
    fn same_version_is_nothing() {
        let local = InstallState {
            rewrite_version: "1.5.0".into(),
            target: "x86_64-unknown-linux-gnu".into(),
        };
        assert!(matches!(
            plan_fetch(&local, &remote("1.4.0")).unwrap(),
            FetchPlan::Nothing
        ));
    }

    #[test]
    fn predecessor_uses_delta() {
        let local = InstallState {
            rewrite_version: "1.4.0".into(),
            target: "x86_64-unknown-linux-gnu".into(),
        };
        match plan_fetch(&local, &remote("1.4.0")).unwrap() {
            FetchPlan::Delta { from, .. } => assert_eq!(from, "1.4.0"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn skip_version_uses_full() {
        let local = InstallState {
            rewrite_version: "1.3.0".into(),
            target: "x86_64-unknown-linux-gnu".into(),
        };
        assert!(matches!(
            plan_fetch(&local, &remote("1.4.0")).unwrap(),
            FetchPlan::Full { .. }
        ));
    }

    #[test]
    fn target_mismatch_errors() {
        let local = InstallState {
            rewrite_version: "1.4.0".into(),
            target: "aarch64-unknown-linux-gnu".into(),
        };
        assert!(matches!(
            plan_fetch(&local, &remote("1.4.0")),
            Err(UpdateError::TargetMismatch)
        ));
    }
}
