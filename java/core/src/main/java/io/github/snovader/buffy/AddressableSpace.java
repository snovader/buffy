package io.github.snovader.buffy;

/** The width of a table's header slots, field offsets and length prefixes */
public enum AddressableSpace {
    /** 1 byte, for tables of up to 255 bytes */
    SIZEOF_8(1),
    /** 2 bytes, for tables of up to 65,535 bytes */
    SIZEOF_16(2),
    /**
     * 4 bytes, for tables of up to 4 GiB in the format, and as long as a Java array can be in Java,
     * about 2 GiB
     */
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
