package io.github.snovader.tafla;

import org.jspecify.annotations.*;

import java.util.*;
import java.util.function.*;

/**
 * Builds a table: a header with one offset per field, after the table's total length and version
 * when its layout has them, followed by the fields in the order they're set. Arrays, strings and
 * vectors start with their count, and a vector then holds the end of each item. Arrays of scalars
 * and of structs are padded so that their elements are aligned from the start of the table. Fields
 * are read back by index with {@link ByteBuffer#rootOffset}, or {@link ByteBuffer#fieldOffset} for
 * a table nested in another.
 *
 * <p>Setting a field records it and its position. {@link #bytes()} then allocates the table at its
 * exact size and writes every field in one pass, so array elements must not change in between.
 * Tables nested from their builders are written in the same pass, straight into this one.</p>
 *
 * <p>A setter throws, and leaves the builder as it was, when the table already holds all its
 * fields, or when its field would make the table longer than the layout's offset type addresses:
 * 255 bytes, 64 KiB, or for {@code uint32} offsets, as long as a Java array can be.</p>
 */
public final class TableBuilder {
    private static final Consumer<ByteBuffer>[] NO_WRITERS = newWriters(0);

    private @Nullable ByteBuffer buffer;

    private final TableLayout layout;
    private final long version;
    /** The type of offsets, lengths and header slots */
    private final OffsetType offsetType;
    private int fieldsSet;
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
    private Consumer<ByteBuffer>[] writers;

    /** Builds a table of {@link TableLayout#DEFAULT}, such as one to nest in another */
    public TableBuilder(int fields) {
        this(fields, TableLayout.DEFAULT);
    }

    /**
     * @param fields the number of fields that the table will hold
     * @param layout the table's header and offset type, which its readers must use too
     * @throws IllegalArgumentException if the layout is versioned, or the header alone is longer
     *     than the layout addresses
     */
    public TableBuilder(int fields, TableLayout layout) {
        this(fields, layout, false, 0);
    }

    /**
     * @param version the version to write to the header. Readers can check it with
     *     {@link ByteBuffer#getVersion()} before reading any field.
     * @throws IllegalArgumentException if the layout isn't versioned, the version doesn't fit in
     *     the layout's offset type, or the header alone is longer than the layout addresses
     */
    public TableBuilder(int fields, TableLayout layout, long version) {
        this(fields, layout, true, version);
    }

    private TableBuilder(int fields, TableLayout layout, boolean versioned, long version) {
        this.offset = layout.headerLength(fields, versioned, version);
        this.layout = layout;
        this.version = version;
        this.offsetType = layout.offsetType();
        this.maxLength = layout.maxLength();

        this.positions = new int[fields];
        this.paddings = new int[fields];
        this.writers = newWriters(fields);
    }

    /**
     * Writes the table, once; later calls return the same bytes.
     *
     * @throws IllegalStateException if fewer fields were set than the table holds
     */
    public byte[] bytes() {
        var buffer = this.buffer;
        if (buffer != null) {
            return buffer.bytes();
        }

        if (fieldsSet != positions.length) {
            throw new IllegalStateException(
                String.format("Only %d of the table's %d fields were set", fieldsSet, positions.length)
            );
        }

        buffer = new ByteBuffer(new byte[offset], layout);
        write(buffer);

        // Release the values, which may be large arrays
        this.writers = NO_WRITERS;
        this.buffer = buffer;

        return buffer.bytes();
    }

    public void setInt8(byte value) {
        addField(Byte.BYTES, 0, buffer -> buffer.setInt8(value));
    }

    public void setUint8(short value) {
        addField(Byte.BYTES, 0, buffer -> buffer.setUint8(value));
    }

    public void setInt16(short value) {
        addField(Short.BYTES, 0, buffer -> buffer.setInt16(value));
    }

    public void setUint16(int value) {
        addField(Short.BYTES, 0, buffer -> buffer.setUint16(value));
    }

    public void setInt32(int value) {
        addField(Integer.BYTES, 0, buffer -> buffer.setInt32(value));
    }

    public void setUint32(long value) {
        addField(Integer.BYTES, 0, buffer -> buffer.setUint32(value));
    }

    public void setInt64(long value) {
        addField(Long.BYTES, 0, buffer -> buffer.setInt64(value));
    }

    /** Sets a {@code uint64} from a {@code long} with its bits */
    public void setUint64(long value) {
        addField(Long.BYTES, 0, buffer -> buffer.setUint64(value));
    }

    public void setFloat32(float value) {
        addField(Float.BYTES, 0, buffer -> buffer.setFloat32(value));
    }

    public void setFloat64(double value) {
        addField(Double.BYTES, 0, buffer -> buffer.setFloat64(value));
    }

    public void setInt8Array(byte[] values) {
        addArray(values.length, Byte.BYTES, buffer -> buffer.setInt8Array(values));
    }

    public void setUint8Array(short[] values) {
        addArray(values.length, Byte.BYTES, buffer -> buffer.setUint8Array(values));
    }

    public void setInt16Array(short[] values) {
        addArray(values.length, Short.BYTES, buffer -> buffer.setInt16Array(values));
    }

    public void setUint16Array(int[] values) {
        addArray(values.length, Short.BYTES, buffer -> buffer.setUint16Array(values));
    }

    public void setInt32Array(int[] values) {
        addArray(values.length, Integer.BYTES, buffer -> buffer.setInt32Array(values));
    }

    public void setUint32Array(long[] values) {
        addArray(values.length, Integer.BYTES, buffer -> buffer.setUint32Array(values));
    }

    public void setInt64Array(long[] values) {
        addArray(values.length, Long.BYTES, buffer -> buffer.setInt64Array(values));
    }

    /** Sets {@code uint64}s from {@code long}s with their bits */
    public void setUint64Array(long[] values) {
        addArray(values.length, Long.BYTES, buffer -> buffer.setUint64Array(values));
    }

    public void setFloat32Array(float[] values) {
        addArray(values.length, Float.BYTES, buffer -> buffer.setFloat32Array(values));
    }

    public void setFloat64Array(double[] values) {
        addArray(values.length, Double.BYTES, buffer -> buffer.setFloat64Array(values));
    }

    /** Writes {@code string} as UTF-8, with lone surrogates replaced by U+FFFD */
    public void setString(String string) {
        var utf8 = Utf8.encode(string);
        addField(offsetType.size() + (long) utf8.length, 0, buffer -> buffer.setString(utf8));
    }

    /**
     * Sets a struct field from its bytes: its scalars, little-endian, one after another, which are
     * written as they are, without padding. {@link ByteBuffer#getStruct} reads them back.
     */
    public void setStruct(byte[] bytes) {
        addField(bytes.length, 0, buffer -> buffer.setBytes(bytes));
    }

    /**
     * Sets an array of structs of {@code size} bytes each, from their bytes one after another,
     * after their count. It's padded so that the structs start at a multiple of {@code alignment}
     * from the start of the table, which is the size of the structs' largest scalar.
     * {@link ByteBuffer#getStructArray} reads them back.
     *
     * @param alignment what the structs' start is padded to a multiple of: 1, 2, 4 or 8
     * @throws IllegalArgumentException if {@code size} is less than 1, {@code bytes} aren't a whole
     *     number of structs, or the alignment isn't 1, 2, 4 or 8
     */
    public void setStructArray(byte[] bytes, int size, int alignment) {
        ByteBuffer.structCount(bytes.length, size);
        addElements(bytes.length, alignment, buffer -> buffer.setStructArray(bytes, size));
    }

    /**
     * Embeds a table from its builder, padded to the largest alignment of its arrays, arrays of
     * structs and nested tables, or not at all if it has none of them.
     *
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
     * @param alignment what the table's start is padded to a multiple of: 1, 2, 4 or 8
     * @throws IllegalArgumentException if the builder has fields left to set
     */
    public void setTable(TableBuilder table, int alignment) {
        var padding = TableLayout.padding(offset, alignment);
        addField(lengthOf(table), padding, buffer -> writeTable(buffer, table));
        alignTo(alignment);
    }

    /**
     * Embeds a table from its bytes, padded to start at a multiple of 4, which suits tables of 8-,
     * 16- and 32-bit elements.
     *
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
     * @param alignment what the table's start is padded to a multiple of: 1, 2, 4 or 8
     */
    public void setTable(byte[] table, int alignment) {
        var padding = TableLayout.padding(offset, alignment);
        addField(table.length, padding, buffer -> buffer.setBytes(table));
        alignTo(alignment);
    }

    /**
     * Writes a vector of tables from their builders, after the end of each, which lets readers find
     * any item without reading the others. Builders must have all their fields set, and are written
     * straight into this table as {@link #setTable(TableBuilder)} writes them, except for those
     * whose layout has another offset type, which are written to their own bytes and copied. Items
     * aren't padded.
     *
     * @throws IllegalArgumentException if a builder has fields left to set
     */
    public void setVector(List<TableBuilder> tables) {
        // A copy, so that the items can't change before the table is written
        var items = List.copyOf(tables);

        var byteLength = offsetType.size() + (long) items.size() * offsetType.size();
        for (var item : items) {
            byteLength += lengthOf(item);
        }

        addField(byteLength, 0, buffer -> writeVector(buffer, items));
    }

    /** Writes a vector of tables from their bytes, after the end of each */
    public void setVector(byte[][] tables) {
        var items = tables.clone();

        var byteLength = offsetType.size() + (long) items.length * offsetType.size();
        for (var item : items) {
            byteLength += item.length;
        }

        addField(byteLength, 0, buffer -> buffer.setVector(items));
    }

    @SuppressWarnings("unchecked")
    private static Consumer<ByteBuffer>[] newWriters(int fields) {
        return (Consumer<ByteBuffer>[]) new Consumer<?>[fields];
    }

    /** Pads the array so that its elements, after the count, are aligned to their size */
    private void addArray(int length, int elementSize, Consumer<ByteBuffer> writer) {
        addElements((long) length * elementSize, elementSize, writer);
    }

    /**
     * Records a count and then {@code byteLength} bytes of elements, padded so that the elements
     * start at a multiple of {@code alignment}
     *
     * @throws IllegalArgumentException if the alignment isn't 1, 2, 4 or 8
     */
    private void addElements(long byteLength, int alignment, Consumer<ByteBuffer> writer) {
        var padding = TableLayout.padding(offset + offsetType.size(), alignment);
        addField(offsetType.size() + byteLength, padding, writer);
        alignTo(alignment);
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
        if (table.buffer == null && table.fieldsSet != table.positions.length) {
            throw new IllegalArgumentException(
                String.format(
                    "Only %d of the nested table's %d fields were set",
                    table.fieldsSet,
                    table.positions.length
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

        var index = fieldsSet;
        if (index == positions.length) {
            throw new IllegalStateException(
                String.format("Can't add field %d to a table of %d", positions.length + 1, positions.length)
            );
        }

        var length = offset + padding + byteLength;
        if (length > maxLength) {
            throw new IllegalArgumentException(tooLong(index, length));
        }

        positions[index] = offset + padding;
        paddings[index] = padding;
        writers[index] = writer;
        fieldsSet = index + 1;

        offset = (int) length;
    }

    private String tooLong(int index, long length) {
        if (maxLength == TableLayout.MAX_ARRAY_LENGTH) {
            return String.format(
                "Field %d makes the table %d bytes, longer than a Java array can be",
                index + 1,
                length
            );
        }

        return String.format(
            "Field %d makes the table %d bytes, too long for %s offsets",
            index + 1,
            length,
            offsetType
        );
    }

    /**
     * Writes the table from the buffer's position, which counts as its start. The buffer's layout
     * must have the table's offset type.
     */
    private void write(ByteBuffer buffer) {
        if (layout.sized()) {
            buffer.setSize(offset);
        }

        if (layout.versioned()) {
            buffer.setVersion(version);
        }

        for (int i = 0; i < positions.length; i++) {
            buffer.setOffset(positions[i]);
        }

        for (int i = 0; i < positions.length; i++) {
            if (paddings[i] > 0) {
                buffer.setPadding(paddings[i]);
            }

            writers[i].accept(buffer);
        }
    }

    private void writeVector(ByteBuffer buffer, List<TableBuilder> vector) {
        buffer.setLength(vector.size());

        // Each end counts from the vector's position, which is its count's
        var end = (vector.size() + 1) * offsetType.size();
        for (var table : vector) {
            end += table.offset;
            buffer.setLength(end);
        }

        for (var table : vector) {
            writeTable(buffer, table);
        }
    }

    /**
     * Writes a builder straight into the buffer, unless it was written already or has another
     * offset type than the buffer, which writes every offset and length in its own type. Those
     * tables are copied.
     */
    private void writeTable(ByteBuffer buffer, TableBuilder table) {
        if (table.buffer == null && table.offsetType == offsetType) {
            table.write(buffer);
        } else {
            buffer.setBytes(table.bytes());
        }
    }
}
