# Cider Press

A high-precision ICC color management bridge that compiles the LittleCMS (`lcms2`) C engine to WebAssembly via Rust.

This library is specifically designed to provide high-performance, memory-safe color profile transformations (including soft proofing) for modern JavaScript applications like Angular and Electron.

## Features

- **Full LCMS2 Power:** Access to the real `cmsCreateProofingTransform` (soft proofing) engine.
- **16-bit Precision:** Supports 16-bit intermediate buffers for transformation pipelines to prevent quantization artifacts.
- **WASI Cross-compiled:** Uses the WASI SDK to properly cross-compile the underlying C engine, preventing "missing libc" errors.
- **Memory Safe:** Powered by `wasm-bindgen`, which automatically handles the complex pointer and memory management between JavaScript and WebAssembly.

---

## Local Development Setup

Because `cider-press` relies on compiling the LittleCMS C library to WebAssembly, standard `cargo build` or `wasm-pack` commands will fail out of the box due to a missing C standard library for the default `wasm32-unknown-unknown` target.

We solve this by cross-compiling to `wasm32-wasip1` using the WASI SDK.

### Prerequisites

1. **Rust Toolchain:**
   If you don't have Rust installed, install it (Mac/Linux) using `rustup`:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
   Then add the WASI target:
   ```bash
   rustup target add wasm32-wasip1
   ```

2. **WASI SDK:**
   Download the [WASI SDK](https://github.com/WebAssembly/wasi-sdk/releases) (version 32 or later) and extract it to `/opt/wasi-sdk`.
   
   **For Linux:**
   ```bash
   wget https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-32/wasi-sdk-32.0-x86_64-linux.tar.gz
   sudo tar -xzf wasi-sdk-32.0-x86_64-linux.tar.gz -C /opt
   sudo mv /opt/wasi-sdk-32.0-x86_64-linux /opt/wasi-sdk
   ```
   
   **For macOS (Apple Silicon):**
   ```bash
   curl -LO https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-32/wasi-sdk-32.0-arm64-macos.tar.gz
   sudo tar -xzf wasi-sdk-32.0-arm64-macos.tar.gz -C /opt
   sudo mv /opt/wasi-sdk-32.0-arm64-macos /opt/wasi-sdk
   ```
   *(For Intel Macs, replace `arm64` with `x86_64` in the URL and filenames above).*

3. **wasm-bindgen CLI:**
   ```bash
   cargo install wasm-bindgen-cli
   ```

### Building the Library

We provide a convenience script that configures the C compiler environment variables and runs the build.

1. Set the path to your extracted WASI SDK:
   ```bash
   export WASI_SDK_PATH=/opt/wasi-sdk
   ```

2. Run the build script:
   ```bash
   ./build.sh
   ```

This will:
- Compile the Rust and C code to WASM
- Generate JavaScript/TypeScript bindings via `wasm-bindgen` (using `--target web` for Zone.js / Angular compatibility)
- Package everything into a `.tgz` file in `dist/`

The output will be a file like `cider-press-0.1.0.tgz`.

---

## Usage in Angular / Electron

### Installing the package

Copy the `.tgz` into your Angular/Electron project and install it:
```bash
npm install ./dist/cider-press-0.1.0.tgz
```

This installs `cider-press` as a regular dependency in your `node_modules`, with full TypeScript definitions included.

### Serving the `.wasm` file

Because we use `--target web`, the `.wasm` binary is loaded at runtime via `fetch`. You need to copy it into your Angular app's served assets.

Add this to your `angular.json` under `architect > build > options > assets`:
```json
{ "glob": "cider_press_bg.wasm", "input": "node_modules/cider-press", "output": "/assets/wasm" }
```

### Electron Configuration
If you are using this inside Electron, you **must** update your Content Security Policy to allow WebAssembly execution:
```html
<meta http-equiv="Content-Security-Policy" content="script-src 'self' 'wasm-eval';">
```

### Angular Integration Example

Wrap the WebAssembly module in an Angular Service. The `init()` call must complete before any exported functions are called:

```typescript
import { Injectable } from '@angular/core';
import init, { apply_soft_proof_16bit } from 'cider-press';

@Injectable({ providedIn: 'root' })
export class ColorCorrectionService {
  private wasmInitPromise: Promise<void> | null = null;

  private ensureInitialized(): Promise<void> {
    if (!this.wasmInitPromise) {
      // Pass the path to the .wasm file served from assets
      this.wasmInitPromise = init('assets/wasm/cider_press_bg.wasm');
    }
    return this.wasmInitPromise;
  }

  async softProof(imageData: ImageData, printerProfileIcc: ArrayBuffer): Promise<ImageData> {
    await this.ensureInitialized();

    const pixels = new Uint8Array(imageData.data.buffer);
    const profileBytes = new Uint8Array(printerProfileIcc);

    // 1 = RelativeColorimetric Intent
    const result = apply_soft_proof_16bit(
      pixels,
      imageData.width,
      imageData.height,
      profileBytes,
      1
    );

    return new ImageData(
      new Uint8ClampedArray(result.buffer),
      imageData.width,
      imageData.height,
    );
  }
}
```

---

## Exposed API

The library exports the following core functions to JavaScript:

### `apply_soft_proof(pixels, width, height, printer_profile_icc, intent)`
Applies a standard 8-bit soft-proofing transform, simulating how sRGB pixels will look on a target printer.

### `apply_soft_proof_16bit(pixels, width, height, printer_profile_icc, intent)`
**Recommended:** Similar to standard soft proofing, but performs the forward and return transformations using 16-bit intermediate color channels to avoid banding and quantization errors. Input and output remain 8-bit RGBA.

### `transform_pixels(pixels, width, height, source_profile_icc, dest_profile_icc, intent)`
A generic utility for performing a standard profile-to-profile color conversion without the soft-proofing simulation wrapper.

## License
MIT
