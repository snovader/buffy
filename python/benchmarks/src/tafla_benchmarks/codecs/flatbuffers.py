from collections.abc import Buffer

import flatbuffers
import numpy as np

from tafla_benchmarks.codecs.codec import Codec, DocumentWorkload, Workload
from tafla_benchmarks.generated.flatbuffers import (
    Columns,
    Document,
    Item,
    List,
    Pair,
    Record,
    Sample,
)
from tafla_benchmarks.workloads import (
    DocumentInput,
    DocumentOutput,
    ItemInput,
    ListInput,
    RecordInput,
    SampleInput,
)


def serialize_record(input: RecordInput) -> bytes:
    builder = flatbuffers.Builder()
    name = builder.CreateString(input.name)

    Record.Start(builder)
    Record.AddX(builder, input.x)
    Record.AddY(builder, input.y)
    Record.AddId(builder, input.id)
    Record.AddName(builder, name)
    builder.Finish(Record.End(builder))

    return builder.Output()


def deserialize_record(data: Buffer) -> RecordInput:
    record = Record.Record.GetRootAs(data, 0)

    return RecordInput(record.X(), record.Y(), record.Id(), record.Name().decode())


def serialize_document(input: DocumentInput) -> bytes:
    """Columns are copied in bulk from numpy arrays, which views an array of
    the column's type without a copy and converts a list"""
    builder = flatbuffers.Builder()
    keys = builder.CreateNumpyVector(np.asarray(input.keys, dtype=np.uint32))
    a = builder.CreateNumpyVector(np.asarray(input.a, dtype=np.float32))
    b = builder.CreateNumpyVector(np.asarray(input.b, dtype=np.float32))

    Columns.Start(builder)
    Columns.AddBase(builder, input.base)
    Columns.AddKeys(builder, keys)
    Columns.AddA(builder, a)
    Columns.AddB(builder, b)
    columns = Columns.End(builder)
    name = builder.CreateString(input.name)

    Document.Start(builder)
    Document.AddX(builder, input.x)
    Document.AddY(builder, input.y)
    Document.AddName(builder, name)
    Document.AddColumns(builder, columns)
    builder.Finish(Document.End(builder))

    return builder.Output()


def deserialize_document(data: Buffer) -> DocumentOutput:
    """Reads the columns as numpy arrays that view the bytes"""
    document = Document.Document.GetRootAs(data, 0)
    columns = document.Columns()

    return DocumentOutput(
        document.X(),
        document.Y(),
        document.Name().decode(),
        columns.Base(),
        columns.KeysAsNumpy(),
        columns.AAsNumpy(),
        columns.BAsNumpy(),
    )


def sum_a(data: Buffer) -> float:
    columns = Document.Document.GetRootAs(data, 0).Columns()

    total = 0.0
    for i in range(columns.ALength()):
        total += columns.A(i)

    return total


def serialize_list(input: ListInput) -> bytes:
    builder = flatbuffers.Builder()
    offsets = [_serialize_item(builder, item) for item in input.items]

    # Builders write back to front, so the last item is prepended first
    List.StartItemsVector(builder, len(offsets))
    for offset in reversed(offsets):
        builder.PrependUOffsetTRelative(offset)
    items = builder.EndVector()

    List.Start(builder)
    List.AddItems(builder, items)
    builder.Finish(List.End(builder))

    return builder.Output()


def _serialize_item(builder: flatbuffers.Builder, input: ItemInput) -> int:
    Pair.Start(builder)
    Pair.AddFirst(builder, input.first)
    Pair.AddSecond(builder, input.second)
    pair = Pair.End(builder)

    Item.Start(builder)
    Item.AddValue(builder, input.value)
    Item.AddPair(builder, pair)

    return Item.End(builder)


def deserialize_list(data: Buffer) -> ListInput:
    vector = List.List.GetRootAs(data, 0)

    items: list[ItemInput] = []
    for i in range(vector.ItemsLength()):
        item = vector.Items(i)
        pair = item.Pair()
        items.append(ItemInput(item.Value(), pair.First(), pair.Second()))

    return ListInput(items)


def serialize_sample(input: SampleInput) -> bytes:
    builder = flatbuffers.Builder()

    Sample.Start(builder)
    Sample.AddKey(builder, input.key)
    Sample.AddA(builder, input.a)
    Sample.AddB(builder, input.b)
    builder.Finish(Sample.End(builder))

    return builder.Output()


def deserialize_sample(data: Buffer) -> SampleInput:
    sample = Sample.Sample.GetRootAs(data, 0)

    return SampleInput(sample.Key(), sample.A(), sample.B())


FLATBUFFERS = Codec(
    name="flatbuffers",
    record=Workload(serialize_record, deserialize_record),
    document=DocumentWorkload(serialize_document, deserialize_document, sum_a),
    list=Workload(serialize_list, deserialize_list),
    sample=Workload(serialize_sample, deserialize_sample),
)
"""flatc's generated code, used as its documentation shows: a builder of the
default size, tables built field by field and read with the generated
accessors. Columns are written with ``CreateNumpyVector()`` and read with the
generated ``AsNumpy()`` accessors, which is how FlatBuffers' Python runtime
copies vectors in bulk; without numpy it has only a call per element."""
