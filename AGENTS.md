# Agent Instructions for HotLine

## Build & Test Commands
- Build: `cargo build --release`
- Test all: `BEEP_VOLUME=0.0 cargo test`
- Test single: `BEEP_VOLUME=0.0 cargo test test_name`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Format check: `cargo fmt --all -- --check`
- Format fix: `cargo fmt --all`

## Code Style
- Use `anyhow::Result` for error handling, not unwrap() in production code
- Imports: group as std, external crates, then local modules
- Allow specific clippy lints at file level when justified (see src/main.rs:1-11)
- Tests modifying env vars MUST use `ENV_MUTEX`/`ASYNC_ENV_MUTEX` from `test_utils`
- Async tests holding locks: use `#[allow(clippy::await_holding_lock)]`
- Feature flags: use `#[cfg(test)]` and `#[cfg(not(test))]` for test-specific code
- Constants in UPPER_SNAKE_CASE, functions/variables in snake_case
- Document public APIs with `///` comments
- Keep functions focused and under 100 lines when possible
- Prefer explicit error messages with context using anyhow's `.context()`