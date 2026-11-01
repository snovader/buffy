# tafla for Rust

`core/` holds `tafla`, the runtime, and `gen/` holds `tafla-gen`, the
code generator, which compiles schemas into code for every language's
runtime and also runs as WebAssembly. `benchmarks/` holds `tafla-benchmarks`,
which compares the runtime with other Rust crates, such as FlatBuffers and
rkyv, as its README describes.

## Development

This is a Cargo workspace on stable Rust, as `rust-toolchain.toml` pins it,
with a `rust-version` of 1.88. Run the commands below from this directory.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

The generator's tests compare its output with golden files, which
`TAFLA_GEN_UPDATE=1 cargo test -p tafla-gen` rewrites. They lie in the test
tree of each target's language, as `gen/README.md` lists them, so a change to
them is followed by that language's checks and tests. Its WebAssembly module
builds with the `wasm` profile, and a Node script checks it:

```sh
rustup target add wasm32-unknown-unknown
cargo build -p tafla-gen --lib --target wasm32-unknown-unknown --profile wasm
node gen/tests/wasm/check.mjs target/wasm32-unknown-unknown/wasm/tafla_gen.wasm
```

`cargo test` runs the benchmarks' check that every codec reads back what it
wrote. The benchmarks themselves run with
`cargo bench -p tafla-benchmarks`, and take several minutes.

Miri runs the runtime's tests on a big-endian target, which reads every
array through its byte-swapping path. It takes several minutes:

```sh
cargo +nightly miri test -p tafla --target powerpc-unknown-linux-gnu
```

Miri keeps tests from reading files, so the conformance suite needs its own
run:

```sh
MIRIFLAGS=-Zmiri-disable-isolation cargo +nightly miri test -p tafla \
    --target powerpc-unknown-linux-gnu --test conformance -- --include-ignored
```
