//! Keeps include/rugi/rugi.h in sync with the Rust source.

use std::path::PathBuf;

#[test]
fn header_is_up_to_date() {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config = cbindgen::Config::from_file(crate_dir.join("cbindgen.toml")).expect("cbindgen.toml");
    let bindings =
        cbindgen::Builder::new().with_crate(&crate_dir).with_config(config).generate().expect("cbindgen generation");
    let mut generated = Vec::new();
    bindings.write(&mut generated);
    let header = crate_dir.join("../../include/rugi/rugi.h");
    if std::env::var_os("RUGI_BLESS_HEADER").is_some() {
        std::fs::write(&header, &generated).expect("write header");
        return;
    }
    let current = std::fs::read(&header).unwrap_or_default();
    assert!(
        current == generated,
        "include/rugi/rugi.h is stale. Regenerate with: RUGI_BLESS_HEADER=1 cargo test -p rugi-ffi header"
    );
}
