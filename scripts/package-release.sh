#!/usr/bin/env bash
set -euo pipefail

# Packages a self-contained Genubar GitHub Release tarball matching
# `genubar-v<version>-linux-<arch>.tar.gz` expected by `genubar-installer`.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n1)"
VERSION="${VERSION:-0.1.0}"
ARCH="$(uname -m)"
PKG_NAME="genubar-v${VERSION}-linux-${ARCH}"
STAGE_DIR="${REPO_ROOT}/dist/${PKG_NAME}"
TARBALL_PATH="${REPO_ROOT}/dist/${PKG_NAME}.tar.gz"
SHA256_PATH="${TARBALL_PATH}.sha256"

echo "Building Genubar v${VERSION} (${ARCH}) in release mode..."
cargo build --workspace --release

rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/systemd"

for bin in genubar-daemon genubar-cli genubar-settings genubar-installer; do
    cp -f "target/release/${bin}" "${STAGE_DIR}/${bin}"
    chmod 0755 "${STAGE_DIR}/${bin}"
done

cp -f "target/release/libgenubar_gtk4_hook.so" "${STAGE_DIR}/libgenubar_gtk4_hook.so"
cp -f "systemd/genubar.service" "${STAGE_DIR}/systemd/genubar.service"
cp -f "LICENSE" "${STAGE_DIR}/LICENSE"
cp -f "README.md" "${STAGE_DIR}/README.md"

if [ -d "genubar-gnome-extension" ]; then
    mkdir -p "${STAGE_DIR}/genubar-gnome-extension"
    cp -a "genubar-gnome-extension/." "${STAGE_DIR}/genubar-gnome-extension/"
    rm -rf "${STAGE_DIR}/genubar-gnome-extension/.git"
fi

rm -f "${TARBALL_PATH}" "${SHA256_PATH}"
tar -C "${REPO_ROOT}/dist" -czf "${TARBALL_PATH}" "${PKG_NAME}"
(
    cd "${REPO_ROOT}/dist"
    sha256sum "${PKG_NAME}.tar.gz" > "${PKG_NAME}.tar.gz.sha256"
)

echo "Release archive ready: ${TARBALL_PATH}"
echo "SHA-256 checksum ready: ${SHA256_PATH}"
ls -lh "${TARBALL_PATH}" "${SHA256_PATH}"
cat "${SHA256_PATH}"
