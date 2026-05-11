#!/usr/bin/env bash
set -euo pipefail

echo " █████╗ ███╗   ███╗ █████╗  ██╗ ██████╗ "
echo "██╔══██╗████╗ ████║██╔══██╗███║██╔═████╗"
echo "███████║██╔████╔██║███████║╚██║██║██╔██║"
echo "██╔══██║██║╚██╔╝██║██╔══██║ ██║████╔╝██║"
echo "██║  ██║██║ ╚═╝ ██║██║  ██║ ██║╚██████╔╝"
echo "╚═╝  ╚═╝╚═╝     ╚═╝╚═╝  ╚═╝ ╚═╝ ╚═════╝ "
echo "                                        "


build_flag="--release"
# The Cargo binary name produced by `cargo build --package zed`. Must match
# `[[bin]] name` in crates/zed/Cargo.toml.
BIN_NAME="Kaltsit-Esperanta"
APP_NAME="Kaltsit-Esperanta"

echo "check rustup"
rustup --version || { echo "rustup not installed. See https://www.rust-lang.org/tools/install"; exit 1; }

echo "🧰 Setup Toolchain"
rustup show active-toolchain || rustup toolchain install

echo "✅ Toolchain Setup Complete"

echo "⬇️ Install System Dependencies"

if [ -x "script/linux" ]; then
    script/linux
else
    echo "WARNING: script/linux not found. Please install build dependencies manually." >&2
fi

echo "✅ Dependencies Installed"

channel=$(<crates/zed/RELEASE_CHANNEL)
export ZED_RELEASE_CHANNEL="${channel}"
export ZED_BUNDLE=true

version_info=$(rustc --version --verbose)
host_line=$(echo "$version_info" | grep host)
target_triple=${host_line#*: }

if [ -z "$target_triple" ]; then
    echo "ERROR: failed to detect target triple from rustc --version" >&2
    exit 1
fi

rustup target add "$target_triple"

export CC=${CC:-$(command -v clang || command -v cc)}

echo "🔨 Build Kal'tsit"

# rpath lets the editor find bundled .so files in ../lib at runtime.
if [[ "$(uname -m)" == "aarch64" ]]; then
    export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-fuse-ld=lld -C link-args=-Wl,--disable-new-dtags,-rpath,\$ORIGIN/../lib"
else
    export RUSTFLAGS="${RUSTFLAGS:-} -C link-args=-Wl,--disable-new-dtags,-rpath,\$ORIGIN/../lib"
fi

cargo build "$build_flag" --package zed --package cli --target "$target_triple"

echo "✅ Build Complete"

echo "📦 Package App"

target_dir="${CARGO_TARGET_DIR:-target}"

suffix=""
if [ "$channel" != "stable" ]; then
    suffix="-$channel"
fi

temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT

app_dir="${temp_dir}/${BIN_NAME}${suffix}.app"

mkdir -p "${app_dir}/bin" "${app_dir}/libexec" "${app_dir}/lib"
cp "${target_dir}/${target_triple}/release/${BIN_NAME}" "${app_dir}/libexec/zed-editor"
cp "${target_dir}/${target_triple}/release/cli" "${app_dir}/bin/zed"

# Bundle the editor's dynamic library dependencies (skipping ones expected on
# every Linux system).
find_libs() {
    ldd "${target_dir}/${target_triple}/release/${BIN_NAME}" \
        | awk '{print $3}' \
        | grep -v '^$' \
        | grep -Ev '/(libstdc\+\+\.so|libc\.so|libgcc_s\.so|libm\.so|libpthread\.so|libdl\.so|libasound\.so)' \
        || true
}

libs=$(find_libs)
if [ -n "$libs" ]; then
    cp $libs "${app_dir}/lib/"
fi

mkdir -p "${app_dir}/share/icons/hicolor/512x512/apps"
cp "crates/zed/resources/app-icon${suffix}.png" \
    "${app_dir}/share/icons/hicolor/512x512/apps/${BIN_NAME}.png"
mkdir -p "${app_dir}/share/icons/hicolor/1024x1024/apps"
cp "crates/zed/resources/app-icon${suffix}@2x.png" \
    "${app_dir}/share/icons/hicolor/1024x1024/apps/${BIN_NAME}.png"

arch=$(uname -m)
archive="${BIN_NAME}-linux-${arch}.tar.gz"

mkdir -p "${target_dir}/release"
rm -f "${target_dir}/release/${archive}"
tar -czf "${target_dir}/release/${archive}" -C "${temp_dir}" "${BIN_NAME}${suffix}.app"

echo "Bundled ${target_dir}/release/${archive}"

echo "✅ Package Complete"
