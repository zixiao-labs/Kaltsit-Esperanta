set -o pipefail

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
# The Cargo binary name produced by `cargo build --package zed`. Must match
# `[[bin]] name` in crates/zed/Cargo.toml and CFBundleExecutable inside the .app.
BIN_NAME="Kaltsit-Esperanta"
APP_NAME="Kaltsit-Esperanta"

echo "check xcode"
xcode-select --version
if [ $? -ne 0 ]; then
    echo "xcode not installed"
    exit 1
fi

echo "check brew"
brew --version
if [ $? -ne 0 ]; then
    echo "brew not installed"
    exit 1
fi

echo "check rustup"
rustup --version
if [ $? -ne 0 ]; then
    echo "rustup not installed"
    exit 1
fi

echo "🧰 Setup Toolchain"
rustup show active-toolchain || rustup toolchain install

echo "✅ Toolchain Setup Complete"

echo "⬇️ Install Dependencies"

brew install cmake

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

echo "🔨 Build Kal'tsit"

cargo build "$build_flag" --package zed --package cli --target "$target_triple"

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

echo "✅ Package Complete"
