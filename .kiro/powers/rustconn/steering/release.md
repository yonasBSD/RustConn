# RustConn Release Workflow

Automates version, dependency, CLI, and changelog updates across all packaging files.

## When to use

1. At the start of a new branch — bump version and create an empty CHANGELOG entry
2. During development — update dependencies and CLI versions
3. After all features are complete — propagate changelog to all packaging files

## Stage 1: Starting a new version

1. Read the current version from `Cargo.toml` → `[workspace.package] version`
2. Bump the version (patch/minor/major as instructed)
3. Update the version in all files (see "Files to update")
4. Create a CHANGELOG.md entry under `## [X.Y.Z] - YYYY-MM-DD`
5. Commit: `chore: bump version to X.Y.Z`

## Stage 2: Updating dependencies (every release)

### Cargo dependencies

```bash
cargo update --dry-run          # Preview what will be updated
cargo update                    # Apply updates
cargo build                     # Verify compilation
cargo clippy --all-targets      # 0 warnings
cargo fmt --check               # Formatting
cargo test -p rustconn-core --test property_tests  # Property tests (timeout 180s)
```

Record updated packages in the CHANGELOG.md `### Dependencies` section:
```markdown
### Dependencies
- **Updated**: crate1 X.Y.Z→X.Y.W, crate2 A.B.C→A.B.D
```

Include only significant updates (not every transitive dep). Group wasm-bindgen/js-sys/web-sys into one line.

### CLI versions (Flatpak downloads)

File: `rustconn-core/src/cli_download.rs`

Run the automated version checker:

```bash
./scripts/check-cli-versions.sh          # human-readable report
./scripts/check-cli-versions.sh --json   # JSON for automation
```

The script checks all 7 pinned CLI tools against upstream latest releases:
- kubectl (dl.k8s.io/release/stable.txt)
- Tailscale (pkgs.tailscale.com/stable)
- Teleport (GitHub API)
- Boundary (HashiCorp checkpoint API)
- Bitwarden CLI (GitHub API, cli-v* tags)
- 1Password CLI (agilebits check endpoint)
- TigerVNC (GitHub API)

Exit code 0 = all current, 1 = updates available.

When the script reports updates, for each outdated component:
1. Update `pinned_version` in `DownloadableComponent`
2. Update `download_url` and `aarch64_url` (version in URL)
3. Update `checksum` if `ChecksumPolicy::Static` — download the `.sha256` file
4. Record in CHANGELOG.md (under `- Updated:` line or as a separate entry):
   ```
   - **CLI downloads** — Tailscale 1.94.2→1.96.2, kubectl 1.35.3→1.35.4
   ```
5. `cargo build && cargo clippy --all-targets` — verify compilation

Components with `SkipLatest` checksum and no `pinned_version` (AWS CLI, gcloud, cloudflared, etc.) — do not require URL updates.

## Stage 3: Finalizing the release

1. Ensure CHANGELOG.md contains a complete description of changes, including:
   - `### Added` — new features
   - `### Fixed` — bug fixes
   - `### Improved` — improvements
   - `### Changed` — CLI version changes, etc.
   - `### Dependencies` — Cargo dependency updates
   - `### Security` — if there are security-related changes
2. Propagate changelog to all packaging files (see below)
3. Sync version across all packaging files
4. Run final checks:
   ```bash
   cargo fmt --check
   cargo clippy --all-targets    # 0 warnings
   cargo test --workspace        # ~120s
   cargo build --release         # Release build
   ```
5. Merge into main
6. `git tag -a vX.Y.Z -m "Release X.Y.Z" && git push origin main --tags`

## Files to update

### 1. `Cargo.toml` (workspace root)
```toml
[workspace.package]
version = "X.Y.Z"
```

### 2. `CHANGELOG.md`
Source of truth. Format:
```markdown
## [Unreleased]

## [X.Y.Z] - YYYY-MM-DD

### Added
- **Feature Name** — Description ([#N](url))

### Fixed
- **Bug Name** — Description ([#N](url))

### Improved
- **Area** — Description

### Changed
- **CLI downloads** — Component X.Y.Z→X.Y.W

### Dependencies
- **Updated**: crate1 X.Y.Z→X.Y.W, crate2 A.B.C→A.B.D
```

### 3. `docs/USER_GUIDE.md`
First line after the heading:
```
**Version X.Y.Z** | GTK4/libadwaita Connection Manager for Linux
```

### 4. `docs/ARCHITECTURE.md`
First line after the heading:
```
**Version X.Y.Z** | Last updated: Month YYYY
```

### 5. `debian/changelog`
New section at the TOP of the file:
```
rustconn (X.Y.Z-1) unstable; urgency=medium

  * Version bump to X.Y.Z
  * [changelog entries]

 -- Anton Isaiev <totoshko88@gmail.com>  DAY, DD MON YYYY HH:MM:SS +0200
```
DAY — abbreviated day of week (Mon, Tue, Wed, Thu, Fri, Sat, Sun).

### 6. `packaging/obs/debian.changelog`
Same format as `debian/changelog`.

### 7. `packaging/obs/rustconn.changes`
```
-------------------------------------------------------------------
DAY MON DD YYYY Anton Isaiev <totoshko88@gmail.com> - X.Y.Z

- Version bump to X.Y.Z
  * [changelog entries]

```
Date format: `Sun Feb 15 2026` (no commas).

### 8. `packaging/obs/rustconn.spec`
Update `Version:` in the header + `Summary:` if protocols changed.
Add a section to `%changelog` at the TOP:
```
* DAY MON DD YYYY Anton Isaiev <totoshko88@gmail.com> - X.Y.Z-0
- Version bump to X.Y.Z
- [changelog entries]
```

### 9. `packaging/obs/rustconn.dsc`
```
Version: X.Y.Z-1
Files:
 00000000000000000000000000000000 0 rustconn_X.Y.Z.orig.tar.xz
 00000000000000000000000000000000 0 rustconn_X.Y.Z-1.debian.tar.xz
```

### 10. `packaging/obs/debian.dsc`
```
Version: X.Y.Z-1
DEBTRANSFORM-TAR: rustconn-X.Y.Z.tar.xz
```

### 11. `packaging/obs/AppImageBuilder.yml`
```yaml
    version: X.Y.Z
```

### 12. `packaging/flatpak/io.github.totoshko88.RustConn.yml`
```yaml
        tag: vX.Y.Z
```

### 13. `packaging/flatpak/io.github.totoshko88.RustConn.local.yml`
DO NOT modify — uses local path.

### 14. `packaging/flathub/io.github.totoshko88.RustConn.yml`
```yaml
        tag: vX.Y.Z
```

### 15. `rustconn/assets/io.github.totoshko88.RustConn.metainfo.xml`
Add `<release>` at the TOP of `<releases>`:
```xml
    <release version="X.Y.Z" date="YYYY-MM-DD">
      <description>
        <p>Version X.Y.Z - [short description]:</p>
        <ul>
          <li>[item]</li>
        </ul>
      </description>
    </release>
```
Also update `<description>` if protocols/features changed.

### 16. `snap/snapcraft.yaml` (if exists)
```yaml
version: 'X.Y.Z'
```

## Version sync checklist

Before tagging the release, verify that version `X.Y.Z` is present in ALL files:

```bash
# Quick check (should find the version in all files):
grep -r "X.Y.Z" Cargo.toml debian/changelog packaging/ rustconn/assets/*.xml docs/USER_GUIDE.md docs/ARCHITECTURE.md
```

| File | What to verify |
|------|----------------|
| `Cargo.toml` | `version = "X.Y.Z"` |
| `debian/changelog` | `rustconn (X.Y.Z-1)` |
| `packaging/obs/debian.changelog` | `rustconn (X.Y.Z-1)` |
| `packaging/obs/rustconn.dsc` | `Version: X.Y.Z-1` |
| `packaging/obs/debian.dsc` | `Version: X.Y.Z-1` |
| `packaging/obs/rustconn.spec` | `Version: X.Y.Z` |
| `packaging/obs/rustconn.changes` | `- X.Y.Z` |
| `packaging/obs/AppImageBuilder.yml` | `version: X.Y.Z` |
| `packaging/flatpak/*.yml` | `tag: vX.Y.Z` |
| `packaging/flathub/*.yml` | `tag: vX.Y.Z` |
| `metainfo.xml` | `<release version="X.Y.Z"` |
| `docs/USER_GUIDE.md` | `Version X.Y.Z` |
| `docs/ARCHITECTURE.md` | `Version X.Y.Z` |

## Changelog conversion

### CHANGELOG.md → Debian
```
### Added
- **Feature** — Description (#N)
```
→
```
  * Added Feature (#N):
    - Description
```

### CHANGELOG.md → RPM spec / .changes
```
### Added
- **Feature** — Description (#N)
```
→
```
- Added Feature (#N)
```

### CHANGELOG.md → Metainfo XML
- Strip markdown bold (`**...**`)
- Strip backticks
- Strip markdown links — keep text only
- Keep each `<li>` short (1 line)
- Escape XML entities: `&` → `&amp;`

## Final checklist

```bash
# 1. Code
cargo fmt --check
cargo clippy --all-targets       # 0 warnings
cargo test --workspace           # ~120s (argon2 property tests are slow in debug mode)
cargo build --release

# 2. CLI versions
./scripts/check-cli-versions.sh  # exit 0 = all current

# 3. Versions synced (all packaging files)
# 4. CHANGELOG.md contains Dependencies and Changed (CLI) sections
# 5. Packaging changelogs updated
# 6. metainfo.xml has a new <release>
# 7. Cargo.lock updated (cargo update)
```
