use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    compile_blueprints();
    bundle_js();

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set by cargo");
    let out_ui = format!("{out_dir}/ui");

    glib_build_tools::compile_resources(
        &[&out_ui, "data/ui", "data"],
        "data/karere.gresource.xml",
        "karere.gresource",
    );

    if let Ok(cef_path) = env::var("CEF_PATH") {
        let p = PathBuf::from(&cef_path);
        println!("cargo:rustc-link-search=native={}", p.display());
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", p.display());
    }

    println!("cargo:rerun-if-env-changed=CEF_PATH");
    println!("cargo:rerun-if-changed=data/karere.gresource.xml");
}

/// Concatenate `data/js/*.js` (lexical order, so `00-bootstrap.js` runs first) into
/// `$OUT_DIR/injected_bundle.js`, embedded by the renderer via `include_str!`.
fn bundle_js() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set by cargo");
    let js_dir = PathBuf::from("data/js");

    // Re-bundle on add/remove ...
    println!("cargo:rerun-if-changed=data/js");

    let mut files: Vec<PathBuf> = std::fs::read_dir(&js_dir)
        .unwrap_or_else(|e| panic!("failed to read data/js: {e}"))
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("js"))
        .collect();
    files.sort();

    let mut bundle = String::new();
    for path in &files {
        // ... and on per-file edits.
        println!("cargo:rerun-if-changed={}", path.display());

        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .expect("js file must have a UTF-8 name");
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if !has_order_prefix(stem) {
            println!(
                "cargo:warning=data/js/{name} is missing the recommended `NN-name.js` \
                 prefix; bundle ordering relative to other scripts is undefined"
            );
        }

        let contents = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        bundle.push_str(&format!("\n// ===== {name} =====\n"));
        bundle.push_str(&contents);
        bundle.push('\n');
    }

    let out_file = PathBuf::from(&out_dir).join("injected_bundle.js");
    std::fs::write(&out_file, bundle)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", out_file.display()));
}

/// True when `stem` starts with `NN-` (two digits + hyphen).
fn has_order_prefix(stem: &str) -> bool {
    let b = stem.as_bytes();
    b.len() >= 3 && b[0].is_ascii_digit() && b[1].is_ascii_digit() && b[2] == b'-'
}

fn compile_blueprints() {
    let probe = Command::new("blueprint-compiler").arg("--version").output();
    let ok = matches!(&probe, Ok(o) if o.status.success());
    if !ok {
        panic!(
            "blueprint-compiler not found on PATH. Install with one of:\n  \
             dnf install blueprint-compiler\n  \
             apt install blueprint-compiler\n  \
             flatpak run --command=blueprint-compiler org.gnome.Sdk//50\n\
             (the GNOME 50 SDK extension already ships blueprint-compiler for flatpak builds), \
             then rerun cargo build."
        );
    }

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set by cargo");
    let out_ui = PathBuf::from(&out_dir).join("ui");
    std::fs::create_dir_all(&out_ui).expect("failed to create $OUT_DIR/ui");

    let blp_dir = PathBuf::from("data/ui");
    let entries = std::fs::read_dir(&blp_dir).expect("data/ui must exist");
    for entry in entries {
        let entry = entry.expect("failed to read data/ui entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("blp") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("blueprint file must have a UTF-8 stem");
        let out_file = out_ui.join(format!("{stem}.ui"));

        println!("cargo:rerun-if-changed={}", path.display());

        let result = Command::new("blueprint-compiler")
            .arg("compile")
            .arg("--output")
            .arg(&out_file)
            .arg(&path)
            .output()
            .expect("failed to spawn blueprint-compiler");
        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            let stdout = String::from_utf8_lossy(&result.stdout);
            panic!(
                "blueprint-compiler failed for {}:\nstdout: {}\nstderr: {}",
                path.display(),
                stdout,
                stderr
            );
        }
    }
}
