# Benchmarks

`yarn bench` times `tafla`'s getters and setters of every element type
on their own, and `yarn bench:compare` compares the package with other
JavaScript libraries. Run both from `typescript/`.

## Comparison

The comparison times tafla and other JavaScript libraries that encode small
tables and typed numeric columns, on generic table shapes:

- **record**: one table of two float64s, an int64 and a 16-byte string.
- **document of 100 and 100,000 rows**: a sized, versioned table of two
  float64s and a string, with a nested table of an int64 and three columns, a
  Uint32 and two Float32.
- **vector of 100 nested tables**: a table holding a vector of tables, each a
  float32 and a nested table of two float32s.
- **array of 100 structs**: a table holding the same items as an array of
  structs of their three float32s, 12 bytes each, written with `StructWriter`
  and `setStructArray()` and read with `getStructArray()` and `readFloat32()`.
- **fixed-size record**: a uint32 and two float32s at fixed offsets, written
  with `StructWriter` and read with `readUint32()`/`readFloat32()`.

Each shape is serialized and deserialized, with columns read back as typed
arrays. Documents are also serialized from `number[]` columns, and a column is
summed through each library's per-element accessor. The suite also prints
encoded sizes and what each library does with a truncated document.

Each library writes the array of structs in its own fixed-size encoding:
FlatBuffers as a vector of structs, protobuf as a repeated message of the three
floats, which it writes as fixed32s, and msgpackr as an array of each struct's
fields in order, without their keys, in an array for the table, as rmp-serde's
`to_vec()` packs them. protobuf has no structs, and packed repeated floats
would make a column of each field, which the documents time already. msgpackr's
records extension, which `new Packr()` enables, already packs the vector's
items as their keys once and then their values, so its two shapes are about the
same size.

### Libraries

| Library               | Why it's comparable                                                                                               |
| --------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `flatbuffers` 25.9.23 | Zero-copy tables with offsets and vectors, like tafla. flatc's generated code is in `comparison/generated/`.      |
| `protobufjs` 8.8.1    | The most used protobuf runtime, with packed repeated floats. Decoding copies into `number[]`.                     |
| `msgpackr` 2.1.2      | A fast schemaless format that packs typed arrays as byte copies. The suite uses its plain JavaScript entry point. |

### Method

Each group runs its variants interleaved, in alternating order, over 31 rounds
by default, after 10 warm-up rounds, and reports the fastest call. Load from
other processes then slows every variant alike, and only for some rounds.
Every variant has its own timer loop, compiled from a unique source, so that
call sites and type feedback aren't shared. Groups with `number[]` inputs run
last, since they change the type feedback of code the typed-array groups
share. Before timing, every codec must read back exactly what it wrote.

### Running

```sh
uptime
yarn bench:compare --rounds 41
```

`--filter <regex>` runs only the matching groups, and `--sample-ms` sets how
long one round of one variant takes (1 ms by default). Check `uptime` first:
a high load from other processes makes even the minimum unreliable.

### Limitations

- Node has wider Smis than V8 builds with pointer compression, such as
  Cloudflare's workerd, so integer-heavy code can run differently there.
- protobufjs generates its codecs with `new Function`, which Workers don't
  allow. `pbjs` emits the same code as a static module for that.
- msgpackr packs successive messages into one shared buffer and returns views
  of it, so it allocates once for many messages. The other libraries allocate
  a buffer for each message.
