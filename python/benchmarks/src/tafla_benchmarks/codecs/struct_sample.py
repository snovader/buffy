from collections.abc import Buffer
from struct import Struct

from tafla_benchmarks.codecs.codec import Workload
from tafla_benchmarks.workloads import SampleInput

_SAMPLE = Struct("<Iff")


def serialize_sample(input: SampleInput) -> bytes:
    return _SAMPLE.pack(input.key, input.a, input.b)


def deserialize_sample(data: Buffer) -> SampleInput:
    key, a, b = _SAMPLE.unpack(data)

    return SampleInput(key, a, b)


STRUCT_SAMPLE = Workload(serialize_sample, deserialize_sample)
"""The fixed-size record in the standard library's ``struct``, which tafla's
documentation advises for records at fixed offsets: the same 12 bytes as
tafla's, packed and unpacked in one call with a precompiled format"""
