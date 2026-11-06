package io.github.snovader.tafla;

/**
 * A table's offset size: the width in bytes of its offset type, the unsigned integer type of its
 * header slots, field offsets, counts, lengths and item ends, which limits how long the table can
 * be. Each constant is named after its offset type.
 */
public enum OffsetSize {
    /** One byte, of {@code uint8} offsets, for tables of up to 255 bytes */
    UINT8(1),
    /** Two bytes, of {@code uint16} offsets, for tables of up to 65,535 bytes */
    UINT16(2),
    /** Four bytes, of {@code uint32} offsets, for tables of up to 4 GiB */
    UINT32(4);

    private final int bytes;

    OffsetSize(int bytes) {
        this.bytes = bytes;
    }

    /** {@return the offset size in bytes: 1, 2 or 4} */
    public int bytes() {
        return this.bytes;
    }

    /** {@return the largest value of the offset type, the longest table that its offsets address} */
    public long maxValue() {
        return (1L << (bytes * Byte.SIZE)) - 1;
    }
}
