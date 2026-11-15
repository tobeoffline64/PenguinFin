//! Windows: embed the app icon and version information in penguinfin.exe.
fn main() {
    println!("cargo:rerun-if-changed=assets/icon/penguinfin.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon/penguinfin.ico")
            .set("ProductName", "PenguinFin")
            .set("FileDescription", "PenguinFin - offline personal finance planner")
            .set("LegalCopyright", "MIT License");
        if let Err(e) = res.compile() {
            // Don't fail the build over the icon (e.g. cross-compiling without a resource compiler).
            println!("cargo:warning=could not embed Windows icon: {e}");
        }
    }
}
