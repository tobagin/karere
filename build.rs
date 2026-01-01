use std::process::Command;

fn main() {
    // Get OUT_DIR
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = std::path::Path::new(&out_dir);
    let ui_out = out_path.join("ui");
    std::fs::create_dir_all(&ui_out).expect("Failed to create ui output dir");

    // Compile Blueprint files
    let status = Command::new("blueprint-compiler")
        .args(&["compile", "data/ui/window.blp", "--output", out_path.join("ui/window.ui").to_str().unwrap()])
        .status()
        .expect("Failed to run blueprint-compiler (window)");

    let status_help = Command::new("blueprint-compiler")
        .args(&["compile", "data/ui/keyboard-shortcuts.blp", "--output", out_path.join("ui/keyboard-shortcuts.ui").to_str().unwrap()])
        .status()
        .expect("Failed to run blueprint-compiler (keyboard-shortcuts)");

    let status_pref = Command::new("blueprint-compiler")
        .args(&["compile", "data/ui/preferences.blp", "--output", out_path.join("ui/preferences.ui").to_str().unwrap()])
        .status()
        .expect("Failed to run blueprint-compiler (preferences)");

    if !status.success() || !status_help.success() || !status_pref.success() {
        panic!("blueprint-compiler failed");
    }

    // Compile GResources
    // We strictly search "data" AND "OUT_DIR" so it finds `ui/window.ui` in `OUT_DIR`
    glib_build_tools::compile_resources(
        &["data", &out_dir],
        "data/resources.gresource.xml",
        "karere.gresource",
    );

    // Compile GSchemas
    let status = Command::new("glib-compile-schemas")
        .arg("data")
        .status()
        .expect("Failed to run glib-compile-schemas");
    
    if !status.success() {
        panic!("glib-compile-schemas failed");
    }

    // Compile translations
    let status = Command::new("msgfmt")
        .arg("--version")
        .status();

    if let Ok(status) = status {
        if status.success() {
            let po_dir = std::path::Path::new("po");
            let locale_dir = out_path.join("locale");
            std::fs::create_dir_all(&locale_dir).expect("Failed to create locale dir");

            for entry in std::fs::read_dir(po_dir).expect("Failed to read po dir") {
                let entry = entry.expect("Failed to read po entry");
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("po") {
                    let lang = path.file_stem().unwrap().to_str().unwrap();
                    let lang_dir = locale_dir.join(lang).join("LC_MESSAGES");
                    std::fs::create_dir_all(&lang_dir).expect("Failed to create lang dir");
                    
                    let status = Command::new("msgfmt")
                        .arg("-o")
                        .arg(lang_dir.join("karere.mo"))
                        .arg(&path)
                        .status()
                        .expect("Failed to run msgfmt");

                    if !status.success() {
                        panic!("msgfmt failed for {}", lang);
                    }
                    println!("cargo:rerun-if-changed={}", path.display());
                }
            }
        }
    }
}
