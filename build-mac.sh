#!/bin/bash
set -euo pipefail

echo " █████╗ ███╗   ███╗ █████╗  ██╗ ██████╗ "
echo "██╔══██╗████╗ ████║██╔══██╗███║██╔═████╗"
echo "███████║██╔████╔██║███████║╚██║██║██╔██║"
echo "██╔══██║██║╚██╔╝██║██╔══██║ ██║████╔╝██║"
echo "██║  ██║██║ ╚═╝ ██║██║  ██║ ██║╚██████╔╝"
echo "╚═╝  ╚═╝╚═╝     ╚═╝╚═╝  ╚═╝ ╚═╝ ╚═════╝ "
echo "                                        "

build_flag="--release"
target_dir="release"
open_result=false
local_install=false
can_code_sign=false
with_deno=false
# The Cargo binary name produced by `cargo build --package zed`. Must match
# `[[bin]] name` in crates/zed/Cargo.toml and CFBundleExecutable inside the .app.
BIN_NAME="Kaltsit-Esperanta"
CLI_BIN_NAME="zeta"

usage() {
    cat <<EOF
Usage: ${0##*/} [--with-deno] [--help]

Options:
  --with-deno   Warm deno_core/V8 via \`cargo xtask setup-deno\` and build with
                the zed/deno-core feature so production bundles embed the JS host.
  -h, --help    Show this help.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --with-deno)
            with_deno=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

echo "check xcode"
xcode-select --version || { echo "xcode not installed"; exit 1; }

echo "check brew"
brew --version || { echo "brew not installed"; exit 1; }

echo "check rustup"
rustup --version || { echo "rustup not installed"; exit 1; }

echo "🧰 Setup Toolchain"
rustup show active-toolchain || rustup toolchain install

echo "✅ Toolchain Setup Complete"

echo "⬇️ Install Dependencies"

brew install --build-from-source cmake

cargo_bundle_version=$(cargo -q bundle --help 2>&1 | head -n 1 || echo "")
if [ "$cargo_bundle_version" != "cargo-bundle v0.6.1-zed" ]; then
    cargo install cargo-bundle --git https://github.com/zed-industries/cargo-bundle.git --branch zed-deploy
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

feature_args=()
if [ "$with_deno" = true ]; then
    echo "🦕 Preparing Deno / V8 for production embed"
    cargo xtask setup-deno
    feature_args+=(--features deno-core)
fi

echo "🔨 Build Kal'tsit"

cargo build "$build_flag" --package zed --package cli --target "$target_triple" "${feature_args[@]+"${feature_args[@]}"}"

echo "✅ Build Complete"

echo "📦 Package App"

pushd crates/zed
cp Cargo.toml Cargo.toml.backup
trap 'mv Cargo.toml.backup Cargo.toml 2>/dev/null || true; popd 2>/dev/null || true' EXIT ERR
sed \
    -i.backup \
    "s/package.metadata.bundle-${channel}/package.metadata.bundle/" \
    Cargo.toml

app_path=$(cargo bundle "$build_flag" --target "$target_triple" --select-workspace-root | xargs)

mv Cargo.toml.backup Cargo.toml
trap - EXIT ERR
popd
echo "Bundled ${app_path}"

cli_src="target/${target_triple}/${target_dir}/${CLI_BIN_NAME}"
if [ ! -f "$cli_src" ]; then
    # Host-triple builds may land without an explicit target subdir.
    cli_src="target/${target_dir}/${CLI_BIN_NAME}"
fi
cp "$cli_src" "${app_path}/Contents/MacOS/${CLI_BIN_NAME}"
echo "Installed CLI binary ${CLI_BIN_NAME} into app bundle"

echo "✅ Package Complete"
if [ "$with_deno" = true ]; then
    echo "Deno JS extension runtime was compiled into this production build."
fi
