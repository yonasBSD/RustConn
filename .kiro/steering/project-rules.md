---
inclusion: always
---

# RustConn — Project Rules

Communication language: Ukrainian.

## Architecture (4 crates)

| Crate | Purpose | Restrictions |
|-------|---------|-------------|
| `rustconn-core` | Business logic, models, protocols | **FORBIDDEN**: gtk4, adw, vte4 |
| `rustconn-cli` | CLI interface | Only rustconn-core |
| `rustconn` | GTK4/libadwaita GUI | May import everything |
| `rustconn-pty-sys` | Isolated FFI helper (macOS PTY controlling terminal, `setsid`+`TIOCSCTTY`) | **Only** sanctioned `unsafe` location (M-UNSAFE); `libc` only, no gtk4/adw/vte4 |

## Code Philosophy (YAGNI / lazy-senior)

The best code is the code never written. Lazy means efficient, not careless.
Before writing any code, stop at the first rung that holds:

1. Does this need to exist at all? → no: skip it (YAGNI)
2. Does `std` already do this? → use it
3. Is there a native platform / GTK4 / libadwaita feature? → use it
4. Does an already-present dependency solve it? → use it
5. Can it be one line? → make it one line
6. Only then: the minimum code that works

- Deletion over addition. Boring over clever. Fewest files possible.
- No abstractions, traits, or generics that weren't asked for. No boilerplate nobody requested.
- No new crate if it can be avoided (also respects `cargo deny` / supply-chain).
- Question complex requests: "Do you actually need X, or does Y cover it?"
- When two `std` approaches are the same size, pick the edge-case-correct one. Lazy means less code, not the flimsier algorithm.
- Mark intentional simplifications with a `// ponytail:` comment that names the ceiling and the upgrade path, e.g. `// ponytail: O(n²) scan, fine for <100 hosts; index if the list grows`.
- **Never lazy about** (these are never on the chopping block): trust-boundary input validation, error handling that prevents data loss, security/credentials (see Absolute Rules), accessibility (see GNOME HIG).
- Tests are **not** subject to laziness: the existing test policy below stands — keep at least the rustconn-core property-test coverage; never drop a test to "save code".

## Absolute Rules

- `unsafe_code = "forbid"` in all crates **except** `rustconn-pty-sys` — the single sanctioned FFI location (M-UNSAFE). Never write `unsafe` anywhere else.
- Passwords/keys → `secrecy::SecretString`, never plain String
- Intermediate `expose_secret().to_string()` → wrap in `zeroize::Zeroizing::new()`
- Errors → `thiserror::Error`, never `unwrap()`/`expect()`
- Logging → `tracing`, never `println!`/`eprintln!`
- i18n → `i18n()` / `i18n_f()` with `{}` placeholders for all user-facing strings
- `display_name()` values used in UI → wrap in `i18n()` at call site
- After new i18n strings → `bash po/update-pot.sh` + `msgmerge --update` for 16 languages
- Rust 2024 edition: let-chains instead of collapsible_if
- Never `set_var`/`remove_var` (unsafe in Rust 2024)

## Quick Commands

```
cargo fmt --all                    # Format
cargo clippy --all-targets         # Lint (0 warnings)
cargo test --workspace             # Tests (~120s, argon2 is slow)
cargo test -p rustconn-core --test property_tests  # Property tests only
bash po/update-pot.sh              # Regenerate POT after new i18n strings
```

## Quality Checks

Delegate to `rust-quality-check` sub-agent for fmt+clippy+tests instead of running in main context.
For quick single-file validation → `getDiagnostics`.

### Self-Check Rules (no hooks — apply mentally)

When writing `.rs` files, verify BEFORE writing:
- **Crate boundary**: `rustconn-core/` and `rustconn-cli/` must NOT contain `use gtk4`, `use adw`, `use vte4`, `gtk4::`, `adw::`, `vte4::`. Move GUI code to `rustconn/`.
- **No unsafe**: never write `unsafe {`, `unsafe fn`, `unsafe impl`, `unsafe trait` — **except** in `rustconn-pty-sys` (the sole sanctioned FFI crate, M-UNSAFE). New `unsafe` outside it is forbidden.

After writing `.rs` files in `rustconn/src/`, verify:
- **i18n**: all user-facing strings (`.set_label()`, `.set_title()`, `.set_tooltip_text()`, `Button::with_label()`) wrapped in `i18n()` or `i18n_f()`. Ignore: tracing, CSS, icons, action names.
- **Credentials** (in secret/password/credential files): `SecretString` for passwords, `.zeroize()` intermediates, no secrets in logs/args/errors.
- **Protocol files**: business logic in rustconn-core, GTK in rustconn.

### When to Run fmt/clippy/tests

- **Do NOT** run `cargo fmt`/`cargo clippy` automatically on every change — use `getDiagnostics` for quick validation.
- Run `rust-quality-check` sub-agent only when: (a) about to commit, (b) user explicitly asks, (c) finishing a multi-file feature.
- Run tests only when: (a) user explicitly asks, (b) finishing a spec task, (c) before release.
- After completing work, inform the user: "Done. Run quality check (fmt+clippy)?" — wait for confirmation.

### Test Run Rules (CRITICAL)

- **NEVER** pipe `cargo test` through `tail`, `grep`, or any filter — run directly to see progress.
- **NEVER** start `cargo test` if another instance is already running (`pgrep -f 'cargo test'`).
- Tests take ~120s (argon2 property tests). This is normal — wait for completion, do NOT assume timeout.
- If a hook or sub-agent already ran tests in this turn, do NOT re-run them.
- Use timeout 180s for test commands.

### Shared Terminal & Sub-agents (CRITICAL)

The main agent and all sub-agents (e.g. `rust-quality-check`) share ONE persistent
bash session. Concurrent or queued commands interleave, producing `Exit Code -1`,
glued-together command lines, stale output, and `bash-5.2$` prompt artifacts.
The terminal architecture cannot be fixed from rules — only the collisions can.
Apply this discipline to avoid them:

- **One terminal owner at a time.** While a sub-agent that may touch the terminal
  is running (`rust-quality-check` and any cargo-running agent), the main agent
  MUST NOT run any bash command — wait for the sub-agent's result.
- **Never delegate cargo runs to more than one sub-agent in parallel.** Centralize
  all `cargo build/clippy/test` through a single `rust-quality-check` invocation.
- **No polling loops.** Never use `sleep N; tail …` to watch progress. Run the
  command once, redirect to a log file, then read it with `readFile`.
- **Logs go inside the workspace** (`target/*.log`), never `/tmp` — `readFile` is
  restricted to the workspace and cannot read `/tmp`.
- **Check before launching.** Run `pgrep -f 'cargo'` first; if anything is running,
  do not start another cargo command.
- **One command per `executeBash` call.** Do not chain unrelated commands with
  `;`/`&&` into a single line that the shared shell may split incorrectly.

## 16 Translation Languages

be, cs, da, de, es, fr, it, kk, nl, pl, pt, sk, sv, uk, uz, zh-cn

## External Standards

In addition to the local rules above, RustConn follows:

- **[Microsoft Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/)** — details and adaptation in `rust-pragmatic-guidelines.md` (auto-included for `*.rs`). Key points: `#[expect]` instead of `#[allow]`, M-PANIC-ON-BUG, `# Errors` / `# Panics` sections in public APIs, `mimalloc` as an option.
- **[GNOME HIG](https://developer.gnome.org/hig/)** — details and adaptation in `gnome-hig.md` (auto-included for `rustconn/src/**/*.rs`). Key points: `adw::AlertDialog` instead of `gtk::MessageDialog`, CSS class `suggested-action` / `destructive-action`, mandatory keyboard shortcuts (Ctrl+W, Ctrl+Q, F10), Toast vs Banner vs Dialog.
- **[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)** — standard Rust conventions (C-CONV, C-GETTER, C-COMMON-TRAITS).
