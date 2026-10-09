## Summary

What does this change and why?

## Checks

- [ ] `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` pass
- [ ] `cargo test --workspace` passes and new behavior has tests
- [ ] No new dependency, or its license is MIT, Apache-2.0, BSD or similar (see CONTRIBUTING.md)
- [ ] Documentation updated if behavior changed

## Security

- [ ] This change does not touch encryption, password handling, archive parsing or path handling
- [ ] If it does, I explained the reasoning and the tests that cover it below
