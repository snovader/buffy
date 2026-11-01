# buffy for Python

`core/` holds `buffy-core`, the runtime, whose package is `buffy_core`.

## Development

This is a uv workspace. Run the commands below from this directory; `uv run`
creates the environment from `uv.lock` first.

```sh
uv run ruff format --check
uv run ruff check
uv run ty check
uv run python -m unittest discover -s core/tests -t core
```

`uv run --python 3.15 ...` runs them with another Python. Where uv would pick
a free-threaded build, `--python 3.14+gil` picks one with the GIL.
