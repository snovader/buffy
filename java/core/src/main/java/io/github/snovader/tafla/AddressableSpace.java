package io.github.snovader.tafla;

/** The width of a table's header slots, field offsets and length prefixes */
public enum AddressableSpace {
    SIZEOF_8(1),
    SIZEOF_16(2),
    SIZEOF_32(4);

    private final int value;

    AddressableSpace(int value) {
        this.value = value;
    }

    /** {@return the width in bytes} */
    public int valueOf() {
        return this.value;
    }

    /** {@return the largest unsigned value that the width holds} */
    public long maxValue() {
        return (1L << (value * Byte.SIZE)) - 1;
    }
}
