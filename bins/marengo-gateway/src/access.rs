//! Gateway request access, separate from runtime motor admission.

use std::collections::HashSet;

use axum::http::{header, uri::Authority, HeaderMap, HeaderValue, StatusCode, Uri};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

const MAX_CREDENTIAL_BYTES: usize = 4096;
const ALL_CAPABILITIES: u8 = 0b1_1111;

#[derive(Clone, Copy)]
pub enum Capability {
    Control = 1,
    Calibration = 2,
    Configuration = 4,
    Management = 8,
    SensitiveRead = 16,
}

struct Grant {
    digest: [u8; 32],
    capabilities: u8,
}

pub struct AccessPolicy {
    grants: Vec<Grant>,
    origins: HashSet<String>,
}

impl Default for AccessPolicy {
    fn default() -> Self {
        Self {
            grants: Vec::new(),
            origins: ["http://localhost:5173", "http://127.0.0.1:5173"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    }
}

impl AccessPolicy {
    #[cfg(test)]
    pub(crate) fn operator_fixture(credential: &str) -> Result<Self, String> {
        let mut policy = Self::default();
        policy
            .add_credential(credential, ALL_CAPABILITIES)
            .map_err(|()| "invalid fixture credential".to_owned())?;
        Ok(policy)
    }

    /// Read trusted startup configuration, never request-supplied role claims.
    pub fn from_environment() -> Result<Self, String> {
        let mut policy = Self::default();
        for (name, capabilities) in [
            ("MARENGO_GATEWAY_OPERATOR_TOKEN", ALL_CAPABILITIES),
            // The historical token already authorized tuning/config/restart/deploy.
            // Preserve that administrative compatibility; new roles may be narrower.
            ("MARENGO_GATEWAY_LOG_TOKEN", ALL_CAPABILITIES),
            ("MARENGO_GATEWAY_CONTROL_TOKEN", Capability::Control as u8),
            (
                "MARENGO_GATEWAY_CALIBRATION_TOKEN",
                Capability::Calibration as u8,
            ),
            (
                "MARENGO_GATEWAY_CONFIG_TOKEN",
                Capability::Configuration as u8,
            ),
            (
                "MARENGO_GATEWAY_MANAGEMENT_TOKEN",
                Capability::Management as u8,
            ),
            (
                "MARENGO_GATEWAY_READ_TOKEN",
                Capability::SensitiveRead as u8,
            ),
        ] {
            match std::env::var(name) {
                Ok(value) if !value.trim().is_empty() => {
                    policy
                        .add_credential(value.trim(), capabilities)
                        .map_err(|()| format!("invalid {name}"))?;
                }
                Ok(_) | Err(std::env::VarError::NotPresent) => {}
                Err(_) => return Err(format!("invalid {name}")),
            }
        }
        match std::env::var("MARENGO_GATEWAY_ALLOWED_ORIGINS") {
            Ok(value) => {
                for origin in value.split(',').map(str::trim).filter(|v| !v.is_empty()) {
                    let origin = origin.trim_end_matches('/');
                    if valid_origin(origin).is_none() {
                        return Err("invalid gateway allowed Origin".into());
                    }
                    policy.origins.insert(origin.to_owned());
                }
            }
            Err(std::env::VarError::NotPresent) => {}
            Err(_) => return Err("invalid gateway allowed Origin".into()),
        }
        Ok(policy)
    }

    fn add_credential(&mut self, value: &str, capabilities: u8) -> Result<(), ()> {
        if value.is_empty()
            || value.len() > MAX_CREDENTIAL_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(());
        }
        self.grants.push(Grant {
            digest: Sha256::digest(value.as_bytes()).into(),
            capabilities,
        });
        Ok(())
    }

    pub fn has_credentials(&self) -> bool {
        !self.grants.is_empty()
    }

    pub fn allows_origin(&self, origin: &HeaderValue, headers: &HeaderMap) -> bool {
        let Ok(value) = origin.to_str() else {
            return false;
        };
        let Some(uri) = valid_origin(value) else {
            return false;
        };
        if self.origins.contains(value) {
            return true;
        }
        let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else {
            return false;
        };
        let Ok(host) = host.parse::<Authority>() else {
            return false;
        };
        // Same-origin robot-hosted Consul. A browser supplies this authority when
        // targeting the gateway; it cannot substitute another Host in fetch.
        uri.authority()
            .is_some_and(|authority| authority.as_str().eq_ignore_ascii_case(host.as_str()))
    }

    pub fn validate_origin(&self, headers: &HeaderMap) -> Result<(), StatusCode> {
        let mut values = headers.get_all(header::ORIGIN).iter();
        let Some(origin) = values.next() else {
            return Ok(());
        };
        if values.next().is_some() || !self.allows_origin(origin, headers) {
            return Err(StatusCode::FORBIDDEN);
        }
        Ok(())
    }

    pub fn authorize(&self, headers: &HeaderMap, capability: Capability) -> Result<(), StatusCode> {
        self.validate_origin(headers)?;
        let capabilities = self.authenticate(headers)?;
        if capabilities & capability as u8 == 0 {
            return Err(StatusCode::FORBIDDEN);
        }
        Ok(())
    }

    fn authenticate(&self, headers: &HeaderMap) -> Result<u8, StatusCode> {
        let authorization = single_header(headers, header::AUTHORIZATION.as_str())?;
        let legacy = single_header(headers, "x-marengo-log-token")?;
        let credential = match (authorization, legacy) {
            (Some(value), None) => value.strip_prefix("Bearer "),
            (None, Some(value)) => Some(value),
            _ => None,
        }
        .ok_or(StatusCode::UNAUTHORIZED)?;
        if credential.is_empty() || credential.len() > MAX_CREDENTIAL_BYTES {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let digest: [u8; 32] = Sha256::digest(credential.as_bytes()).into();
        let mut capabilities = 0;
        for grant in &self.grants {
            let matched = digest.ct_eq(&grant.digest).unwrap_u8();
            capabilities |= grant.capabilities & 0u8.wrapping_sub(matched);
        }
        if capabilities == 0 {
            Err(StatusCode::UNAUTHORIZED)
        } else {
            Ok(capabilities)
        }
    }
}

fn single_header<'a>(headers: &'a HeaderMap, name: &str) -> Result<Option<&'a str>, StatusCode> {
    let mut values = headers.get_all(name).iter();
    let value = values
        .next()
        .map(|v| v.to_str().map_err(|_| StatusCode::UNAUTHORIZED))
        .transpose()?;
    if values.next().is_some() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(value)
}

fn valid_origin(value: &str) -> Option<Uri> {
    let uri = value.parse::<Uri>().ok()?;
    if !matches!(uri.scheme_str(), Some("http" | "https"))
        || uri.authority()?.as_str().contains('@')
        || uri.query().is_some()
        || !matches!(uri.path(), "" | "/")
    {
        return None;
    }
    Some(uri)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    fn bearer(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {value}")).expect("fixture header"),
        );
        headers
    }

    #[test]
    fn unset_missing_wrong_and_ambiguous_credentials_fail_closed() {
        let empty = AccessPolicy::default();
        assert_eq!(
            empty.authorize(&bearer("fixture"), Capability::Control),
            Err(StatusCode::UNAUTHORIZED)
        );
        let mut policy = AccessPolicy::default();
        policy
            .add_credential("fixture", ALL_CAPABILITIES)
            .expect("fixture grant");
        assert_eq!(
            policy.authorize(&HeaderMap::new(), Capability::Control),
            Err(StatusCode::UNAUTHORIZED)
        );
        assert_eq!(
            policy.authorize(&bearer("wrong"), Capability::Control),
            Err(StatusCode::UNAUTHORIZED)
        );
        let mut ambiguous = bearer("fixture");
        ambiguous.insert("x-marengo-log-token", HeaderValue::from_static("fixture"));
        assert_eq!(
            policy.authorize(&ambiguous, Capability::Control),
            Err(StatusCode::UNAUTHORIZED)
        );
        let mut duplicate = bearer("fixture");
        duplicate.append(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer fixture"),
        );
        assert_eq!(
            policy.authorize(&duplicate, Capability::Control),
            Err(StatusCode::UNAUTHORIZED)
        );
    }

    #[test]
    fn role_credentials_cannot_gain_another_capability() {
        let mut policy = AccessPolicy::default();
        policy
            .add_credential("read-fixture", Capability::SensitiveRead as u8)
            .expect("read grant");
        policy
            .add_credential("control-fixture", Capability::Control as u8)
            .expect("control grant");
        assert_eq!(
            policy.authorize(&bearer("read-fixture"), Capability::Control),
            Err(StatusCode::FORBIDDEN)
        );
        assert_eq!(
            policy.authorize(&bearer("control-fixture"), Capability::Management),
            Err(StatusCode::FORBIDDEN)
        );
        assert_eq!(
            policy.authorize(&bearer("read-fixture"), Capability::SensitiveRead),
            Ok(())
        );
        assert_eq!(
            policy.authorize(&bearer("control-fixture"), Capability::Control),
            Ok(())
        );
    }

    #[test]
    fn browser_origins_are_exact_and_non_browser_clients_remain_authenticated() {
        let mut policy = AccessPolicy::default();
        policy
            .add_credential("fixture", ALL_CAPABILITIES)
            .expect("grant");
        let mut headers = bearer("fixture");
        assert_eq!(policy.authorize(&headers, Capability::Control), Ok(()));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://localhost:5173"),
        );
        assert_eq!(policy.authorize(&headers, Capability::Control), Ok(()));
        for origin in [
            "https://foreign.example",
            "http://localhost:5173.foreign.example",
            "null",
            "http://user@localhost:5173",
        ] {
            headers.insert(
                header::ORIGIN,
                HeaderValue::from_str(origin).expect("origin"),
            );
            assert_eq!(
                policy.authorize(&headers, Capability::Control),
                Err(StatusCode::FORBIDDEN)
            );
        }
        headers.insert(header::HOST, HeaderValue::from_static("marengo.local:8444"));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://marengo.local:8444"),
        );
        assert_eq!(policy.authorize(&headers, Capability::Control), Ok(()));
    }
}
