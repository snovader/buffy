from typing import Literal

# A table's offset type: the unsigned integer type of its header slots, field
# offsets, counts, lengths and item ends, named as SPEC.md and schemas name it
type OffsetType = Literal["uint8", "uint16", "uint32"]

# The offset size: the width in bytes of an offset type, 1, 2 or 4
type OffsetSize = Literal[1, 2, 4]

type ElementSize = Literal[1, 2, 4, 8]

# The array module's type codes of the elements that tables store
type IntTypeCode = Literal["b", "B", "h", "H", "i", "I", "q", "Q"]
type FloatTypeCode = Literal["f", "d"]
type TypeCode = IntTypeCode | FloatTypeCode
