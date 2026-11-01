# buffy for Python

`core/` holds `buffy-core`, the runtime, whose package is `buffy_core`.

`benchmarks/` compares buffy with FlatBuffers, protobuf, MessagePack and
`struct` in pyperf benchmarks, on the same workloads as the TypeScript
comparison. Its [README](benchmarks/README.md) describes them and has results.
It isn't published, and its libraries are its own dependencies, not
`buffy-core`'s.

## Development

This is a uv workspace. Run the commands below from this directory; `uv run`
creates the environment from `uv.lock` first.

```sh
uv run ruff format --check
uv run ruff check
uv run ty check
uv run python -m unittest discover -s core/tests -t core
uv run python -m unittest discover -s benchmarks/tests -t benchmarks
```

The benchmarks' tests check that their workloads have the TypeScript
comparison's values, and that every codec in the comparison reads back what it
wrote. `uv run --python 3.15 ...` runs the commands with another Python. Where
uv would pick a free-threaded build, `--python 3.14+gil` picks one with the
GIL.

`core/tests/generated` holds the Python that buffy-gen generates from the
example schemas in `schemas/`, in a directory of each schema, and the tests
that run it. They're the generator's golden files, which `rust/gen/README.md`
describes: rewrite them with the generator rather than by hand.
