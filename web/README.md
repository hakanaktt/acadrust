# wasm viewer

This is a small browser harness for the renderer. It uses the same `compile`
and public `Renderer`/wgpu draw path as the native viewer and is intended for
visual comparison while fixing entity layout.

From the repository root:

```text
cargo build --manifest-path web/Cargo.toml --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir web/pkg web/target/wasm32-unknown-unknown/debug/acadrust_wasm_viewer.wasm
python -m http.server 8000 --directory web
```

The package pins `wasm-bindgen` to the schema used by the repository's
`wasm-bindgen` CLI (0.2.121). Keep the CLI and crate versions aligned when
updating either one.

Open `http://127.0.0.1:8000/`, choose a DWG, and inspect the canvas. Choose
the drawing's `.shx` or `.shp` font in the second file picker when it uses a
stroke font such as `txt.shx`; browser builds cannot resolve native font paths.
The generated `web/pkg` directory is a build artifact and is ignored by git.
