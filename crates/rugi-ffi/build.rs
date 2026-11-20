fn main() {
    // Make the macOS dylib relocatable: consumers find it through their rpath
    // instead of an absolute path into Cargo's target directory.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/librugi_ffi.dylib");
    }
}
