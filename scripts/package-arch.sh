#!/usr/bin/env bash
set -euo pipefail

pkgname=sai
pkgver="$(grep '^version = ' "$(dirname "${BASH_SOURCE[0]}")/../Cargo.toml" | head -n1 | cut -d '"' -f2)"
pkgrel="${1:-1}"
arch=x86_64
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
pkgdir="${TMPDIR:-/tmp}/sai-pkg-${pkgver}-${pkgrel}"
pkgout="${SAI_PACKAGE_OUT_DIR:-${XDG_CACHE_HOME:-${HOME}/.cache}/sai/packages}"
pkgfile="${pkgout}/${pkgname}-${pkgver}-${pkgrel}-${arch}.pkg.tar.zst"

mkdir -p "${pkgout}"
rm -rf "${pkgdir}" "${pkgfile}"
mkdir -p "${pkgdir}/usr/bin"
install -Dm755 "${root}/target/release/sai" "${pkgdir}/usr/bin/sai"

size="$(du -sb "${pkgdir}/usr" | cut -f1)"
cat > "${pkgdir}/.PKGINFO" <<EOF
pkgname = ${pkgname}
pkgbase = ${pkgname}
pkgver = ${pkgver}-${pkgrel}
pkgdesc = Sai command-line AI assistant
url = https://github.com/jswysnemc/sai
builddate = $(date +%s)
packager = Sai Release <noreply@example.com>
size = ${size}
arch = ${arch}
license = MIT
depend = gcc-libs
depend = glibc
depend = alsa-lib
depend = chafa
depend = ripgrep
optdepend = fish: fish shell integration support
optdepend = bash: bash shell integration support
optdepend = zsh: zsh shell integration support
EOF

bsdtar --zstd -cf "${pkgfile}" -C "${pkgdir}" .PKGINFO usr
echo "${pkgfile}"
