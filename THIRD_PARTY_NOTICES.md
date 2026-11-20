# Third-party notices

Rugi is licensed under MIT OR Apache-2.0 (see `LICENSE-MIT` and
`LICENSE-APACHE`). It links permissively licensed open-source components. No third-party source code is vendored or copied into
the Rugi crates; dependencies are consumed unmodified from crates.io.

## Bundled assets

| Asset | Source | License | Obligations |
|---|---|---|---|
| Inter typeface (Regular/Medium/SemiBold, static instances at opsz 14 generated from the variable font with fontTools) | `google/fonts` `ofl/inter`, © 2020 The Inter Project Authors | SIL Open Font License 1.1 (`crates/rugi-text/assets/OFL.txt`) | Ship `OFL.txt` with the fonts; do not sell the fonts by themselves; the "Inter" name is not a Reserved Font Name. |
| JetBrains Mono (Regular, Medium), the default `FontFamily::Mono` | JetBrains/JetBrainsMono v2.304, © 2020 The JetBrains Mono Project Authors | SIL OFL 1.1 (`crates/rugi-text/assets/OFL-JetBrainsMono.txt`) | Ship the license with the font; do not sell the font by itself. |
| Lucide icon font + codepoints (`crates/rugi-icons/assets/`), from `lucide-static` 1.73.0 | lucide.dev | ISC (Feather-derived icons: MIT), `crates/rugi-icons/assets/LUCIDE_LICENSE.txt` | Keep the copyright and permission notice with the font. |

## Rust dependencies (runtime graph of `rugi-ffi` + demo)

Audited with `cargo metadata` for `aarch64-apple-darwin` (139 crates),
`x86_64-pc-windows-msvc` (127) and `x86_64-unknown-linux-gnu` (154). Every crate
is available under a permissive license:

* MIT and/or Apache-2.0 (the large majority: wgpu, winit, cosmic-text, swash,
  harfrust, skrifa, fontdb, bytemuck, pollster, …)
* Zlib: `foldhash`, `slotmap`, `zlib-rs` (attribution in docs)
* ISC: `libloading`
* BSL-1.0 (Windows only, via `arboard`): `clipboard-win`, `error-code`
* BSD-2/3-Clause (Linux only): `arrayref`, `tiny-skia`, `tiny-skia-path`
* Unicode-3.0 (with MIT/Apache): `unicode-ident`
* `self_cell` is dual-licensed "Apache-2.0 OR GPL-2.0-only"; Rugi uses it
  under Apache-2.0.

No copyleft-only dependency is present. Before shipping a binary, include the
license texts of these crates in your distribution (e.g. generate them with
`cargo about` or `cargo-bundle-licenses`).

## Build-time / benchmark-only tools (not linked into shipped binaries)

| Tool | License | Use |
|---|---|---|
| cbindgen | MPL-2.0 | dev-dependency that generates `include/rugi/rugi.h`. Not linked. |
| egui | MIT OR Apache-2.0 | optional `egui` feature of `benchmarks/` only. |
| Dear ImGui | MIT | `benchmarks/imgui/` compiles a user-supplied checkout for comparison; nothing from it is in this repository. |
