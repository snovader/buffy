package io.github.snovader.tafla;

import java.util.*;

/**
 * The header and offset width of a table. A format creates its layout once and passes it to both
 * the {@link TableBuilder} that writes the table and the {@link ByteBuffer} that reads it, so that
 * the two can't disagree. Nested tables use {@link #DEFAULT}.
 *
 * {@snippet :
 * // The response's length and format version, then its field offsets
 * static final TableLayout LAYOUT = TableLayout.builder()
 *     .sized(true)
 *     .versioned(true)
 *     .build();
 * }
 */
public final class TableLayout {
    /** No header slots, and 32-bit offsets and lengths */
    public static final TableLayout DEFAULT = new TableLayout(false, false, AddressableSpace.SIZEOF_32);

    private final boolean sized;
    private final boolean versioned;
    private final AddressableSpace offsetSize;
    private final int headerSize;

    private TableLayout(boolean sized, boolean versioned, AddressableSpace offsetSize) {
        this.sized = sized;
        this.versioned = versioned;
        this.offsetSize = offsetSize;

        var headerSlots = (sized ? 1 : 0) + (versioned ? 1 : 0);
        this.headerSize = headerSlots * offsetSize.valueOf();
    }

    /** {@return a builder of a layout, which starts as {@link #DEFAULT}} */
    public static Builder builder() {
        return new Builder();
    }

    /** {@return whether the header starts with the table's total byte length} */
    public boolean sized() {
        return sized;
    }

    /** {@return whether the header has a slot for a version, after the length when sized} */
    public boolean versioned() {
        return versioned;
    }

    /** {@return the width of the header slots, field offsets and length prefixes} */
    public AddressableSpace offsetSize() {
        return offsetSize;
    }

    /** {@return the byte length of the header slots before the field offsets} */
    public int headerSize() {
        return headerSize;
    }

    @Override
    public String toString() {
        return String.format(
            "TableLayout[sized=%s, versioned=%s, offsetSize=%s]",
            sized,
            versioned,
            offsetSize
        );
    }

    /**
     * Sets the options of a {@link TableLayout}, as the options object of the TypeScript
     * {@code TableLayout.create()} and the keyword arguments of the Python constructor do.
     */
    public static final class Builder {
        private boolean sized = false;
        private boolean versioned = false;
        private AddressableSpace offsetSize = AddressableSpace.SIZEOF_32;

        private Builder() {}

        /** Whether the header starts with the table's total byte length, which is false by default */
        public Builder sized(boolean sized) {
            this.sized = sized;
            return this;
        }

        /**
         * Whether the header has a slot for a version, after the length when sized, which is false by
         * default. Readers can check it before reading any field, and a {@link TableBuilder} of the
         * layout needs a version to write to it.
         */
        public Builder versioned(boolean versioned) {
            this.versioned = versioned;
            return this;
        }

        /**
         * The width of the header slots, field offsets and length prefixes, which limits the table's
         * size: {@link AddressableSpace#SIZEOF_8} for up to 255 bytes,
         * {@link AddressableSpace#SIZEOF_16} for 65,535, and {@link AddressableSpace#SIZEOF_32}, the
         * default, for as long as a Java array can be, about 2 GiB.
         */
        public Builder offsetSize(AddressableSpace offsetSize) {
            this.offsetSize = Objects.requireNonNull(offsetSize);
            return this;
        }

        public TableLayout build() {
            return new TableLayout(sized, versioned, offsetSize);
        }
    }
}
