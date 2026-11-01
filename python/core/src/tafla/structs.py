import struct

int8 = struct.Struct("<b")
uint8 = struct.Struct("<B")

int16 = struct.Struct("<h")
uint16 = struct.Struct("<H")

int32 = struct.Struct("<i")
uint32 = struct.Struct("<I")

int64 = struct.Struct("<q")
uint64 = struct.Struct("<Q")

float32 = struct.Struct("<f")
float64 = struct.Struct("<d")
