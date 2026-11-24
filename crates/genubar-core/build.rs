use std::env;
use std::fs;
use std::path::PathBuf;

const EXTENSION_METADATA_REL_PATH: &str = "../../genubar-gnome-extension/metadata.json";

fn main() {
    println!("cargo:rerun-if-changed={EXTENSION_METADATA_REL_PATH}");

    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set by Cargo"),
    );
    let metadata_path = manifest_dir.join(EXTENSION_METADATA_REL_PATH);

    let raw = fs::read_to_string(&metadata_path).unwrap_or_else(|err| {
        panic!(
            "Failed to read {} ({err}). Initialize the submodule with `git submodule update --init`.",
            metadata_path.display()
        )
    });

    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .unwrap_or_else(|err| panic!("Failed to parse {} as JSON: {err}", metadata_path.display()));

    let version = parsed
        .get("genubar-version")
        .or_else(|| parsed.get("version-name"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            panic!(
                "Missing non-empty 'genubar-version' or 'version-name' in {}",
                metadata_path.display()
            )
        });

    println!("cargo:rustc-env=GENUBAR_EXPECTED_EXTENSION_VERSION={version}");
}
