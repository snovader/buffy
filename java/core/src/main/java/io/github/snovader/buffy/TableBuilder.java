package io.github.snovader.buffy;

import org.jspecify.annotations.*;

import java.util.*;
import java.util.function.*;

/**
 * Builds a table: a header with one offset per field, after the table's total length and version
 * when its layout has them, followed by the fields in the order they're set. Arrays, strings and
 * vectors are length-prefixed, and typed arrays are padded so their elements are aligned from the
 * start of the table. Fields are read back by index with {@link ByteBuffer#rootOffset}, or
 * {@link ByteBuffer#fieldOffset} for a table nested in another.
 *
 * <p>Setting a field records it and its position. {@link #bytes()} then allocates the table at its
 * exact size and writes every field in one pass, so array elements must not change in between.
 * Tables nested from their builders are written in the same pass, straight into this one.</p>
 *
 * <p>A setter throws, and leaves the builder as it was, when the table already holds all its
 * fields, or when its field would make the table longer than the layout's offset size addresses:
 * 255 bytes, 64 KiB, or for 32-bit offsets, as long as a Java array can be.</p>
 */
public final class TableBuilder {
    /**
     * The longest array that the JDK's own collections allocate, which limits a table of 32-bit
     * offsets to about 2 GiB rather than their 4 GiB
     */
    private static final int MAX_ARRAY_LENGTH = Integer.MAX_VALUE - 8;

    private static final List<Consumer<ByteBuffer>> NO_WRITERS = List.of();

    private @Nullable ByteBuffer buffer;

    private final TableLayout layout;
    private final long version;
    /** The width of offsets, lengths and header slots */
    private final AddressableSpace size;
    private final int fields;
    /** The longest table that the layout's offsets and lengths address */
    private final long maxLength;
    /** The table's length so far, which is where the next field goes */
    private int offset;
    /**
     * The largest alignment that a field needs from the table's start, which a parent pads this
     * table to when it nests it
     */
    private int alignment = Byte.BYTES;

    // Per field: its position from the table's start, the padding before it, and what writes its
    // value. A writer captures a scalar without boxing it.
    private final int[] positions;
    private final int[] paddings;
    private List<Consumer<ByteBuffer>> writers;

    /**
     * Builds a table of {@link TableLayout#DEFAULT}, such as one to nest in another.
     *
     * @param fields the number of fields that the table will hold
     */
    public TableBuilder(int fields) {
        this(fields, TableLayout.DEFAULT);
    }

    /**
     * Builds a table of a layout without a version.
     *
     * @param fields the number of fields that the table will hold
     * @param layout the table's header and offset width, which its readers must use too
     * @throws IllegalArgumentException if the layout is versioned, or the header alone is longer
     *     than the layout addresses
     */
    public TableBuilder(int fields, TableLayout layout) {
        this(fields, layout, false, 0);
    }

    /**
     * Builds a table of a versioned layout, with its version in the header.
     *
     * @param fields the number of fields that the table will hold
     * @param layout the table's header and offset width, which its readers must use too
     * @param version the version to write to the header. Readers can check it with
     *     {@link ByteBuffer#getVersion()} before reading any field.
     * @throws IllegalArgumentException if the layout isn't versioned, the version doesn't fit in
     *     the layout's offset size, or the header alone is longer than the layout addresses
     */
    public TableBuilder(int fields, TableLayout layout, long version) {
        this(fields, layout, true, version);
    }

    private TableBuilder(int fields, TableLayout layout, boolean versioned, long version) {
        var size = layout.offsetSize();

        if (layout.versioned() != versioned) {
            throw new IllegalArgumentException(
                layout.versioned() ? "A versioned layout needs a version" : "The layout has no version slot"
            );
        }

        if (versioned && (version < 0 || version > size.maxValue())) {
            throw new IllegalArgumentException(
                String.format("Version %d doesn't fit in a %d-byte slot", version, size.valueOf())
            );
        }

        if (fields < 0) {
            throw new IllegalArgumentException(String.format("A table can't hold %d fields", fields));
        }

        this.layout = layout;
        this.version = version;
        this.size = size;
        this.fields = fields;
        this.maxLength = Math.min(size.maxValue(), MAX_ARRAY_LENGTH);

        var headerLength = layout.headerSize() + (long) fields * size.valueOf();
        if (headerLength > maxLength) {
            throw new IllegalArgumentException(
                String.format("A header of %d fields doesn't fit in %d-byte offsets", fields, size.valueOf())
            );
        }

        this.offset = (int) headerLength;
        this.positions = new int[fields];
        this.paddings = new int[fields];
        this.writers = new ArrayList<>(fields);
    }

    /**
     * Writes the table, once; later calls return the same bytes.
     *
     * @return the table's bytes
     * @throws IllegalStateException if fewer fields were set than the table holds
     */
    public byte[] bytes() {
        var buffer = this.buffer;
        if (buffer != null) {
            return buffer.bytes();
        }

        if (writers.size() != fields) {
            throw new IllegalStateException(
                String.format("Only %d of the table's %d fields were set", writers.size(), fields)
            );
        }

        buffer = new ByteBuffer(new byte[offset], layout);
        write(buffer);

        // Release the values, which may be large arrays
        this.writers = NO_WRITERS;
        this.buffer = buffer;

        return buffer.bytes();
    }

    /**
     * Sets the next field to an {@code int8}.
     *
     * @param value the field's value
     */
    public void setInt8(byte value) {
        addField(Byte.BYTES, 0, buffer -> buffer.setInt8(value));
    }

    /**
     * Sets the next field to a {@code uint8}, the low 8 bits of {@code value}.
     *
     * @param value the field's value
     */
    public void setUint8(short value) {
        addField(Byte.BYTES, 0, buffer -> buffer.setUint8(value));
    }

    /**
     * Sets the next field to an {@code int16}.
     *
     * @param value the field's value
     */
    public void setInt16(short value) {
        addField(Short.BYTES, 0, buffer -> buffer.setInt16(value));
    }

    /**
     * Sets the next field to a {@code uint16}, the low 16 bits of {@code value}.
     *
     * @param value the field's value
     */
    public void setUint16(int value) {
        addField(Short.BYTES, 0, buffer -> buffer.setUint16(value));
    }

    /**
     * Sets the next field to an {@code int32}.
     *
     * @param value the field's value
     */
    public void setInt32(int value) {
        addField(Integer.BYTES, 0, buffer -> buffer.setInt32(value));
    }

    /**
     * Sets the next field to a {@code uint32}, the low 32 bits of {@code value}.
     *
     * @param value the field's value
     */
    public void setUint32(long value) {
        addField(Integer.BYTES, 0, buffer -> buffer.setUint32(value));
    }

    /**
     * Sets the next field to an {@code int64}.
     *
     * @param value the field's value
     */
    public void setInt64(long value) {
        addField(Long.BYTES, 0, buffer -> buffer.setInt64(value));
    }

    /**
     * Sets the next field to a {@code uint64}, from a {@code long} with its bits.
     *
     * @param value the field's value
     */
    public void setUint64(long value) {
        addField(Long.BYTES, 0, buffer -> buffer.setUint64(value));
    }

    /**
     * Sets the next field to a {@code float32}.
     *
     * @param value the field's value
     */
    public void setFloat32(float value) {
        addField(Float.BYTES, 0, buffer -> buffer.setFloat32(value));
    }

    /**
     * Sets the next field to a {@code float64}.
     *
     * @param value the field's value
     */
    public void setFloat64(double value) {
        addField(Double.BYTES, 0, buffer -> buffer.setFloat64(value));
    }

    /**
     * Sets the next field to an array of {@code int8}s. The table copies {@code values} when it's
     * written, so they must not change until then.
     *
     * @param values the array's elements
     */
    public void setInt8Array(byte[] values) {
        addArray(values.length, Byte.BYTES, buffer -> buffer.setInt8Array(values));
    }

    /**
     * Sets the next field to an array of {@code uint8}s, the low 8 bits of each value. The table
     * copies {@code values} when it's written, so they must not change until then.
     *
     * @param values the array's elements
     */
    public void setUint8Array(short[] values) {
        addArray(values.length, Byte.BYTES, buffer -> buffer.setUint8Array(values));
    }

    /**
     * Sets the next field to an array of {@code int16}s. The table copies {@code values} when it's
     * written, so they must not change until then.
     *
     * @param values the array's elements
     */
    public void setInt16Array(short[] values) {
        addArray(values.length, Short.BYTES, buffer -> buffer.setInt16Array(values));
    }

    /**
     * Sets the next field to an array of {@code uint16}s, the low 16 bits of each value. The table
     * copies {@code values} when it's written, so they must not change until then.
     *
     * @param values the array's elements
     */
    public void setUint16Array(int[] values) {
        addArray(values.length, Short.BYTES, buffer -> buffer.setUint16Array(values));
    }

    /**
     * Sets the next field to an array of {@code int32}s. The table copies {@code values} when it's
     * written, so they must not change until then.
     *
     * @param values the array's elements
     */
    public void setInt32Array(int[] values) {
        addArray(values.length, Integer.BYTES, buffer -> buffer.setInt32Array(values));
    }

    /**
     * Sets the next field to an array of {@code uint32}s, the low 32 bits of each value. The table
     * copies {@code values} when it's written, so they must not change until then.
     *
     * @param values the array's elements
     */
    public void setUint32Array(long[] values) {
        addArray(values.length, Integer.BYTES, buffer -> buffer.setUint32Array(values));
    }

    /**
     * Sets the next field to an array of {@code int64}s. The table copies {@code values} when it's
     * written, so they must not change until then.
     *
     * @param values the array's elements
     */
    public void setInt64Array(long[] values) {
        addArray(values.length, Long.BYTES, buffer -> buffer.setInt64Array(values));
    }

    /**
     * Sets the next field to an array of {@code uint64}s, from {@code long}s with their bits. The
     * table copies {@code values} when it's written, so they must not change until then.
     *
     * @param values the array's elements
     */
    public void setUint64Array(long[] values) {
        addArray(values.length, Long.BYTES, buffer -> buffer.setUint64Array(values));
    }

    /**
     * Sets the next field to an array of {@code float32}s. The table copies {@code values} when
     * it's written, so they must not change until then.
     *
     * @param values the array's elements
     */
    public void setFloat32Array(float[] values) {
        addArray(values.length, Float.BYTES, buffer -> buffer.setFloat32Array(values));
    }

    /**
     * Sets the next field to an array of {@code float64}s. The table copies {@code values} when
     * it's written, so they must not change until then.
     *
     * @param values the array's elements
     */
    public void setFloat64Array(double[] values) {
        addArray(values.length, Double.BYTES, buffer -> buffer.setFloat64Array(values));
    }

    /**
     * Sets the next field to {@code string} as UTF-8, with lone surrogates replaced by U+FFFD.
     *
     * @param string the field's value
     */
    public void setString(String string) {
        var utf8 = Utf8.encode(string);
        addField(size.valueOf() + (long) utf8.length, 0, buffer -> buffer.setString(utf8));
    }

    /**
     * Embeds a table from its builder, padded to the largest element size of its arrays and the
     * alignment of its nested tables, or not at all if it has neither.
     *
     * @param table the nested table's builder
     * @throws IllegalArgumentException if the builder has fields left to set
     * @see #setTable(TableBuilder, int)
     */
    public void setTable(TableBuilder table) {
        setTable(table, table.alignment);
    }

    /**
     * Embeds a table from its builder, without a length prefix. Readers find its fields with
     * {@link ByteBuffer#fieldOffset} from the field's offset, so its layout must be
     * {@link TableLayout#DEFAULT}.
     *
     * <p>The builder must have all its fields set. {@link #bytes()} writes it straight into this
     * table, without bytes of its own, so its arrays must not change until then either. It can
     * still be written itself or nested elsewhere too, since a complete builder doesn't change.</p>
     *
     * @param table the nested table's builder
     * @param alignment what the table's start is padded to a multiple of: 1, 2, 4 or 8
     * @throws IllegalArgumentException if the builder has fields left to set
     */
    public void setTable(TableBuilder table, int alignment) {
        var padding = paddingTo(offset, alignment);
        addField(lengthOf(table), padding, buffer -> writeTable(buffer, table));
        alignTo(alignment);
    }

    /**
     * Embeds a table from its bytes, padded to start at a multiple of 4, which suits tables of 8-,
     * 16- and 32-bit elements.
     *
     * @param table the nested table's bytes
     * @see #setTable(byte[], int)
     */
    public void setTable(byte[] table) {
        setTable(table, Integer.BYTES);
    }

    /**
     * Embeds a table from its bytes, without a length prefix. Readers find its fields with
     * {@link ByteBuffer#fieldOffset} from the field's offset, so its layout must be
     * {@link TableLayout#DEFAULT}.
     *
     * <p>The nested table's arrays are aligned from its own start, so it's padded to start at a
     * multiple of {@code alignment}, which keeps them aligned in this table too. Pass 8 for a table
     * with 64-bit arrays.</p>
     *
     * @param table the nested table's bytes
     * @param alignment what the table's start is padded to a multiple of: 1, 2, 4 or 8
     */
    public void setTable(byte[] table, int alignment) {
        var padding = paddingTo(offset, alignment);
        addField(table.length, padding, buffer -> buffer.setBytes(table));
        alignTo(alignment);
    }

    /**
     * Writes a vector of tables from their builders, each with a length prefix. Builders must have
     * all their fields set, and are written straight into this table as
     * {@link #setTable(TableBuilder)} writes them, except for those whose layout has another
     * offset size, which are written to their own bytes and copied. Items aren't padded.
     *
     * @param tables the builder of each item
     * @throws IllegalArgumentException if a builder has fields left to set
     */
    public void setVector(List<TableBuilder> tables) {
        // A copy, so that the items can't change before the table is written
        var items = List.copyOf(tables);

        var byteLength = size.valueOf() + (long) items.size() * size.valueOf();
        for (var item : items) {
            byteLength += lengthOf(item);
        }

        addField(byteLength, 0, buffer -> writeVector(buffer, items));
    }

    /**
     * Writes a vector of tables from their bytes, each with a length prefix.
     *
     * @param tables the bytes of each item
     */
    public void setVector(byte[][] tables) {
        var items = tables.clone();

        var byteLength = size.valueOf() + (long) items.length * size.valueOf();
        for (var item : items) {
            byteLength += item.length;
        }

        addField(byteLength, 0, buffer -> buffer.setVector(items));
    }

    /** Pads the array so that its elements, after the length, are aligned */
    private void addArray(int length, int elementSize, Consumer<ByteBuffer> writer) {
        var padding = paddingTo(offset + size.valueOf(), elementSize);
        addField(size.valueOf() + (long) length * elementSize, padding, writer);
        alignTo(elementSize);
    }

    private void alignTo(int alignment) {
        this.alignment = Math.max(this.alignment, alignment);
    }

    /**
     * The byte length of a table to nest, which a builder knows once all its fields are set
     *
     * @throws IllegalArgumentException if the builder has fields left to set
     */
    private static int lengthOf(TableBuilder table) {
        // A finalized builder had all its fields set, and released its writers
        if (table.buffer == null && table.writers.size() != table.fields) {
            throw new IllegalArgumentException(
                String.format(
                    "Only %d of the nested table's %d fields were set",
                    table.writers.size(),
                    table.fields
                )
            );
        }

        return table.offset;
    }

    /**
     * Records a field of {@code byteLength} bytes after {@code padding} bytes. Checking the
     * table's length here, rather than in {@link #bytes()}, reports a field too long for the layout
     * from its own setter and leaves the builder as it was. Every offset and length is shorter
     * than the table, so the table's length is the one value to check.
     *
     * @throws IllegalStateException if the table is already written, or holds all its fields
     * @throws IllegalArgumentException if the field would make the table longer than the layout
     *     addresses
     */
    private void addField(long byteLength, int padding, Consumer<ByteBuffer> writer) {
        if (buffer != null) {
            throw new IllegalStateException("The table was already written");
        }

        var index = writers.size();
        if (index == fields) {
            throw new IllegalStateException(
                String.format("Can't add field %d to a table of %d", fields + 1, fields)
            );
        }

        var length = offset + padding + byteLength;
        if (length > maxLength) {
            throw new IllegalArgumentException(tooLong(index, length));
        }

        positions[index] = offset + padding;
        paddings[index] = padding;
        writers.add(writer);

        offset = (int) length;
    }

    private String tooLong(int index, long length) {
        if (maxLength == MAX_ARRAY_LENGTH) {
            return String.format(
                "Field %d makes the table %d bytes, longer than a Java array can be",
                index + 1,
                length
            );
        }

        return String.format(
            "Field %d makes the table %d bytes, too long for %d-byte offsets",
            index + 1,
            length,
            size.valueOf()
        );
    }

    /**
     * Writes the table from the buffer's position, which counts as its start. The buffer's offsets
     * must be as wide as the table's.
     */
    private void write(ByteBuffer buffer) {
        if (layout.sized()) {
            buffer.setSize(offset);
        }

        if (layout.versioned()) {
            buffer.setVersion(version);
        }

        for (int i = 0; i < fields; i++) {
            buffer.setOffset(positions[i]);
        }

        for (int i = 0; i < fields; i++) {
            if (paddings[i] > 0) {
                buffer.setPadding(paddings[i]);
            }

            writers.get(i).accept(buffer);
        }
    }

    private void writeVector(ByteBuffer buffer, List<TableBuilder> vector) {
        buffer.setLength(vector.size());

        for (var table : vector) {
            buffer.setLength(table.offset);
            writeTable(buffer, table);
        }
    }

    /**
     * Writes a builder straight into the buffer, unless it was written already or has offsets of
     * another width than the buffer, which writes every offset and length at its own width. Those
     * tables are copied.
     */
    private void writeTable(ByteBuffer buffer, TableBuilder table) {
        if (table.buffer == null && table.size == size) {
            table.write(buffer);
        } else {
            buffer.setBytes(table.bytes());
        }
    }

    /**
     * The padding that brings {@code offset} to a multiple of {@code alignment}
     *
     * @throws IllegalArgumentException if the alignment isn't 1, 2, 4 or 8
     */
    private static int paddingTo(int offset, int alignment) {
        if (alignment != 1 && alignment != 2 && alignment != 4 && alignment != 8) {
            throw new IllegalArgumentException(
                String.format("An alignment of %d isn't 1, 2, 4 or 8 bytes", alignment)
            );
        }

        var remainder = offset % alignment;
        return remainder == 0 ? 0 : alignment - remainder;
    }
}
