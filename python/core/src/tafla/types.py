from typing import Literal

# A table's offset size: the width in bytes of its offset type, uint8, uint16 or
# uint32, the unsigned integer type of its header slots, field offsets, counts,
# lengths and item ends
type OffsetSize = Literal[1, 2, 4]

type ElementSize = Literal[1, 2, 4, 8]

# The array module's type codes of the elements that tables store
type IntTypeCode = Literal["b", "B", "h", "H", "i", "I", "q", "Q"]
type FloatTypeCode = Literal["f", "d"]
type TypeCode = IntTypeCode | FloatTypeCode
