use std::path::Path;

/// The contents of `.deploy-rev`, split into the installed SHA and timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDeployRev {
    pub sha: String,
    pub deployed_at: Option<String>,
}

/// Parse `SHA` or `SHA ISO8601` as written by the deploy scripts.
pub fn parse_deploy_rev(raw: &str) -> ParsedDeployRev {
    let cleaned = raw
        .trim()
        .replace("\\n", "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut parts = cleaned.split(' ');
    let rev = match parts.next() {
        Some(value) => value.to_string(),
        None => String::new(),
    };
    if rev.len() >= 7 && rev.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        let rest: Vec<&str> = parts.collect();
        return ParsedDeployRev {
            sha: rev,
            deployed_at: (!rest.is_empty()).then(|| rest.join(" ")),
        };
    }
    ParsedDeployRev {
        sha: cleaned,
        deployed_at: None,
    }
}

/// Return whether two full or abbreviated SHAs identify the same revision.
pub fn shas_match(installed: &str, upstream: &str) -> bool {
    let a = installed.trim().to_ascii_lowercase();
    let b = upstream.trim().to_ascii_lowercase();
    if a.is_empty() || b.is_empty() {
        return false;
    }
    if a.len() >= 7 && b.len() >= 7 {
        return a == b || a.starts_with(&b) || b.starts_with(&a);
    }
    a == b
}

/// Read and parse `.deploy-rev`, returning an empty revision when unavailable.
pub fn read_deploy_rev(path: &Path) -> ParsedDeployRev {
    match std::fs::read_to_string(path) {
        Ok(raw) => parse_deploy_rev(&raw),
        Err(_) => ParsedDeployRev {
            sha: String::new(),
            deployed_at: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_deploy_rev_sha_and_timestamp() {
        let parsed = parse_deploy_rev("abcdef0123456789 2026-08-11T12:00:00Z\n");
        assert_eq!(parsed.sha, "abcdef0123456789");
        assert_eq!(parsed.deployed_at.as_deref(), Some("2026-08-11T12:00:00Z"));
    }

    #[test]
    fn shas_match_prefix_and_full() {
        assert!(shas_match(
            "abcdef0123456789abcdef0123456789abcdef01",
            "abcdef0"
        ));
        assert!(shas_match(
            "abcdef0",
            "abcdef0123456789abcdef0123456789abcdef01"
        ));
        assert!(!shas_match("aaaaaaa", "bbbbbbb"));
        assert!(!shas_match("", "abcdef0"));
    }

    #[test]
    fn read_deploy_rev_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".deploy-rev");
        let mut file = std::fs::File::create(&path).expect("create");
        writeln!(file, "abc1234 2026-08-11T01:02:03Z").expect("write");
        let parsed = read_deploy_rev(&path);
        assert_eq!(parsed.sha, "abc1234");
    }
}
