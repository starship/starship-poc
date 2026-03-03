use std::{env, fs, io::Write, path::Path};

/// Generates a compile-time icon lookup table from vendored Nerd Font glyphnames.json.
/// Source: <https://github.com/ryanoasis/nerd-fonts/blob/master/glyphnames.json>
fn main() {
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

    println!("cargo::rerun-if-changed=resources/glyphnames.json");
}
