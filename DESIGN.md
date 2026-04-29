# Cider Press — Rust-to-WASM lcms2 Bridge

A high-precision ICC color management bridge that compiles the LittleCMS C engine to WebAssembly via Rust, for use in your Angular + Electron application (both standalone web and desktop).

## 1. The Repository Structure

```text
cider-press/
├── .cargo/
│   └── config.toml          # WASM cross-compilation config for WASI
├── .github/
│   └── workflows/
│       └── build.yml        # CI automation for building WASM
├── src/
│   └── lib.rs               # The Rust bridge logic (lcms2 to JS)
├── build.sh                 # Convenience script for local building
├── Cargo.toml               # Rust dependencies
├── README.md                
└── LICENSE                  # MIT
```

## 2. Architecture & Challenges

The `lcms2` Rust crate is a wrapper around the LittleCMS C library (`lcms2-sys`). Because it's fundamentally C code, it cannot be straightforwardly compiled to `wasm32-unknown-unknown` without a C standard library (libc).

**The Solution:** We target `wasm32-wasip1` using the **WASI SDK**. This provides a POSIX-like environment and libc for WebAssembly. We then use `wasm-bindgen-cli` to generate the JavaScript bindings for bundlers.

## 3. Recommended Cargo.toml

```toml
[package]
name = "cider-press"
version = "0.1.0"
edition = "2021"
description = "A high-precision lcms2 (LittleCMS) bridge for WebAssembly and TypeScript."

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
wasm-bindgen = "0.2"
lcms2 = "6" # Ensure we use the latest API
console_error_panic_hook = { version = "0.1", optional = true }

[features]
default = ["console_error_panic_hook"]

[profile.release]
lto = true
opt-level = "s"       # Optimizes for binary size
strip = true          # Strip debug symbols
codegen-units = 1     # Better optimization
```

## 4. Automation with GitHub Actions

Since compiling C to WASI requires the WASI SDK, it's easiest to automate this via GitHub Actions. Your Angular app can then simply pull the built `pkg/` folder as an npm dependency or asset.

```yaml
# Simplified GitHub Action (.github/workflows/build.yml)
name: Build WASM
on: [push, release]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-wasip1
      - name: Install WASI SDK
        run: |
          # Fetch and extract WASI SDK to configure CC and AR
      - name: Build WASM
        run: cargo build --release --target wasm32-wasip1
      - name: Generate JS bindings
        run: wasm-bindgen target/wasm32-wasip1/release/cider_press.wasm --out-dir pkg/ --target bundler
```

## 5. Angular & Electron Integration

We use `--target bundler` for the WebAssembly bindings because Angular uses Webpack/esbuild.

In Angular, you wrap this logic in an Injectable Service:

```typescript
import { Injectable } from '@angular/core';
// Import the generated "glue" code
import init, { apply_soft_proof_16bit } from 'cider-press';

@Injectable({ providedIn: 'root' })
export class ColorCorrectionService {
  private initPromise: Promise<void> | null = null;

  async initialize() {
    if (!this.initPromise) {
      this.initPromise = init().then(() => {});
    }
    return this.initPromise;
  }

  async proofImage(pixels: Uint8ClampedArray, width: number, height: number, profile: ArrayBuffer): Promise<Uint8Array> {
    await this.initialize();

    // Direct Memory Transfer
    // Rust sees the Uint8Array as a &[u8] slice. 
    const result = apply_soft_proof_16bit(
      new Uint8Array(pixels.buffer), 
      width, 
      height, 
      new Uint8Array(profile),
      1 // Relative Colorimetric
    );
    
    return result;
  }
}
```

### Memory Safety & Performance

1. **Memory Allocation:** When calling the WASM function, the JS glue code automatically allocates temporary WASM memory, copies the arrays, and passes pointers to Rust.
2. **16-bit Precision:** The bridge uses `apply_soft_proof_16bit` to perform the internal transformations at 16 bits per channel, avoiding quantization errors, before returning the final 8-bit output.
3. **Electron CSP:** Ensure your Electron app's Content Security Policy allows `'wasm-eval'`.