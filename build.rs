#[cfg(windows)]
fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");
    let mut resources = winresource::WindowsResource::new();
    resources
        .set_icon("assets/app.ico")
        .set("ProductName", "Zaaps Dofus 3")
        .set(
            "FileDescription",
            "Zaaps Dofus 3 - utilitaire de navigation",
        )
        .set("OriginalFilename", "Zaaps Dofus 3.exe")
        .set("InternalName", "zaaps-dofus3")
        .set("FileVersion", env!("CARGO_PKG_VERSION"))
        .set("ProductVersion", env!("CARGO_PKG_VERSION"));
    resources
        .compile()
        .expect("Impossible d'integrer l'icone Windows");
}

#[cfg(not(windows))]
fn main() {}
