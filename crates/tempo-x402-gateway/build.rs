fn main() {
    // Priority: GIT_SHA (Docker build-arg) > RAILWAY_GIT_COMMIT_SHA (Railway built-in)
    // > git rev-parse HEAD (local dev) > "dev" (fallback)
    let sha = std::env::var("GIT_SHA")
        .ok()
        .filter(|s| !s.is_empty() && s != "dev")
        .or_else(|| {
            std::env::var("RAILWAY_GIT_COMMIT_SHA")
                .ok()
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            std::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        String::from_utf8(o.stdout)
                            .ok()
                            .map(|s| s.trim().to_string())
                    } else {
                        None
                    }
                })
        })
        .unwrap_or_else(|| "dev".to_string());

    println!("cargo:rustc-env=GIT_SHA={sha}");
    println!("cargo:rerun-if-env-changed=GIT_SHA");
    println!("cargo:rerun-if-env-changed=RAILWAY_GIT_COMMIT_SHA");
}
