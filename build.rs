// Embed the duck icon and version info into the Windows .exe.
fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_manifest(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1"><application>
    <supportedOS Id="{4a2f28e3-53b9-4441-ba9c-d69d4a4a6e38}"/>
    <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
  </application></compatibility>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3"><security><requestedPrivileges>
    <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
  </requestedPrivileges></security></trustInfo>
</assembly>"#,
        );
        res.set_icon("assets/app.ico")
            .set("ProductName", "Duck Pond")
            .set(
                "FileDescription",
                "Duck Pond: your desktop, as ducks on a Monet pond",
            )
            .set("CompanyName", "Duck Pond");
        // cross-compiling from macOS/Linux uses mingw's windres
        let cross = std::env::var("HOST")
            .map(|h| !h.contains("windows"))
            .unwrap_or(false);
        if cross {
            res.set_windres_path("x86_64-w64-mingw32-windres");
            res.set_ar_path("x86_64-w64-mingw32-ar");
        }
        match res.compile() {
            Ok(()) => {}
            // A cross *check* from macOS/Linux may have no resource compiler.
            Err(e) if cross => {
                println!("cargo:warning=Windows resources not embedded (cross build): {e}")
            }
            // On Windows the manifest is required (DPI + desktop integration).
            Err(e) => panic!("embed Windows compatibility manifest and icon: {e}"),
        }
    }
}
