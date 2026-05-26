fn main() {
    copy_pdfium_to_target_dirs();
    tauri_build::build()
}

fn copy_pdfium_to_target_dirs() {
    let manifest_dir = match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(value) => std::path::PathBuf::from(value),
        Err(_) => return,
    };
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let lib_name = match target_os.as_str() {
        "windows" => "pdfium.dll",
        "macos" => "libpdfium.dylib",
        "linux" => "libpdfium.so",
        _ => return,
    };
    let source = manifest_dir.join("resources").join(lib_name);
    if !source.exists() {
        return;
    }

    let out_dir = match std::env::var("OUT_DIR") {
        Ok(value) => std::path::PathBuf::from(value),
        Err(_) => return,
    };
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    for ancestor in out_dir.ancestors() {
        if ancestor.file_name().and_then(|name| name.to_str()) == Some(profile.as_str()) {
            let _ = std::fs::copy(&source, ancestor.join(lib_name));
            break;
        }
    }
}
