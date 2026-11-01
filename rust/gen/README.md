# buffy-gen

Compiles a schema of messages, tables and structs into code that writes and
reads them with buffy's runtime for TypeScript, JavaScript, Python, Java or
Rust. It runs as a command, as a Rust library, and as a WebAssembly module
without imports, which the packages for the other languages run.

```text
/// A document, whose header holds its length and version, with a nested
/// table of columns of the same length
message Document (sized, version: 1) {
    x: float64;
    y: float64;
    name: string;
    columns: Columns;
}

/// Columns with a value per key
table Columns {
    /// The value that `keys` count from
    base: int64;
    keys: [uint32];
    a: [float32];
    b: [float32];
}
```

`schemas/` at the root of the repository holds example schemas, which use
every part of the language.

## Schemas

A schema declares messages, tables and structs, in any order:

- A **message** is a table that's sent on its own and read from its bytes.
  Its attributes set its layout, as the [format](../../SPEC.md) describes
  it: `sized` puts the table's length in its header, `version: <n>` puts a
  version there, which readers check, and `offset_size: 8`, `16` or `32`, the
  default, sets the width of offsets and lengths, which limits the table to
  255 bytes, 64 KiB or 4 GiB.
- A **table** is nested in a message or another table, as a field of its type
  or in a vector of them. It has no header, and 32-bit offsets.
- A **struct** holds scalars at fixed offsets, without a header, offsets or
  padding, such as the frames of a stream.

Each field is `name: type;`, of one of these types:

| Type                                                                     | Field                     |
| ------------------------------------------------------------------------ | ------------------------- |
| `int8`, `uint8`, `int16`, `uint16`, `int32`, `uint32`, `int64`, `uint64` | An integer                |
| `float32`, `float64`                                                     | A float                   |
| `[int8]` … `[float64]`                                                   | An array of a scalar type |
| `string`                                                                 | A UTF-8 string            |
| A table's name                                                           | A nested table            |
| `[` a table's name `]`                                                   | A vector of tables        |

Every field is required, and is read by its index, so its position is part of
the format: a message that changes its fields needs another version.

`///` comments document the declaration or field after them, and the
generated code carries them, with Markdown's code spans in each language's
markup. `//` comments are left out.

The grammar, where `NAME` is ASCII letters, digits and `_` that don't start
with a digit, and `INTEGER` is decimal digits:

```text
schema      = declaration*
declaration = doc* ("message" | "table" | "struct") NAME attributes? "{" field* "}"
attributes  = "(" (attribute ("," attribute)* ","?)? ")"
attribute   = NAME (":" INTEGER)?
field       = doc* NAME ":" type ";"
type        = NAME | "[" NAME "]"
```

### Rules

The checker rejects a schema that breaks a rule, since buffy couldn't write
or read its tables, or a target couldn't generate its code:

- Declarations are `PascalCase` and fields `snake_case`, in ASCII. Each name
  is declared once, and a declaration can't take a built-in type's name.
- Messages, tables and structs hold at least one field.
- Only messages have attributes, each at most once.
- A version fits in the message's offsets: 255 for 8-bit offsets, 65,535 for
  16-bit and 4,294,967,295 for 32-bit.
- Messages can't be nested; declare a table to nest it.
- A table can't contain itself, except in a vector, which can be empty.
- A message with 8- or 16-bit offsets can't nest tables, since readers read a
  nested table's offsets as wide as its message's, and tables have 32-bit
  offsets.
- Structs hold scalars only, and tables can't hold structs yet. Arrays of
  strings aren't supported.

It doesn't check yet for names that clash with a target's own: a declaration
named after a class that the generated code imports, such as `List` in Java,
or a field named after a member that it generates, such as `new` in Rust or
`builder` in Python.

Errors point at their cause, as compilers print them, and the checker reports
every error it finds, after the first syntax error if there's one:

```text
error: unknown type `flaot64`
 --> point.buffy:2:8
  |
2 |     x: flaot64;
  |        ^^^^^^^ did you mean `float64`?
```

## Generated code

Messages and tables read their fields lazily from the bytes, and are written
from their fields in one call; tables nested from builders are written
straight into their parent. Structs are read and written whole. A versioned
message's reader fails with the runtime's decode error when the bytes are in
another version.

| Target     | Files                                    | Messages and tables                                                      | Structs                                              |
| ---------- | ---------------------------------------- | ------------------------------------------------------------------------ | ---------------------------------------------------- |
| TypeScript | `<schema>.ts`, in camelCase              | Classes, written from interfaces of their fields                         | Classes, read eagerly                                |
| JavaScript | `<schema>.js`, an ES module with `JSDoc` | As in TypeScript, with `@typedef`s of their fields                       | As in TypeScript                                     |
| Python     | `<schema>.py`                            | Classes, written from `TypedDict`s of their fields as keyword arguments  | Frozen dataclasses, read and written with `struct`   |
| Java       | `<package>/<Name>.java`                  | Final classes, written from a nested `Fields` record                     | Records, read and written in place with `VarHandle`s |
| Rust       | `<schema>.rs`                            | `Copy` views that borrow strings and arrays, written from `<Name>Fields` | Plain structs, read and written as arrays of bytes   |

Field names that are keywords in a target take a `_` after them, such as
`class_` in Python and Java, or are raw identifiers, such as `r#type` in Rust.

Each target's code is formatted as its formatter in this repository leaves
it: Biome for TypeScript and JavaScript, Ruff for Python and rustfmt for
Rust, with Java at 100 columns. With the schema above, from
`schemas/shapes.buffy`, the generated code reads and writes a document like
this:

```ts
import { Document } from './shapes.ts';

const bytes = Document.serialize({
    x: 1.5,
    y: 2,
    name: 'a',
    columns: { base: 0n, keys: [1, 2], a: [0.5, 1.5], b: [2.5, 3.5] }
});
const document = new Document(bytes); // checks the version
document.columns().a(); // a Float32Array
```

```python
from shapes import Document

data = Document.serialize(
    x=1.5,
    y=2,
    name="a",
    columns={"base": 0, "keys": [1, 2], "a": [0.5, 1.5], "b": [2.5, 3.5]},
)
document = Document(data)  # checks the version
document.columns().a()  # a memoryview of floats
```

```java
var columns = new Columns.Fields(
    0, new long[] {1, 2}, new float[] {0.5f, 1.5f}, new float[] {2.5f, 3.5f});
byte[] bytes = Document.serialize(new Document.Fields(1.5, 2, "a", columns));
var document = new Document(bytes); // checks the version
document.columns().a(); // a float[]
```

```rust
# mod shapes {
#     include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/shapes/rust/shapes.rs"));
# }
use shapes::{ColumnsFields, Document, DocumentFields};

let columns = ColumnsFields { base: 0, keys: &[1, 2], a: &[0.5, 1.5], b: &[2.5, 3.5] };
let bytes = DocumentFields { x: 1.5, y: 2.0, name: "a", columns }.serialize()?;

let document = Document::new(&bytes)?; // checks the version
assert_eq!(document.columns()?.a()?.to_vec(), [0.5, 1.5]); // from an ArrayView<f32>
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Command

```sh
buffy-gen <typescript|javascript|python|java|rust> <schema>... [--out <dir>] [--runtime <module>] [--package <package>]
```

`ts`, `js`, `py` and `rs` abbreviate the targets. The command writes each
schema's files under `--out`, which is the current directory by default, and
names the schema by its path in their header. It prints the errors of the
schemas that have any, and then fails.

- `--runtime` names the module, package or crate that the generated code
  imports buffy's runtime from, by default:

  | Target                 | Runtime                    |
  | ---------------------- | -------------------------- |
  | TypeScript, JavaScript | `@buffy/core`              |
  | Python                 | `buffy_core`               |
  | Java                   | `io.github.snovader.buffy` |
  | Rust                   | `buffy_core`               |

- `--package` names the package of the Java classes, which is `generated` by
  default, since they belong in a package of the project that uses them,
  which the generator can't know. Other targets ignore it.

From this repository, `cargo run -p buffy-gen --` runs the command.

## Library

`compile()` generates a schema's files, and `parse()` only checks it. Both
return the schema's errors as `Diagnostic`s, which `Diagnostic::render_all()`
renders as the command prints them.

```rust
use buffy_gen::{Diagnostic, Options, Target, compile};

let source = "message Point { x: float64; y: float64; }";
let files = compile("point.buffy", source, Target::Python, &Options::default())
    .expect("the schema is valid");
assert_eq!(files[0].path, "point.py");

let source = "message Point { x: flaot64; }";
let errors = compile("point.buffy", source, Target::Rust, &Options::default())
    .expect_err("the schema has an error");
assert!(Diagnostic::render_all(&errors, "point.buffy", source)
    .starts_with("error: unknown type `flaot64`\n --> point.buffy:1:20\n"));
```

Each target has a Cargo feature, `typescript` (which generates JavaScript
too), `python`, `java` and `rust`, and the default features are all four. A
crate that generates one language from its build script can depend on that
feature alone.

## WebAssembly

```sh
rustup target add wasm32-unknown-unknown
cargo build -p buffy-gen --lib --target wasm32-unknown-unknown --profile wasm
```

The build writes `target/wasm32-unknown-unknown/wasm/buffy_gen.wasm`, of
about 190 KB. It has no imports, so any host runs it with an empty import
object, without WASI or bindings. The `wasm` profile in the workspace's
`Cargo.toml` optimizes for size and aborts on a panic.

| Export                                            | Does                                                              |
| ------------------------------------------------- | ----------------------------------------------------------------- |
| `memory`                                          | The module's memory, which holds requests and responses           |
| `buffy_alloc(length: i32) -> i32`                 | Allocates `length` bytes for a request, and returns their address |
| `buffy_compile(request: i32, length: i32) -> i32` | Compiles a request, frees it, and returns the response's address  |
| `buffy_free(pointer: i32, length: i32)`           | Frees a response, whose length is in its first 4 bytes            |

Every string is its byte length as a little-endian `u32`, then its UTF-8
bytes.

- A request is five strings: the target's name, the schema's file name, its
  source, the runtime and the Java package. An empty runtime or package
  takes the default.
- A response is its own length as a `u32`, a `u32` status, and then, for
  status 0, a `u32` count of files and each file's path and contents, or, for
  status 1, the errors as the command prints them. An unknown target and a
  malformed request are errors too.

Allocating and compiling may grow the memory, which detaches a host's views
of it, so a host makes its views after each call. `tests/wasm/check.mjs`
runs the module in Node.

## Development

Run the commands of `rust/README.md` from `rust/`. The tests are:

- `tests/golden.rs`: the code generated from each example schema in
  `schemas/`, for each target, matches the files under
  `tests/golden/<schema>/<target>/`.
- `tests/diagnostics.rs`: each invalid schema in `tests/diagnostics/` fails
  with the errors in the `.stderr` file beside it.
- `tests/properties.rs`: any input parses or fails with errors that render,
  any schema that the checker accepts generates code for every target, and
  the command writes and prints what the library generates and finds.
- `tests/cli.rs` and `tests/compile.rs`: the command and the library's
  options.
- `tests/generated_rust.rs`: compiles the golden Rust, so that clippy and
  rustfmt check it.

After a change to the generator or the schemas, rewrite the golden files and
the expected errors, and review their diff:

```sh
BUFFY_GEN_UPDATE=1 cargo test -p buffy-gen
```

CI also builds the WebAssembly module, and checks that it has no imports and
generates the golden files and the expected errors:

```sh
node gen/tests/wasm/check.mjs target/wasm32-unknown-unknown/wasm/buffy_gen.wasm
```
