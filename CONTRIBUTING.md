# Contributing to Hydrax

Thank you for your interest in contributing to Hydrax.

## Development

1. Fork and clone the repository.
2. Install the stable Rust toolchain with `rustup`.
3. Install optional local tools:

    ```shell
    cargo install cargo-deny cargo-audit cargo-nextest git-cliff
    pipx install pre-commit
    ```

4. Install hooks:

    ```shell
    pre-commit install
    ```

5. Run the local checks. A change is not finished until all of them pass:

    ```shell
    cargo fmt --all --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo test --all-features
    cargo doc --no-deps --all-features
    pre-commit run --all-files
    ```

6. Before a release, and whenever a dependency or a manifest changes, also run
   the checks that only a release would otherwise reach:

    ```shell
    cargo +1.85 check --all-targets --all-features --locked
    cargo package --workspace --exclude hydrax-kernels-sys --locked
    cargo deny check
    uvx zensical build --strict
    ```

    The first checks the minimum supported Rust version the manifests promise;
    the explicit `+1.85` is needed because `rust-toolchain.toml` would otherwise
    select stable. The second packages exactly the set the release publishes:
    `hydrax-kernels-sys` is `publish = false` and compiles C sources from outside
    its own directory, so a packaged copy of it cannot build. The last needs
    [`uv`](https://docs.astral.sh/uv/), and builds the documentation site with
    broken cross-references treated as errors.

Use Conventional Commits for commit messages and pull request titles. The
changelog is generated with `git-cliff` at release time from those messages, so
a commit subject that does not follow the convention is a changelog entry that
reads badly.

A change to a numerical operator needs a convergence test, not only a
correctness test. See
[convergence and orders of accuracy](docs/design/convergence.md) for what such a
test has to establish and how to write one that can fail.

## Licensing

By contributing, you agree that your contributions will be licensed under the
project's 3-Clause BSD License.
