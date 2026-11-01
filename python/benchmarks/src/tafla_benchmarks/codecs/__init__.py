"""Each library's encoding of the workloads, with tafla first as the reference
that the others are compared with"""

from tafla_benchmarks.codecs.codec import Codec, DocumentWorkload, Workload
from tafla_benchmarks.codecs.flatbuffers import FLATBUFFERS
from tafla_benchmarks.codecs.msgpack import MSGPACK
from tafla_benchmarks.codecs.protobuf import PROTOBUF
from tafla_benchmarks.codecs.struct_sample import STRUCT_SAMPLE
from tafla_benchmarks.codecs.tafla import TAFLA
from tafla_benchmarks.workloads import SampleInput

CODECS: tuple[Codec, ...] = (TAFLA, FLATBUFFERS, PROTOBUF, MSGPACK)

STRUCT = "struct"
"""The name of the standard library's encoding of the fixed-size record"""


def samples() -> dict[str, Workload[SampleInput, SampleInput]]:
    """Returns the encodings of the fixed-size record by name: every codec's,
    and the standard library's ``struct``, which encodes only records at fixed
    offsets"""
    return {codec.name: codec.sample for codec in CODECS} | {STRUCT: STRUCT_SAMPLE}


__all__ = [
    "TAFLA",
    "CODECS",
    "Codec",
    "DocumentWorkload",
    "FLATBUFFERS",
    "MSGPACK",
    "PROTOBUF",
    "STRUCT",
    "Workload",
    "samples",
]
