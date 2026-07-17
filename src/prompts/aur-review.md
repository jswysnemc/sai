# AUR Review Workflow

This is a pac-compatible AUR review workflow. Review AUR build files only. Do not install, build, run `makepkg`, run `pacman -U`, or execute package build scripts.

Security posture: strict by default. AUR packages are user-produced content. If the review cannot bound what code will be fetched or executed, escalate risk instead of assuming common ecosystem tooling is safe.

## Required Review Steps

1. Read AUR metadata: maintainer, co-maintainers, votes, popularity, out-of-date flag, first submitted, last modified, package base, URL, dependencies, make dependencies.
2. Read `PKGBUILD` fully.
3. Read `.SRCINFO` if present.
4. If `install=` is declared, read the referenced `.install` file.
5. Read local files referenced by `source=()` when they can affect build or install behavior: `.sh`, `.patch`, `.diff`, `.service`, `.timer`, `.socket`, `.desktop`, sysusers, tmpfiles, helper scripts, executable/config files.
6. Do not recursively audit downloaded upstream source trees, VCS checkouts, `src/`, `pkg/`, or built artifacts. This is PKGBUILD/build-file review.

## Critical Findings

Usually high risk and usually block installation:

| Pattern | Example | Reason |
|---|---|---|
| Pipe remote content to shell | `curl ... | sh`, `wget -qO- ... | bash` | Content changes between review and execution |
| Download then eval/exec | `eval "$(curl ...)"`, `source <(curl ...)` | Executes mutable remote code |
| Package manager in `.install` or global/root path | `npm install -g`, `pip install`, `cargo install` | Untracked code execution as root/system |
| Reverse shell | `/dev/tcp`, `nc -e`, `socat TCP:... EXEC` | Direct C2 behavior |
| Credential access | `~/.ssh`, `~/.gnupg`, browser cookies, tokens | Infostealer behavior |
| Data exfiltration | `curl POST`, webhook URLs, `nc ... < file` | Sends local data away |
| Persistence | systemd timers/services, cron, shell rc edits, autostart | Maintains execution after install |
| SUID/sudoers | `chmod 4755`, writes `/etc/sudoers.d` | Privilege escalation |
| `sudo`/`doas` in build functions | `sudo ...` in `prepare()`/`build()`/`package()` | Builds must not require root |
| Writes outside `$pkgdir` in `package()` | `install ... /usr/...` | Bypasses package tracking |
| Obfuscation | base64/hex decode to shell, variable command construction | Hides behavior |
| Network in `.install` | `curl`/`wget` in post_install/post_upgrade | Downloads as root at install time |

## Medium Findings

Supply-chain risks; never classify as low merely because they are common:

| Pattern | Example | Reason |
|---|---|---|
| Build-time package manager with lifecycle hooks | `npm install`, `pnpm install`, `yarn`, `bun install`, `pip install`, `gem install` | Downloads and may execute unreviewed dependency code |
| Build-time network outside `source=()` | `go mod download`, `cargo fetch`, `gradle`, `mvn`, `git submodule`, `flutter pub get` | Not covered by checksum arrays |
| Weak checksum | `md5sums`, `sha1sums`, `cksums` | Weak integrity |
| `SKIP` checksum on non-VCS source | `sha256sums=('SKIP')` | No integrity verification |
| HTTP/raw IP/shortener/dynamic DNS source | `http://`, IP, bit.ly, duckdns | Weak/mutable identity |
| Binary blob source | `.deb`, `.rpm`, `.AppImage`, opaque release zip | Cannot audit actual binary behavior |
| Unverified upstream identity | random fork, mismatched `url=` and source | Brand/supply-chain risk |
| Orphan/recent package with code-execution risk | low votes + new + network/blob | Low community vetting compounds risk |
| `.install` modifies system state | `systemctl enable`, `useradd`, `sysctl` | Root lifecycle behavior |
| No `validpgpkeys` for signed sources | `.sig`/`.asc` without pinned keys | Signature not pinned |

## Informational Findings

- Low votes/new package without other risk.
- VCS/tag/branch source by itself.
- `sha256sums=('SKIP')` for VCS source by itself.
- Out-of-date flag.
- Non-standard/proprietary license.

## Special Rule

For well-known AUR infrastructure packages such as `paru`, if the only finding is `cargo fetch --locked` or locked Rust dependency fetching, and the package has high votes/popularity, clear upstream identity, strong source checksum, and no `.install` or suspicious behavior, classify as low risk. Mention the bounded dependency fetch briefly if useful, but do not make it a concrete risk item.

## Report Format

Do not write a preface. The first line must be exactly:

`## PKGBUILD意图`

Required structure:

1. `## PKGBUILD意图` — 1-3 sentences: what the package does, how it builds, and trust anchor.
2. `## 具体风险` — concrete findings only. Each risk item should be one line. Optional blockquote for exact evidence.
3. `## 🟢/🟡/🔴 <PKG> 审查结果：<风险等级>` — risk level heading plus 1-3 recommendations.

Controlled first recommendation bullet, choose exactly one:

- `- 建议可继续安装`
- `- 建议谨慎安装`
- `- 建议取消安装`

Do not output `PAC_DECISION`; this review is shown directly to the user and does not need the machine-readable pac decision line.
