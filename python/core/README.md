# tafla

Binary tables of scalars, strings and typed arrays, in tafla's format.
`TableBuilder` writes a table at its exact size, copying typed arrays in bulk,
and `ByteBuffer` reads fields back by offset, with arrays as views of the
bytes. Every read checks that it stays within the bytes it was given.

The package's docstring, `help(tafla)`, has an example, which
`tests/test_docs.py` runs.
