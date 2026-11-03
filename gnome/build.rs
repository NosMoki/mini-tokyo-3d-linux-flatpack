//! Compiles the GSettings schema so that `cargo run` works without installing
//! it. Installed builds use the schema installed next to the app instead.

use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

const SCHEMA: &str = "data/io.github.nosmoki.MiniTokyo3D.gschema.xml";

fn main() {
    println!("cargo:rerun-if-changed={SCHEMA}");
    let dir = PathBuf::from(env::var("OUT_DIR").unwrap()).join("schemas");
    fs::create_dir_all(&dir).unwrap();
    fs::copy(SCHEMA, dir.join("io.github.nosmoki.MiniTokyo3D.gschema.xml")).unwrap();

    match Command::new("glib-compile-schemas").arg("--strict").arg(&dir).status() {
        Ok(status) if status.success() => {}
        Ok(status) => panic!("glib-compile-schemas failed: {status}"),
        Err(error) => {
            println!("cargo:warning=glib-compile-schemas not found ({error}); `cargo run` needs the schema installed")
        }
    }
    println!("cargo:rustc-env=MT3D_SCHEMA_DIR={}", dir.display());
}
