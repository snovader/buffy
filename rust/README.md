# buffy for Rust

`core/` holds `buffy-core`, the runtime.

## Development

This is a Cargo workspace on stable Rust, as `rust-toolchain.toml` pins it,
with a `rust-version` of 1.88. Run the commands below from this directory.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Miri runs the tests on a big-endian target, which reads every array through
its byte-swapping path. It takes several minutes:

```sh
cargo +nightly miri test --target powerpc-unknown-linux-gnu
```

Miri keeps tests from reading files, so the conformance suite needs its own
run:

```sh
MIRIFLAGS=-Zmiri-disable-isolation cargo +nightly miri test \
    --target powerpc-unknown-linux-gnu --test conformance -- --include-ignored
```
