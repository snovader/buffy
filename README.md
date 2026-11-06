# tafla

Binary tables of scalars, strings and typed arrays, in one format with
implementations in TypeScript, Python, Java and Rust. A builder writes a table
at its exact size, copying typed arrays in bulk, and a reader reads fields back
by offset, with arrays as views of the bytes where it can. Every read checks
that it stays within the bytes it was given, so tables can come from untrusted
sources.

## Status

tafla hasn't been released yet.

[SPEC.md](SPEC.md) describes the format.

## Packages

| Language   | Runtime                                           | Code generator           |
| ---------- | ------------------------------------------------- | ------------------------ |
| TypeScript | `tafla` on npm                                    | Not yet packaged         |
| Python     | `tafla` on PyPI                                   | Not yet packaged         |
| Java       | `io.github.snovader.tafla:tafla` on Maven Central | Not yet packaged         |
| Rust       | `tafla` on crates.io                              | `tafla-gen` on crates.io |

A runtime writes and reads tables. The code generator compiles a schema of
messages and tables into code that writes and reads them with a language's
runtime. It's written in Rust, and also builds as a WebAssembly module, which
packages for the other languages are to run.

## Repository

| Directory      | Contents                                                                 |
| -------------- | ------------------------------------------------------------------------ |
| `conformance/` | Golden tables that every implementation reads and rebuilds byte for byte |
| `typescript/`  | The reference implementation, which writes the conformance tables        |
| `python/`      | The Python runtime                                                       |
| `java/`        | The Java runtime                                                         |
| `rust/`        | The Rust runtime and the generator                                       |
| `schemas/`     | Example schemas, which the generator's tests compile                     |

Each language's directory is the root of its own toolchain, and its README
lists the commands that check it.

## License

[MIT](LICENSE)
