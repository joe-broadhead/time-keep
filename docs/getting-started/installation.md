# Installation

## Installer

```bash
curl -fsSL https://raw.githubusercontent.com/joe-broadhead/time-keep/HEAD/scripts/install.sh | bash
```

Install the binary and time-keep agent skills:

```bash
curl -fsSL https://raw.githubusercontent.com/joe-broadhead/time-keep/HEAD/scripts/install.sh | bash -s -- --install-skills
```

Install shell completions explicitly:

```bash
curl -fsSL https://raw.githubusercontent.com/joe-broadhead/time-keep/HEAD/scripts/install.sh | bash -s -- --install-completions --shell zsh
```

Installer options:

```bash
scripts/install.sh --install-dir "$HOME/.local/bin"
scripts/install.sh --install-skills --skills-dir "$HOME/.agents/skills"
scripts/install.sh --install-skills --skill time-keep
scripts/install.sh --install-completions --shell zsh --completions-dir "$HOME/.zsh/completions"
scripts/install.sh --dry-run
```

Environment overrides include `TIME_KEEP_VERSION`, `TIME_KEEP_INSTALL_DIR`,
`TIME_KEEP_INSTALL_SKILLS`, `TIME_KEEP_SKILLS_DIR`,
`TIME_KEEP_INSTALL_COMPLETIONS`, `TIME_KEEP_COMPLETIONS_DIR`,
`TIME_KEEP_COMPLETION_SHELL`, and `TIME_KEEP_GITHUB_TOKEN`.

Set `TIME_KEEP_GITHUB_TOKEN` for private repositories or rate-limit-sensitive
automation. `gh auth token` is a convenient source when the GitHub CLI is
already authenticated.

Checksum verification is enabled by default. Set `TIME_KEEP_VERIFY_CHECKSUM=0`
only for controlled local testing.

## Prebuilt Binaries

Release assets are published from the GitHub Release workflow with checksums,
SBOMs, provenance JSON assets, and GitHub-hosted attestations where supported.

```bash
# macOS Apple Silicon, using an authenticated GitHub CLI session.
gh release download \
  --repo joe-broadhead/time-keep \
  -p time-keep-macos-arm64.tar.gz \
  -p time-keep-macos-arm64.sha256

shasum -a 256 -c time-keep-macos-arm64.sha256
tar -xzf time-keep-macos-arm64.tar.gz
./time-keep-macos-arm64/time-keep --version
```

Choose the asset for your platform from
[Releases](https://github.com/joe-broadhead/time-keep/releases).

## From Source

```bash
git clone https://github.com/joe-broadhead/time-keep.git
cd time-keep
cargo build --locked --release
./target/release/time-keep --version
```

For local development, use `cargo run`:

```bash
cargo run -- now --tz UTC --tz Europe/Madrid
```

## Rust Version

time-keep targets Rust 1.93+ and commits `Cargo.lock` for reproducible binary
builds.
