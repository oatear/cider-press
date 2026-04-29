#!/bin/bash
# Requires: WASI SDK installed (https://github.com/WebAssembly/wasi-sdk)
# Requires: wasm-bindgen-cli installed

set -e

# Use provided path or default to /opt/wasi-sdk
export WASI_SDK_PATH="${WASI_SDK_PATH:-/opt/wasi-sdk}"

if [ ! -d "$WASI_SDK_PATH" ]; then
    echo "Error: WASI SDK not found at $WASI_SDK_PATH"
    echo "Please set WASI_SDK_PATH to the location of the extracted wasi-sdk archive."
    exit 1
fi

export CC_wasm32_wasip1="${WASI_SDK_PATH}/bin/clang --sysroot=${WASI_SDK_PATH}/share/wasi-sysroot"
export AR_wasm32_wasip1="${WASI_SDK_PATH}/bin/llvm-ar"

# Add WASI SDK bin to PATH so rustc can find the wasm-ld linker
export PATH="${WASI_SDK_PATH}/bin:$PATH"

echo "Building Rust crate..."
cargo build --release --target wasm32-wasip1

echo "Generating JS bindings..."
rm -rf pkg
mkdir -p pkg
wasm-bindgen target/wasm32-wasip1/release/cider_press.wasm --out-dir pkg/ --target web

echo "Injecting WASI shim into pkg/cider_press.js..."
# 1. Remove the external WASI imports
# 2. Inject the wasi_shim object
# 3. Update the imports object to use the shim

JS_FILE="pkg/cider_press.js"
TEMP_JS="pkg/cider_press.tmp.js"

cat > "$TEMP_JS" << 'EOF'
/* @ts-self-types="./cider_press.d.ts" */
const wasi_shim = {
    fd_write: () => 0, fd_read: () => 0, fd_seek: () => 0, fd_close: () => 0,
    fd_fdstat_get: () => 0, fd_fdstat_set_flags: () => 0, fd_filestat_get: () => 0,
    fd_prestat_get: () => 0, fd_prestat_dir_name: () => 0, fd_advise: () => 0,
    fd_allocate: () => 0, fd_datasync: () => 0, fd_pwrite: () => 0, fd_pread: () => 0,
    fd_readdir: () => 0, fd_renumber: () => 0, fd_sync: () => 0, fd_tell: () => 0,
    path_create_directory: () => 0, path_filestat_get: () => 0, path_filestat_set_times: () => 0,
    path_link: () => 0, path_open: () => 0, path_readlink: () => 0,
    path_remove_directory: () => 0, path_rename: () => 0, path_symlink: () => 0,
    path_unlink_file: () => 0, proc_exit: () => {}, proc_raise: () => 0,
    environ_sizes_get: () => 0, environ_get: () => 0, args_sizes_get: () => 0,
    args_get: () => 0, random_get: () => 0, clock_time_get: () => 0,
    clock_res_get: () => 0, poll_oneoff: () => 0, sched_yield: () => 0,
    sock_recv: () => 0, sock_send: () => 0, sock_shutdown: () => 0,
};
EOF

# Append the rest of the file, skipping the wasi imports and replacing the import refs
grep -v 'from "wasi_snapshot_preview1"' "$JS_FILE" | grep -v 'ts-self-types' | \
    sed 's/"wasi_snapshot_preview1": import[0-9]*/"wasi_snapshot_preview1": wasi_shim/g' >> "$TEMP_JS"

mv "$TEMP_JS" "$JS_FILE"

# Read the version from Cargo.toml
CARGO_VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')

echo "Creating package.json (v${CARGO_VERSION})..."
cat > pkg/package.json << EOF
{
  "name": "cider-press",
  "version": "${CARGO_VERSION}",
  "description": "High-precision ICC color management (LittleCMS) bridge for WebAssembly.",
  "license": "MIT",
  "repository": {
    "type": "git",
    "url": "https://github.com/oatear/cider-press"
  },
  "files": [
    "cider_press_bg.wasm",
    "cider_press.js",
    "cider_press.d.ts",
    "cider_press_bg.wasm.d.ts"
  ],
  "main": "cider_press.js",
  "types": "cider_press.d.ts",
  "sideEffects": [
    "./snippets/*"
  ]
}
EOF

echo "Packing .tgz..."
cd pkg
npm pack
cd ..

# Move the .tgz to dist/ for easy access
mkdir -p dist
mv pkg/cider-press-${CARGO_VERSION}.tgz dist/

echo ""
echo "Build successful!"
echo "  Package: dist/cider-press-${CARGO_VERSION}.tgz"
echo "  Install: npm install ./dist/cider-press-${CARGO_VERSION}.tgz"
