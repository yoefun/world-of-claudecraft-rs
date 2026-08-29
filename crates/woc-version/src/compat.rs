//! Client / realm rewrite + protocol compatibility.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

pub fn parse_semver(s: &str) -> Option<SemVer> {
    let core = s.split('-').next().unwrap_or(s).trim();
    if core.is_empty() {
        return None;
    }
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(SemVer {
        major,
        minor,
        patch,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientIdentity {
    pub rewrite_version: String,
    pub protocol_rev: u32,
}

impl ClientIdentity {
    pub fn from_hello(protocol_rev: Option<u32>, rewrite_version: Option<&str>) -> Self {
        Self {
            protocol_rev: protocol_rev.unwrap_or(0),
            rewrite_version: rewrite_version.unwrap_or("(unknown)").to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealmIdentity {
    pub rewrite_version: String,
    pub protocol_rev: Option<u32>,
    pub min_client_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compat {
    Compatible,
    ClientTooOld { client: String, min_client: String },
    ProtocolMismatch { client_rev: u32, realm_rev: u32 },
    BadClientVersion(String),
    BadMinVersion(String),
}

impl Compat {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Compatible)
    }

    pub fn user_message(&self) -> String {
        match self {
            Self::Compatible => "compatible".into(),
            Self::ClientTooOld { client, min_client } => {
                format!("version: update required (client {client} < min {min_client})")
            }
            Self::ProtocolMismatch {
                client_rev,
                realm_rev,
            } if client_rev < realm_rev => {
                format!("version: update required (protocol {client_rev} < {realm_rev})")
            }
            Self::ProtocolMismatch {
                client_rev,
                realm_rev,
            } => {
                format!("version: realm outdated (protocol {client_rev} > {realm_rev})")
            }
            Self::BadClientVersion(s) | Self::BadMinVersion(s) => {
                format!("version: invalid version string ({s})")
            }
        }
    }
}

pub fn check_compat(client: &ClientIdentity, realm: &RealmIdentity) -> Compat {
    let Some(_) = parse_semver(&client.rewrite_version) else {
        return Compat::BadClientVersion(client.rewrite_version.clone());
    };
    let Some(min) = parse_semver(&realm.min_client_version) else {
        return Compat::BadMinVersion(realm.min_client_version.clone());
    };
    if let Some(realm_rev) = realm.protocol_rev {
        if client.protocol_rev != realm_rev {
            return Compat::ProtocolMismatch {
                client_rev: client.protocol_rev,
                realm_rev,
            };
        }
    }
    let client_sem = parse_semver(&client.rewrite_version).expect("checked");
    if client_sem < min {
        return Compat::ClientTooOld {
            client: client.rewrite_version.clone(),
            min_client: realm.min_client_version.clone(),
        };
    }
    Compat::Compatible
}

pub fn min_client_version_from_env() -> String {
    std::env::var("WOC_MIN_CLIENT_VERSION")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| crate::REWRITE_VERSION.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn realm(min: &str, proto: Option<u32>) -> RealmIdentity {
        RealmIdentity {
            rewrite_version: "1.4.0".into(),
            protocol_rev: proto,
            min_client_version: min.into(),
        }
    }

    fn client(ver: &str, proto: u32) -> ClientIdentity {
        ClientIdentity {
            rewrite_version: ver.into(),
            protocol_rev: proto,
        }
    }

    #[test]
    fn parse_strips_prerelease_and_compares_triples() {
        assert_eq!(
            parse_semver("1.4.0"),
            Some(SemVer {
                major: 1,
                minor: 4,
                patch: 0
            })
        );
        assert_eq!(parse_semver("1.4.0-pre"), parse_semver("1.4.0"));
        assert!(parse_semver("1.3.0") < parse_semver("1.4.0"));
        assert!(parse_semver("(unknown)").is_none());
        assert!(parse_semver("").is_none());
        assert!(parse_semver("nope").is_none());
    }

    #[test]
    fn equal_client_and_min_with_matching_protocol_is_compatible() {
        let c = check_compat(&client("1.4.0", 6), &realm("1.4.0", Some(6)));
        assert!(c.is_ok());
    }

    #[test]
    fn newer_client_against_older_min_is_compatible() {
        let c = check_compat(&client("1.5.0", 6), &realm("1.4.0", Some(6)));
        assert!(c.is_ok());
    }

    #[test]
    fn prerelease_equals_release_triple() {
        let c = check_compat(&client("1.4.0-pre", 6), &realm("1.4.0", Some(6)));
        assert!(c.is_ok());
    }

    #[test]
    fn client_below_min_is_too_old() {
        let c = check_compat(&client("1.3.0", 6), &realm("1.4.0", Some(6)));
        assert_eq!(
            c,
            Compat::ClientTooOld {
                client: "1.3.0".into(),
                min_client: "1.4.0".into(),
            }
        );
        let msg = c.user_message();
        assert!(msg.starts_with("version:"));
        assert!(msg.contains("update required"));
        assert!(msg.contains("1.3.0"));
        assert!(msg.contains("1.4.0"));
    }

    #[test]
    fn protocol_client_behind_realm() {
        let c = check_compat(&client("1.4.0", 5), &realm("1.4.0", Some(6)));
        assert_eq!(
            c,
            Compat::ProtocolMismatch {
                client_rev: 5,
                realm_rev: 6,
            }
        );
        let msg = c.user_message();
        assert!(msg.starts_with("version:"));
        assert!(msg.contains("update required"));
        assert!(msg.contains("protocol 5 < 6"));
    }

    #[test]
    fn protocol_client_ahead_of_realm() {
        let c = check_compat(&client("1.4.0", 7), &realm("1.4.0", Some(6)));
        assert_eq!(
            c,
            Compat::ProtocolMismatch {
                client_rev: 7,
                realm_rev: 6,
            }
        );
        let msg = c.user_message();
        assert!(msg.starts_with("version:"));
        assert!(msg.contains("realm outdated"));
        assert!(msg.contains("protocol 7 > 6"));
    }

    #[test]
    fn missing_realm_protocol_skips_protocol_check() {
        let c = check_compat(&client("1.4.0", 6), &realm("1.4.0", None));
        assert!(c.is_ok());
    }

    #[test]
    fn missing_hello_identity_fails_closed() {
        let c = check_compat(
            &ClientIdentity::from_hello(None, None),
            &realm("1.4.0", Some(6)),
        );
        assert!(!c.is_ok());
        assert!(c.user_message().starts_with("version:"));
    }

    #[test]
    fn bad_min_version() {
        let c = check_compat(&client("1.4.0", 6), &realm("not-a-version", Some(6)));
        assert_eq!(c, Compat::BadMinVersion("not-a-version".into()));
        assert!(c.user_message().starts_with("version:"));
    }
}
