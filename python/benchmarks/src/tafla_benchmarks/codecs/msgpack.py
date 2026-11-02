from array import array
from collections.abc import Buffer, Sequence
from typing import Any

import msgpack

from tafla_benchmarks.codecs.codec import Codec, DocumentWorkload, Workload
from tafla_benchmarks.workloads import (
    DocumentInput,
    DocumentOutput,
    ItemInput,
    ListInput,
    RecordInput,
    SampleInput,
)

# The extension types of the arrays that columns pack as. Their elements are in
# the host's byte order, as msgpackr packs typed arrays.
_UINT32_ARRAY = 1
_FLOAT32_ARRAY = 2


def _pack_array(value: object) -> msgpack.ExtType:
    """Packs an array of a column's type as an extension of its bytes, as
    msgpack's documentation packs an array"""
    if isinstance(value, array):
        if value.typecode == "I":
            return msgpack.ExtType(_UINT32_ARRAY, value.tobytes())
        if value.typecode == "f":
            return msgpack.ExtType(_FLOAT32_ARRAY, value.tobytes())

    raise TypeError(f"Can't pack {type(value).__name__}")


def _unpack_array(ext_type: int, data: bytes) -> memoryview[Any] | msgpack.ExtType:
    """Reads a column's extension as a view of its bytes, which the unpacker
    copied out of the message. That took half the time of copying them again
    into an array, as msgpack's documentation does, for 100,000 rows."""
    if ext_type == _UINT32_ARRAY:
        return memoryview(data).cast("I")
    if ext_type == _FLOAT32_ARRAY:
        return memoryview(data).cast("f")

    return msgpack.ExtType(ext_type, data)


# One packer serves every message, as msgpackr's does. It returns each message
# as new bytes.
_packer = msgpack.Packer(default=_pack_array)


def _unpack(data: Buffer) -> Any:
    return msgpack.unpackb(data, ext_hook=_unpack_array)


def serialize_record(input: RecordInput) -> bytes:
    return _packer.pack(
        {"x": input.x, "y": input.y, "id": input.id, "name": input.name}
    )


def deserialize_record(data: Buffer) -> RecordInput:
    record = _unpack(data)
    return RecordInput(record["x"], record["y"], record["id"], record["name"])


def serialize_document(input: DocumentInput) -> bytes:
    """Packs the document as one map, with list columns converted to arrays
    first, as a caller who wants float32s on the wire would"""
    return _packer.pack(
        {
            "x": input.x,
            "y": input.y,
            "name": input.name,
            "base": input.base,
            "keys": _as_array("I", input.keys),
            "a": _as_array("f", input.a),
            "b": _as_array("f", input.b),
        }
    )


def _as_array[T: (int, float)](type_code: str, values: Sequence[T]) -> array[T]:
    if isinstance(values, array) and values.typecode == type_code:
        return values

    return array(type_code, values)


def deserialize_document(data: Buffer) -> DocumentOutput:
    """Reads the columns as the views that the extension hook returns"""
    document = _unpack(data)

    return DocumentOutput(
        document["x"],
        document["y"],
        document["name"],
        document["base"],
        document["keys"],
        document["a"],
        document["b"],
    )


def sum_a(data: Buffer) -> float:
    a = _unpack(data)["a"]

    total = 0.0
    for i in range(len(a)):
        total += a[i]

    return total


def serialize_list(input: ListInput) -> bytes:
    return _packer.pack({"items": [_item_map(item) for item in input.items]})


def _item_map(input: ItemInput) -> dict[str, float]:
    return {"value": input.value, "first": input.first, "second": input.second}


def deserialize_list(data: Buffer) -> ListInput:
    items = _unpack(data)["items"]
    return ListInput([ItemInput(i["value"], i["first"], i["second"]) for i in items])


def serialize_structs(input: ListInput) -> bytes:
    """Packs each struct as an array of its fields in order, without their
    keys, and the table that holds them as an array too, as rmp-serde's
    ``to_vec()`` packs both"""
    return _packer.pack([[(i.value, i.first, i.second) for i in input.items]])


def deserialize_structs(data: Buffer) -> ListInput:
    [items] = _unpack(data)
    return ListInput([ItemInput(*fields) for fields in items])


def serialize_sample(input: SampleInput) -> bytes:
    return _packer.pack({"key": input.key, "a": input.a, "b": input.b})


def deserialize_sample(data: Buffer) -> SampleInput:
    sample = _unpack(data)
    return SampleInput(sample["key"], sample["a"], sample["b"])


MSGPACK = Codec(
    name="msgpack",
    record=Workload(serialize_record, deserialize_record),
    document=DocumentWorkload(serialize_document, deserialize_document, sum_a),
    list=Workload(serialize_list, deserialize_list),
    structs=Workload(serialize_structs, deserialize_structs),
    sample=Workload(serialize_sample, deserialize_sample),
)
"""msgpack's packer and ``unpackb()``. MessagePack is schemaless: a table packs
as a map with its keys, as msgpackr packs an object, and floats as float64s,
which is how msgpack packs Python's floats. Columns pack as extensions of
their arrays' bytes, as msgpackr packs typed arrays, and read back as views of
the bytes that the unpacker copies out."""
