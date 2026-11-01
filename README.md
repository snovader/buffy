# buffy

Binary tables of scalars, strings and typed arrays, in one format with
implementations in TypeScript, Python, Java and Rust. A builder writes a table
at its exact size, copying typed arrays in bulk, and a reader reads fields back
by offset, with arrays as views of the bytes where it can. Every read checks
that it stays within the bytes it was given, so tables can come from untrusted
sources.

## Status

buffy hasn't been released yet.

## Packages

| Language   | Runtime                               | Code generator                       |
| ---------- | ------------------------------------- | ------------------------------------ |
| TypeScript | `@buffy/core` on npm                  | `@buffy/gen` on npm                  |
| Python     | `buffy-core` on PyPI                  | `buffy-gen` on PyPI                  |
| Java       | `io.github.snovader.buffy:buffy-core` | `io.github.snovader.buffy:buffy-gen` |
| Rust       | `buffy-core` on crates.io             | `buffy-gen` on crates.io             |

A runtime writes and reads tables. The code generator compiles a schema of
messages and tables into code that writes and reads them with a language's
runtime. It's written in Rust, and the packages for the other languages run it
as WebAssembly.

## Repository

| Directory      | Contents                                                                                       |
| -------------- | ---------------------------------------------------------------------------------------------- |
| `conformance/` | Golden tables that every implementation reads and rebuilds byte for byte                       |
| `typescript/`  | The reference implementation, which writes the conformance tables, and the generator's package |
| `python/`      | The Python runtime and the generator's package                                                 |
| `java/`        | The Java runtime and the generator's package                                                   |
| `rust/`        | The Rust runtime and the generator                                                             |

Each language's directory is the root of its own toolchain, and its README
lists the commands that check it.

## License

[MIT](LICENSE)
