use std::process::Command;
use std::{env, fs, io::Write, path::Path};

/// Generates a compile-time icon lookup table from vendored Nerd Font glyphnames.json.
/// Source: <https://github.com/ryanoasis/nerd-fonts/blob/master/glyphnames.json>
fn main() {
    build_icons();
    build_test_plugins();
}

fn build_icons() {
    println!("cargo::rerun-if-changed=resources/glyphnames.json");

    let output = Path::new(&env::var("OUT_DIR").unwrap()).join("icons.rs");
    let mut file = std::io::BufWriter::new(fs::File::create(output).unwrap());

    let json: serde_json::Value =
        serde_json::from_str(include_str!("resources/glyphnames.json")).unwrap();

    let mut map = phf_codegen::Map::new();
    for (name, val) in json.as_object().unwrap() {
        if name == "METADATA" {
            continue;
        }
        if let Some(ch) = val["char"].as_str() {
            map.entry(name.as_str(), &format!("\"{ch}\""));
        }
    }

    writeln!(
        file,
        "static ICONS: phf::Map<&'static str, &'static str> = {};",
        map.build()
    )
    .unwrap();
}

fn build_test_plugins() {
    println!("cargo::rerun-if-changed=../../plugins/test-harness/src");
    println!("cargo::rerun-if-changed=../../plugins/test-harness/Cargo.toml");
    println!("cargo::rerun-if-changed=../../plugins/nodejs/src");
    println!("cargo::rerun-if-changed=../../plugins/nodejs/Cargo.toml");

    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

    // Use a separate target dir to avoid deadlocking on the parent cargo's
    // target directory lock (https://github.com/rust-lang/cargo/issues/6412).
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let wasm_target_dir = workspace_root.join("target/wasm-plugins");
    let wasm_release_dir = wasm_target_dir.join("wasm32-unknown-unknown/release");

    println!(
        "cargo::rustc-env=WASM_PLUGIN_DIR={}",
        wasm_release_dir.display()
    );

    for plugin in ["starship-plugin-test-harness", "starship-plugin-nodejs"] {
        let status = Command::new(&cargo)
            .args([
                "build",
                "-p",
                plugin,
                "--target",
                "wasm32-unknown-unknown",
                "--release",
                "--target-dir",
            ])
            .arg(&wasm_target_dir)
            .env_remove("CARGO_MAKEFLAGS")
            .env_remove("MAKEFLAGS")
            .env_remove("MFLAGS")
            .env_remove("RUSTFLAGS")
            .status()
            .unwrap_or_else(|e| panic!("failed to run cargo build for {plugin}: {e}"));

        assert!(
            status.success(),
            "failed to compile {plugin} to wasm32-unknown-unknown"
        );
    }
}
