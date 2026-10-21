//! Embeds the app icon in the Windows executables, `kiln.exe` and
//! `kilnw.exe` alike (a resource applies to every binary of the crate), so
//! Explorer, a taskbar pin and the Start menu show Kiln's mark. The `.ico`
//! is drawn here from `kiln-brand`'s geometry rather than read from the
//! committed `kiln.ico`, as Fenix's build does, so every build carries the
//! current icon and there is no copy to forget to update. Elsewhere it does
//! nothing: Linux executables carry no icon.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let out_dir = std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR for build scripts");
        let ico = std::path::Path::new(&out_dir).join("kiln.ico");
        std::fs::write(&ico, kiln_brand::ico_bytes()).expect("writing kiln.ico into OUT_DIR");
        winresource::WindowsResource::new()
            .set_icon(ico.to_str().expect("OUT_DIR is valid UTF-8"))
            .compile()
            .expect("embedding kiln.ico in the Windows executables' resources");
    }
}
