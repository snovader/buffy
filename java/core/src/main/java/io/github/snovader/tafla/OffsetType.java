package io.github.snovader.tafla;

/**
 * A table's offset type: the unsigned integer type of its header slots, field offsets, counts,
 * lengths and item ends, named as SPEC.md and schemas name it, which limits how long the table can
 * be. Its offset size is its width in bytes.
 */
public enum OffsetType {
    /** {@code uint8}, for tables of up to 255 bytes */
    UINT8(1),
    /** {@code uint16}, for tables of up to 65,535 bytes */
    UINT16(2),
    /** {@code uint32}, for tables of up to 4 GiB */
    UINT32(4);

    private final int size;

    OffsetType(int size) {
        this.size = size;
    }

    /** {@return the offset size, the type's width in bytes: 1, 2 or 4} */
    public int size() {
        return this.size;
    }

    /** {@return the largest value of the type, the longest table that its offsets address} */
    public long maxValue() {
        return (1L << (size * Byte.SIZE)) - 1;
    }

    /** {@return the type's name as SPEC.md spells it: {@code "uint8"}, {@code "uint16"} or {@code "uint32"}} */
    @Override
    public String toString() {
        return switch (this) {
            case UINT8 -> "uint8";
            case UINT16 -> "uint16";
            case UINT32 -> "uint32";
        };
    }
}
