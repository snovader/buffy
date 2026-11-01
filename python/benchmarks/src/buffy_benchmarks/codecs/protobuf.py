from collections.abc import Buffer

from buffy_benchmarks.codecs.codec import Codec, DocumentWorkload, Workload
from buffy_benchmarks.generated import messages_pb2
from buffy_benchmarks.workloads import (
    DocumentInput,
    DocumentOutput,
    ItemInput,
    ListInput,
    RecordInput,
    SampleInput,
)


def serialize_record(input: RecordInput) -> bytes:
    return messages_pb2.Record(
        x=input.x, y=input.y, id=input.id, name=input.name
    ).SerializeToString()


def deserialize_record(data: Buffer) -> RecordInput:
    record = messages_pb2.Record.FromString(data)

    return RecordInput(record.x, record.y, record.id, record.name)


def serialize_document(input: DocumentInput) -> bytes:
    document = messages_pb2.Document(x=input.x, y=input.y, name=input.name)

    columns = document.columns
    columns.base = input.base
    columns.keys.extend(input.keys)
    columns.a.extend(input.a)
    columns.b.extend(input.b)

    return document.SerializeToString()


def deserialize_document(data: Buffer) -> DocumentOutput:
    """Reads the columns as the message's repeated fields"""
    document = messages_pb2.Document.FromString(data)
    columns = document.columns

    return DocumentOutput(
        document.x,
        document.y,
        document.name,
        columns.base,
        columns.keys,
        columns.a,
        columns.b,
    )


def sum_a(data: Buffer) -> float:
    a = messages_pb2.Document.FromString(data).columns.a

    total = 0.0
    for i in range(len(a)):
        total += a[i]

    return total


def serialize_list(input: ListInput) -> bytes:
    vector = messages_pb2.List()

    items = vector.items
    for item in input.items:
        pair = items.add(value=item.value).pair
        pair.first = item.first
        pair.second = item.second

    return vector.SerializeToString()


def deserialize_list(data: Buffer) -> ListInput:
    items: list[ItemInput] = []
    for item in messages_pb2.List.FromString(data).items:
        pair = item.pair
        items.append(ItemInput(item.value, pair.first, pair.second))

    return ListInput(items)


def serialize_sample(input: SampleInput) -> bytes:
    return messages_pb2.Sample(key=input.key, a=input.a, b=input.b).SerializeToString()


def deserialize_sample(data: Buffer) -> SampleInput:
    sample = messages_pb2.Sample.FromString(data)

    return SampleInput(sample.key, sample.a, sample.b)


PROTOBUF = Codec(
    name="protobuf",
    record=Workload(serialize_record, deserialize_record),
    document=DocumentWorkload(serialize_document, deserialize_document, sum_a),
    list=Workload(serialize_list, deserialize_list),
    sample=Workload(serialize_sample, deserialize_sample),
)
"""protoc's generated messages, parsed with ``FromString()``, in the upb runtime
that protobuf's wheels use. Messages are built from their fields, and nested
messages in place, as protobuf's tutorial builds them: passing a nested message
to its parent's constructor copies it, which made writing the vector of nested
tables 2.6 times slower, and the large document 13% slower. Repeated fields
are converted element by element when a message is built, and read back as
the message's containers."""
