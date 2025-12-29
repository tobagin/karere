use std::process::Command;

fn main() {
    // Compile Blueprint files
    let status = Command::new("blueprint-compiler")
        .args(&["compile", "data/window.blp", "--output", "data/window.ui"])
        .status()
        .expect("Failed to run blueprint-compiler (window)");

    let status_help = Command::new("blueprint-compiler")
        .args(&["compile", "data/help-overlay.blp", "--output", "data/help-overlay.ui"])
        .status()
        .expect("Failed to run blueprint-compiler (help-overlay)");

    if !status.success() || !status_help.success() {
        panic!("blueprint-compiler failed");
    }

    // Compile GResources
    glib_build_tools::compile_resources(
        &["data"],
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
}
