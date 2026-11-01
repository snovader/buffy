from typing import Literal

type AddressableSpace = Literal[1, 2, 4]

type ElementSize = Literal[1, 2, 4, 8]

# The array module's type codes of the elements that tables store
type IntTypeCode = Literal["b", "B", "h", "H", "i", "I", "q", "Q"]
type FloatTypeCode = Literal["f", "d"]
type TypeCode = IntTypeCode | FloatTypeCode
