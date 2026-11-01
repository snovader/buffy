"""The benchmarks: every workload through every codec, in the TypeScript
comparison's groups and order.

A variant prepares its input when its worker first times it, after checking
that its codec reads back what it wrote, so that each worker prepares only the
variant that it runs.
"""

from collections.abc import Buffer, Callable, Sequence
from dataclasses import dataclass
from functools import cache, partial
from time import perf_counter
from typing import Any

from buffy_benchmarks.check import serialize_checked
from buffy_benchmarks.codecs import CODECS, DocumentWorkload, Workload, samples
from buffy_benchmarks.workloads import (
    DOCUMENT_ROWS,
    LIST_ITEMS,
    RECORD,
    SAMPLE,
    DocumentInput,
    document_input,
    document_lists,
    list_input,
)


@dataclass(frozen=True, slots=True)
class Call[T]:
    """An operation and the argument that it's timed with"""

    operation: Callable[[T], object]
    argument: T


@dataclass(frozen=True, slots=True)
class Variant:
    """One codec's benchmark in a group"""

    codec: str
    prepare: Callable[[], Call[Any]]
    """Returns the call to time, after checking the codec's round trip"""


@dataclass(frozen=True, slots=True)
class Group:
    """The codecs' variants of one operation on one workload"""

    name: str
    variants: tuple[Variant, ...]


def groups() -> list[Group]:
    """Returns the groups in the TypeScript comparison's order, with the
    documents of list columns last, as its documents of ``number[]`` are"""
    return [
        *_round_trip_groups(
            "record", [(c.name, c.record) for c in CODECS], lambda: RECORD
        ),
        *(group for rows in DOCUMENT_ROWS for group in _document_groups(rows)),
        *_round_trip_groups(
            f"vector of {LIST_ITEMS} nested tables",
            [(c.name, c.list) for c in CODECS],
            partial(list_input, LIST_ITEMS),
        ),
        *_round_trip_groups(
            "fixed-size record", list(samples().items()), lambda: SAMPLE
        ),
        *(_document_lists_group(rows) for rows in DOCUMENT_ROWS),
    ]


def _round_trip_groups[I](
    name: str,
    workloads: Sequence[tuple[str, Workload[I, Any]]],
    input: Callable[[], I],
) -> list[Group]:
    serialize = [
        Variant(codec, partial(_serializing, workload, input))
        for codec, workload in workloads
    ]
    deserialize = [
        Variant(codec, partial(_deserializing, workload, input))
        for codec, workload in workloads
    ]

    return [
        Group(f"{name}: serialize", tuple(serialize)),
        Group(f"{name}: deserialize", tuple(deserialize)),
    ]


def _document_groups(rows: int) -> list[Group]:
    name = _document_name(rows)
    input = partial(document_input, rows)
    summing = [
        Variant(codec.name, partial(_summing, codec.document, input))
        for codec in CODECS
    ]

    return [
        *_round_trip_groups(name, [(c.name, c.document) for c in CODECS], input),
        Group(f"{name}: sum a column per element", tuple(summing)),
    ]


def _document_lists_group(rows: int) -> Group:
    """Serializes the document from list columns, as parsed JSON would have"""
    input = partial(_document_of_lists, rows)
    serialize = [
        Variant(codec.name, partial(_serializing, codec.document, input))
        for codec in CODECS
    ]

    return Group(f"{_document_name(rows)}: serialize lists", tuple(serialize))


def _document_name(rows: int) -> str:
    return f"document of {rows:,} rows"


def _document_of_lists(rows: int) -> DocumentInput:
    return document_lists(document_input(rows))


def _serializing[I](workload: Workload[I, Any], input: Callable[[], I]) -> Call[I]:
    value = input()
    serialize_checked(workload, value)

    return Call(workload.serialize, value)


def _deserializing[I](
    workload: Workload[I, Any], input: Callable[[], I]
) -> Call[Buffer]:
    return Call(workload.deserialize, serialize_checked(workload, input()))


def _summing(
    workload: DocumentWorkload, input: Callable[[], DocumentInput]
) -> Call[Buffer]:
    return Call(workload.sum_a, serialize_checked(workload, input()))


@cache
def _prepared(prepare: Callable[[], Call[Any]]) -> Call[Any]:
    return prepare()


def time_variant(loops: int, prepare: Callable[[], Call[Any]]) -> float:
    """Times ``loops`` calls of the variant's operation, in seconds, after
    preparing it on the first call. pyperf runs each variant in worker
    processes of its own, so CPython specializes this loop for one operation,
    and it frees each result before the next call, as a caller would."""
    call = _prepared(prepare)
    operation = call.operation
    argument = call.argument
    iterations = range(loops)

    start = perf_counter()
    for _ in iterations:
        operation(argument)

    return perf_counter() - start
