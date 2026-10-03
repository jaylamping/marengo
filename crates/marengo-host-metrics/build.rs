fn main() {
    let sha = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=MARENGO_GIT_SHA={sha}");
    // The SHA changes on every commit without touching crate sources, so watch
    // the git HEAD (and its target ref) explicitly — otherwise incremental
    // builds keep reporting a stale SHA (L-marengo-host-metrics-05).
    // `--absolute-git-dir` also resolves worktrees, where `.git` is a pointer
    // file rather than a directory.
    if let Some(git_dir) = git_dir() {
        let head = git_dir.join("HEAD");
        println!("cargo:rerun-if-changed={}", head.display());
        if let Ok(target) = std::fs::read_to_string(&head) {
            if let Some(refname) = target.strip_prefix("ref:").map(str::trim) {
                let target = git_dir.join(refname);
                if target.is_file() {
                    println!("cargo:rerun-if-changed={}", target.display());
                } else {
                    let packed = git_dir.join("packed-refs");
                    if packed.is_file() {
                        println!("cargo:rerun-if-changed={}", packed.display());
                    }
                }
            }
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}

fn git_dir() -> Option<std::path::PathBuf> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--absolute-git-dir"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(std::path::PathBuf::from(
        String::from_utf8_lossy(&out.stdout).trim(),
    ))
}
