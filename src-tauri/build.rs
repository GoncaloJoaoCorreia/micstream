use tauri_build::WindowsAttributes;

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    if target_os == "windows" && target_env == "msvc" {
        let manifest = std::env::current_dir()
            .unwrap()
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg=/MANIFESTINPUT:{}",
            manifest.display()
        );
        tauri_build::try_build(
            tauri_build::Attributes::new()
                .windows_attributes(WindowsAttributes::new_without_app_manifest()),
        )
        .expect("failed to run tauri-build");
    } else {
        tauri_build::build();
    }
}
