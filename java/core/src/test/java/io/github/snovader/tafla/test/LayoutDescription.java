package io.github.snovader.tafla.test;

import io.github.snovader.tafla.*;

/** The layout of a described table, as {@code cases.json} writes it out in full */
record LayoutDescription(boolean sized, boolean versioned, OffsetSize offsetSize) {
    /** {@return the layout with neither header slot and offsets of {@code offsetSize}} */
    static LayoutDescription headerless(OffsetSize offsetSize) {
        return new LayoutDescription(false, false, offsetSize);
    }

    TableLayout tableLayout() {
        return TableLayout.builder().sized(sized).versioned(versioned).offsetSize(offsetSize).build();
    }

    /** {@return the offset size: the width of offsets, header slots and length prefixes in bytes} */
    int width() {
        return offsetSize.bytes();
    }

    /** {@return the byte length of the header slots before the field offsets} */
    int headerSize() {
        return ((sized ? 1 : 0) + (versioned ? 1 : 0)) * width();
    }

    @Override
    public String toString() {
        return DescriptionJson.write(this).toString();
    }
}
