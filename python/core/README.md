# tafla

Binary tables of scalars, strings and typed arrays, in tafla's format.
`TableBuilder` writes a table at its exact size, copying typed arrays in bulk,
and `ByteBuffer` reads fields back by offset, with arrays as views of the
bytes. Every read checks that it stays within the bytes it was given.
`TableWriter` writes a table whose fields are known as it's written, as the
code that tafla-gen generates writes its tables, without recording them first
as `TableBuilder` does.

The package's docstring, `help(tafla)`, has an example, which the tests run
as a doctest.
