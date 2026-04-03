use std::process::Command;
use std::time::Instant;
use std::{env, fs, io::Write, path::Path};

/// Generates a compile-time icon lookup table from vendored Nerd Font glyphnames.json.
/// Source: <https://github.com/ryanoasis/nerd-fonts/blob/master/glyphnames.json>
fn main() {
    eprintln!("[build.rs] starting");
    build_icons();
    eprintln!("[build.rs] icons done, building test plugins...");
    build_test_plugins();
    eprintln!("[build.rs] done");
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

    for plugin in ["starship-plugin-test-harness", "starship-plugin-nodejs"] {
        eprintln!("[build.rs] building {plugin}...");
        let t = Instant::now();
        let status = Command::new(&cargo)
            .args([
                "build",
                "-p",
                plugin,
                "--target",
                "wasm32-unknown-unknown",
                "--release",
                "-vv",
            ])
            .env_remove("CARGO_MAKEFLAGS")
            .env_remove("MAKEFLAGS")
            .status()
            .unwrap_or_else(|e| panic!("failed to run cargo build for {plugin}: {e}"));
        eprintln!(
            "[build.rs] {plugin} finished in {:.1}s",
            t.elapsed().as_secs_f64()
        );

        assert!(
            status.success(),
            "failed to compile {plugin} to wasm32-unknown-unknown"
        );
    }
}
