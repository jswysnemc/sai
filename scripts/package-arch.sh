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
memes_dir="${pkgdir}/usr/share/sai/memes"

mkdir -p "${pkgout}"
rm -rf "${pkgdir}" "${pkgfile}"
mkdir -p "${pkgdir}/usr/bin" "${memes_dir}"
install -Dm755 "${root}/target/release/sai" "${pkgdir}/usr/bin/sai"

# 默认不打包知识库资料；用户可在配置界面或 `sai kb` 自行管理。
if [[ -d "${root}/src/memes" ]]; then
    while IFS= read -r -d '' file; do
        rel="${file#"${root}/src/memes/"}"
        install -Dm644 "${file}" "${memes_dir}/${rel}"
    done < <(find "${root}/src/memes" -type f \( -name '*.json' -o -name '*.jpg' -o -name '*.jpeg' -o -name '*.png' -o -name '*.gif' -o -name '*.webp' \) -print0 | sort -z)
fi

size="$(du -sb "${pkgdir}/usr" | cut -f1)"
cat > "${pkgdir}/.PKGINFO" <<EOF
pkgname = ${pkgname}
pkgbase = ${pkgname}
pkgver = ${pkgver}-${pkgrel}
pkgdesc = Sai command-line AI assistant
url = https://github.com/SHORiN-KiWATA/Sai
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
