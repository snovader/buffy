from collections.abc import Buffer

from tafla import SIZEOF_32, ByteBuffer, TableBuilder, TableLayout
from tafla_benchmarks.codecs.codec import Codec, DocumentWorkload, Workload
from tafla_benchmarks.workloads import (
    DocumentInput,
    DocumentOutput,
    ItemInput,
    ListInput,
    RecordInput,
    SampleInput,
)

DOCUMENT_LAYOUT = TableLayout(sized=True, versioned=True)
DOCUMENT_VERSION = 1

SAMPLE_SIZE = 3 * SIZEOF_32


def serialize_record(input: RecordInput) -> memoryview:
    builder = TableBuilder(4)
    builder.set_float64(input.x)
    builder.set_float64(input.y)
    builder.set_int64(input.id)
    builder.set_string(input.name)

    return builder.bytes()


def deserialize_record(data: Buffer) -> RecordInput:
    buffer = ByteBuffer(data)

    return RecordInput(
        buffer.get_float64(buffer.root_offset(0)),
        buffer.get_float64(buffer.root_offset(1)),
        buffer.get_int64(buffer.root_offset(2)),
        buffer.get_string(buffer.root_offset(3)),
    )


def serialize_document(input: DocumentInput) -> memoryview:
    """Arrays of the columns' element types are copied as bytes, and lists are
    converted as ``array()`` converts them"""
    columns = TableBuilder(4)
    columns.set_int64(input.base)
    columns.set_uint32_array(input.keys)
    columns.set_float32_array(input.a)
    columns.set_float32_array(input.b)

    document = TableBuilder(4, DOCUMENT_LAYOUT, DOCUMENT_VERSION)
    document.set_float64(input.x)
    document.set_float64(input.y)
    document.set_string(input.name)
    document.set_table(columns)

    return document.bytes()


def deserialize_document(data: Buffer) -> DocumentOutput:
    buffer = _open_document(data)
    columns = buffer.root_offset(3)

    return DocumentOutput(
        buffer.get_float64(buffer.root_offset(0)),
        buffer.get_float64(buffer.root_offset(1)),
        buffer.get_string(buffer.root_offset(2)),
        buffer.get_int64(buffer.field_offset(columns, 0)),
        buffer.get_uint32_array(buffer.field_offset(columns, 1)),
        buffer.get_float32_array(buffer.field_offset(columns, 2)),
        buffer.get_float32_array(buffer.field_offset(columns, 3)),
    )


def sum_a(data: Buffer) -> float:
    """Sums the elements of the column's view, as tafla's documentation advises
    over a getter call per element"""
    buffer = _open_document(data)
    a = buffer.get_float32_array(buffer.field_offset(buffer.root_offset(3), 2))

    total = 0.0
    for i in range(len(a)):
        total += a[i]

    return total


def _open_document(data: Buffer) -> ByteBuffer:
    """Raises ``ValueError`` if the document is in another version"""
    buffer = ByteBuffer(data, DOCUMENT_LAYOUT)
    if buffer.get_version() != DOCUMENT_VERSION:
        raise ValueError(f"Unsupported document version {buffer.get_version()}")

    return buffer


def serialize_list(input: ListInput) -> memoryview:
    builder = TableBuilder(1)
    builder.set_vector([_item_table(item) for item in input.items])

    return builder.bytes()


def _item_table(input: ItemInput) -> TableBuilder:
    pair = TableBuilder(2)
    pair.set_float32(input.first)
    pair.set_float32(input.second)

    item = TableBuilder(2)
    item.set_float32(input.value)
    item.set_table(pair)

    return item


def deserialize_list(data: Buffer) -> ListInput:
    buffer = ByteBuffer(data)
    offsets = buffer.get_vector_offsets(buffer.root_offset(0))

    items: list[ItemInput] = []
    for item in offsets:
        pair = buffer.field_offset(item, 1)
        items.append(
            ItemInput(
                buffer.get_float32(buffer.field_offset(item, 0)),
                buffer.get_float32(buffer.field_offset(pair, 0)),
                buffer.get_float32(buffer.field_offset(pair, 1)),
            )
        )

    return ListInput(items)


def serialize_sample(input: SampleInput) -> memoryview:
    """Writes the fields one after another with ``ByteBuffer``'s setters, since
    tafla's Python runtime has no writer of fixed-size records"""
    buffer = ByteBuffer(bytearray(SAMPLE_SIZE))
    buffer.set_uint32(input.key)
    buffer.set_float32(input.a)
    buffer.set_float32(input.b)

    return buffer.bytes()


def deserialize_sample(data: Buffer) -> SampleInput:
    buffer = ByteBuffer(data)

    return SampleInput(
        buffer.get_uint32(0),
        buffer.get_float32(SIZEOF_32),
        buffer.get_float32(2 * SIZEOF_32),
    )


TAFLA = Codec(
    name="tafla",
    record=Workload(serialize_record, deserialize_record),
    document=DocumentWorkload(serialize_document, deserialize_document, sum_a),
    list=Workload(serialize_list, deserialize_list),
    sample=Workload(serialize_sample, deserialize_sample),
)
"""tafla's tables, nested from their builders, with columns read back as
views. The fixed-size record is written with ``ByteBuffer``'s setters and read
with its getters at fixed positions."""
