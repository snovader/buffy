"""Checks that a codec reads back exactly what it wrote, so that the benchmarks
can't time a codec that skips or corrupts part of the work"""

from array import array
from collections.abc import Buffer, Callable, Iterable
from functools import singledispatch
from struct import Struct

from tafla_benchmarks.codecs import Workload
from tafla_benchmarks.workloads import (
    DocumentInput,
    DocumentOutput,
    ListInput,
    RecordInput,
    SampleInput,
)

_FLOAT64 = Struct("<d")

type Comparable = dict[str, object]


class RoundTripError(Exception):
    """A codec read back other values than it wrote"""


def serialize_checked[I, O](workload: Workload[I, O], input: I) -> Buffer:
    """Returns the input's bytes after checking that the workload reads them
    back as the input's values.

    Raises ``RoundTripError`` if it doesn't.
    """
    data = workload.serialize(input)
    expected = comparable(input)
    actual = comparable(workload.deserialize(data))

    if actual != expected:
        fields = [name for name in expected if actual.get(name) != expected[name]]
        raise RoundTripError(
            f"{workload.deserialize.__module__} didn't read back "
            f"{', '.join(fields)} of the {type(input).__name__}"
        )

    return data


def check_sum(read: Callable[[Buffer], float], data: Buffer, expected: float) -> None:
    """Checks that a read of the list's items sums their float32s as expected,
    by the bits of the sum.

    Raises ``RoundTripError`` if it doesn't.
    """
    actual = read(data)
    if _float_bits(actual) != _float_bits(expected):
        raise RoundTripError(f"{read!r} summed {actual}, not {expected}")


@singledispatch
def comparable(value: object) -> Comparable:
    """Returns a workload's values in a form that compares equal only when
    they're exactly the same: floats by their bits, so that -0.0 differs from
    0.0, and columns element by element, whatever their type."""
    raise TypeError(f"Can't compare a {type(value).__name__}")


@comparable.register
def _comparable_record(record: RecordInput) -> Comparable:
    return {
        "x": _float_bits(record.x),
        "y": _float_bits(record.y),
        "id": record.id,
        "name": record.name,
    }


@comparable.register
def _comparable_document(document: DocumentInput | DocumentOutput) -> Comparable:
    return {
        "x": _float_bits(document.x),
        "y": _float_bits(document.y),
        "name": document.name,
        "base": document.base,
        "keys": array("q", document.keys),
        "a": _float_column_bits(document.a),
        "b": _float_column_bits(document.b),
    }


@comparable.register
def _comparable_list(vector: ListInput) -> Comparable:
    items = [
        (_float_bits(item.value), _float_bits(item.first), _float_bits(item.second))
        for item in vector.items
    ]

    return {"items": items}


@comparable.register
def _comparable_sample(sample: SampleInput) -> Comparable:
    return {"key": sample.key, "a": _float_bits(sample.a), "b": _float_bits(sample.b)}


def _float_bits(value: float) -> bytes:
    return _FLOAT64.pack(value)


def _float_column_bits(values: Iterable[float]) -> bytes:
    """Widening float32s to float64s keeps their values and signs exactly"""
    return array("d", values).tobytes()
