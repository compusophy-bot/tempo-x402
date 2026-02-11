fn main() {
    // If GIT_SHA is already set (e.g. Docker --build-arg), use it.
    // Otherwise, extract from git rev-parse HEAD (works on Railway source builds).
    let sha = std::env::var("GIT_SHA")
        .ok()
        .filter(|s| !s.is_empty() && s != "dev")
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
}
