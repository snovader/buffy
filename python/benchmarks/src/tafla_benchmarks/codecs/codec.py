from collections.abc import Buffer, Callable
from dataclasses import dataclass

from tafla_benchmarks.workloads import (
    DocumentInput,
    DocumentOutput,
    ListInput,
    RecordInput,
    SampleInput,
)


@dataclass(frozen=True, slots=True)
class Workload[I, O]:
    """One library's encoding of one of the workloads' shapes"""

    serialize: Callable[[I], Buffer]
    """Returns the input's bytes, which the caller owns"""

    deserialize: Callable[[Buffer], O]
    """Reads every field, with columns as the library returns them"""


@dataclass(frozen=True, slots=True)
class DocumentWorkload(Workload[DocumentInput, DocumentOutput]):
    sum_a: Callable[[Buffer], float]
    """Sums column ``a`` through the library's own per-element accessor"""


@dataclass(frozen=True, slots=True)
class Codec:
    """One library's encoding of the workloads"""

    name: str
    record: Workload[RecordInput, RecordInput]
    document: DocumentWorkload
    list: Workload[ListInput, ListInput]
    structs: Workload[ListInput, ListInput]
    """The list's items as an array of structs, in the library's fixed-size
    encoding"""
    sample: Workload[SampleInput, SampleInput]
