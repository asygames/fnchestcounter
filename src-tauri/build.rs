fn main() {
    // FNCC_CHECK_ONLY=1 sare peste resursele Windows (iconiță/manifest) — doar pentru
    // `cargo check --target x86_64-pc-windows-gnu` de pe Linux, unde lipsește windres.
    println!("cargo:rerun-if-env-changed=FNCC_CHECK_ONLY");
    if std::env::var_os("FNCC_CHECK_ONLY").is_some() {
        println!("cargo:rustc-check-cfg=cfg(desktop)");
        println!("cargo:rustc-cfg=desktop");
        println!("cargo:rustc-check-cfg=cfg(mobile)");
        return;
    }
    tauri_build::build()
}
