# Changelog

All notable user-visible changes to smartgrep. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/).

Add entries under **Unreleased** in the same commit as the change. `scripts/release.sh` turns that section into the release's version heading and uses it as the GitHub release notes. Releases before 0.4.0 are described only on the [GitHub releases page](https://github.com/rohitkg98/smartgrep/releases).

## [Unreleased]

### Added
- `smartgrep init`: one-step, non-interactive project onboarding for coding agents. Detects languages, builds the index, writes a short smartgrep block (between `<!-- smartgrep:start -->` / `<!-- smartgrep:end -->` markers, refreshed in place on re-run) into existing `CLAUDE.md`/`AGENTS.md` (or a new `CLAUDE.md`) with example commands using real symbols from the project and the kind vocabulary for detected languages only, installs the repo-scoped skill, and adds `.smartgrep/` to `.gitignore` in git repos. Flags: `--agents-md`, `--no-skill`, `--dry-run`. ([#7](https://github.com/rohitkg98/smartgrep/issues/7))
- Prebuilt binaries for more platforms: Intel macOS, Linux armv7, armv6 (Raspberry Pi Zero/1), i686, riscv64, ppc64le, s390x and loongarch64, FreeBSD x86_64, and Windows x86_64/arm64. `install.sh` also works on Android (Termux), installing into `$PREFIX/bin`.
- Windows installer: `irm https://raw.githubusercontent.com/rohitkg98/smartgrep/main/install.ps1 | iex` installs to `%LOCALAPPDATA%\Programs\smartgrep` and adds it to the user `PATH`.
- Releases include a `SHA256SUMS` file; `install.sh` and `install.ps1` verify the download against it.
- `SMARTGREP_VERSION` pins the version to install and `SMARTGREP_INSTALL_DIR` overrides the install directory (both installers). Unsupported platforms get a pointer to `cargo install --git https://github.com/rohitkg98/smartgrep`.

### Fixed
- `install.sh` on Intel Macs failed with a 404 because no x86_64 macOS binary was published. ([#16](https://github.com/rohitkg98/smartgrep/issues/16))
- `install.sh` in a Rosetta shell on Apple Silicon now installs the native arm64 binary.

## [0.4.0] - 2026-09-23

### Added
- Call graph for all languages (Rust, Java, Go, TypeScript, Python): function and method bodies are walked for calls, so `deps <fn>` lists what a function calls and `refs <fn>` lists its callers. Static/module-qualified calls keep their path (`User::new`, `fmt.Errorf`, `os.path.join`); calls on an instance are recorded by method name (`self.save()` → `save`). ([#12](https://github.com/rohitkg98/smartgrep/issues/12))
- `refs` accepts qualified names to narrow matches: `refs User::new`, `refs fmt.Errorf` (`::`, `.` and `/` are interchangeable).
- `refs <Type>` also lists qualified call sites on that type (`User::new(..)`, `AppError::NotFound(..)`).
- Rust trait default methods are indexed as `method` symbols with the trait as `parent`.

### Changed
- `refs` and `implementing` match by name instead of exact text: a bare name matches the last path segment with generics ignored. `refs Index` now finds `use crate::index::types::Index`, `implementing Display` finds `impl fmt::Display for X`, and `implementing Processor` finds `implements Processor<String>`.
- Rust grouped imports (`use a::{B, C}`) are recorded as one import per name.
- Python base classes are recorded as written (`cabc.Mapping`, not `Mapping`); `implementing Mapping` still matches.
- Index format version 4: existing indexes rebuild automatically on first use. Indexes are roughly 2x larger because of call dependencies.

### Fixed
- `refs <name>` returned "No references found" for anything imported by path (most Rust and Python symbols).
- `deps <function>` was always empty because no function-level dependencies were recorded.
