package io.github.snovader.buffy;

import java.nio.*;
import java.nio.charset.*;
import java.util.*;

/**
 * Reads and writes the supplied bytes in the table layout it was created with, storing multi-byte
 * values little-endian. Array reads copy the elements into a new Java array.
 *
 * <p>Every read checks that it stays within the bytes, and throws a {@link DecodeException}
 * otherwise. Writes past the end throw an {@link IndexOutOfBoundsException}, and lengths, offsets
 * and versions too large for the layout's offset size an {@link IllegalArgumentException}; either
 * writes nothing.</p>
 *
 * <p>Unsigned values are read into the next wider type, such as a {@code uint32} into a
 * {@code long}, except for {@code uint64}, which Java has no wider type for: it's read into a
 * {@code long} with the same bits, for {@link Long#toUnsignedString} and the like. Unsigned
 * setters write the low bits of their argument, as a cast would.</p>
 */
public final class ByteBuffer {
    private final byte[] bytes;
    private final java.nio.ByteBuffer view;

    private final boolean sized;
    private final boolean versioned;
    /** The width in bytes of offsets, lengths and header slots */
    private final int offsetSize;
    /** The largest offset, length or version that the width holds */
    private final long maxOffset;
    /** The byte length of the header slots before the root's field offsets */
    private final int headerSize;

    private int position = 0;

    /**
     * Reads and writes {@code bytes} as a table of {@link TableLayout#DEFAULT}, without a header
     * and with 32-bit offsets.
     *
     * @param bytes the bytes to read or write, which the buffer shares rather than copies
     */
    public ByteBuffer(byte[] bytes) {
        this(bytes, TableLayout.DEFAULT);
    }

    /**
     * Reads and writes {@code bytes} as a table of {@code layout}.
     *
     * @param bytes the bytes to read or write, which the buffer shares rather than copies
     * @param layout the table's header and offset width, which its writer used too
     */
    public ByteBuffer(byte[] bytes, TableLayout layout) {
        this.bytes = bytes;
        this.view = java.nio.ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);

        this.sized = layout.sized();
        this.versioned = layout.versioned();
        this.offsetSize = layout.offsetSize().valueOf();
        this.maxOffset = layout.offsetSize().maxValue();
        this.headerSize = layout.headerSize();
    }

    /**
     * {@return a buffer of {@code size} zeros, to write into}
     *
     * @param size the number of bytes
     */
    public static ByteBuffer allocate(int size) {
        return new ByteBuffer(new byte[size]);
    }

    /** {@return the bytes that the buffer reads and writes, which it shares rather than copies} */
    public byte[] bytes() {
        return bytes;
    }

    /**
     * {@return the table's byte length: the header's when the layout is sized, and otherwise that
     * of the bytes}
     */
    public long getSize() {
        return sized ? getUnsigned(0) : bytes.length;
    }

    void setSize(int size) {
        setUnsigned(size);
    }

    /**
     * {@return the version that the table's header carries after its length} It's as wide as the
     * header's other slots, and read unsigned.
     *
     * @throws IllegalStateException if the buffer's layout isn't versioned
     */
    public long getVersion() {
        if (!versioned) {
            throw new IllegalStateException("The table has no version");
        }

        return getUnsigned(sized ? offsetSize : 0);
    }

    void setVersion(long version) {
        setUnsigned(version);
    }

    /**
     * {@return the position of field {@code index} of the root table}
     *
     * @param index the field's index, from 0
     * @throws DecodeException if the offset isn't within the bytes, or points past the largest
     *     {@code int}. An offset to elsewhere outside them throws from the read at its position.
     */
    public int rootOffset(int index) {
        return toPosition(getUnsigned(headerSize + (long) index * offsetSize));
    }

    /**
     * {@return the position of field {@code index} of a nested table} A nested table has no header,
     * and stores its field offsets from its own start.
     *
     * @param tablePosition the nested table's position, which is the offset of the field that
     *     holds it
     * @param index the field's index, from 0
     * @throws DecodeException if the offset isn't within the bytes, or points past the largest
     *     {@code int}. An offset to elsewhere outside them throws from the read at its position.
     */
    public int fieldOffset(int tablePosition, int index) {
        var offset = getUnsigned(tablePosition + (long) index * offsetSize);
        return toPosition(tablePosition + offset);
    }

    /**
     * Writes a field offset as wide as the layout's offsets, and moves past it.
     *
     * @param offset the field's position from the table's start
     */
    public void setOffset(int offset) {
        setUnsigned(offset);
    }

    // The scalar getters let the JDK's buffer check the range, which is free, and rethrow its error,
    // as the TypeScript getters rethrow DataView's

    /**
     * {@return the {@code int8} at {@code position}}
     *
     * @param position the value's position in the bytes
     * @throws DecodeException if the value isn't within the bytes
     */
    public byte getInt8(int position) {
        try {
            return view.get(position);
        } catch (IndexOutOfBoundsException ex) {
            throw outOfRange(position, Byte.BYTES, ex);
        }
    }

    /**
     * {@return the {@code uint8} at {@code position}, widened to a {@code short}}
     *
     * @param position the value's position in the bytes
     * @throws DecodeException if the value isn't within the bytes
     */
    public short getUint8(int position) {
        return (short) Byte.toUnsignedInt(getInt8(position));
    }

    /**
     * {@return the {@code int16} at {@code position}}
     *
     * @param position the value's position in the bytes
     * @throws DecodeException if the value isn't within the bytes
     */
    public short getInt16(int position) {
        try {
            return view.getShort(position);
        } catch (IndexOutOfBoundsException ex) {
            throw outOfRange(position, Short.BYTES, ex);
        }
    }

    /**
     * {@return the {@code uint16} at {@code position}, widened to an {@code int}}
     *
     * @param position the value's position in the bytes
     * @throws DecodeException if the value isn't within the bytes
     */
    public int getUint16(int position) {
        return Short.toUnsignedInt(getInt16(position));
    }

    /**
     * {@return the {@code int32} at {@code position}}
     *
     * @param position the value's position in the bytes
     * @throws DecodeException if the value isn't within the bytes
     */
    public int getInt32(int position) {
        try {
            return view.getInt(position);
        } catch (IndexOutOfBoundsException ex) {
            throw outOfRange(position, Integer.BYTES, ex);
        }
    }

    /**
     * {@return the {@code uint32} at {@code position}, widened to a {@code long}}
     *
     * @param position the value's position in the bytes
     * @throws DecodeException if the value isn't within the bytes
     */
    public long getUint32(int position) {
        return Integer.toUnsignedLong(getInt32(position));
    }

    /**
     * {@return the {@code int64} at {@code position}}
     *
     * @param position the value's position in the bytes
     * @throws DecodeException if the value isn't within the bytes
     */
    public long getInt64(int position) {
        try {
            return view.getLong(position);
        } catch (IndexOutOfBoundsException ex) {
            throw outOfRange(position, Long.BYTES, ex);
        }
    }

    /**
     * {@return the {@code uint64} at {@code position}, as a {@code long} with the same bits}
     *
     * @param position the value's position in the bytes
     * @throws DecodeException if the value isn't within the bytes
     */
    public long getUint64(int position) {
        return getInt64(position);
    }

    /**
     * {@return the {@code float32} at {@code position}}
     *
     * @param position the value's position in the bytes
     * @throws DecodeException if the value isn't within the bytes
     */
    public float getFloat32(int position) {
        try {
            return view.getFloat(position);
        } catch (IndexOutOfBoundsException ex) {
            throw outOfRange(position, Float.BYTES, ex);
        }
    }

    /**
     * {@return the {@code float64} at {@code position}}
     *
     * @param position the value's position in the bytes
     * @throws DecodeException if the value isn't within the bytes
     */
    public double getFloat64(int position) {
        try {
            return view.getDouble(position);
        } catch (IndexOutOfBoundsException ex) {
            throw outOfRange(position, Double.BYTES, ex);
        }
    }

    // The array getters check the length prefix against the bytes before allocating for it. Arrays
    // of the elements' own Java type are copied in bulk, and unsigned ones are widened one by one.

    /**
     * {@return a copy of the {@code int8} array at {@code position}}
     *
     * @param position the array's position in the bytes, where its length is
     * @throws DecodeException if the array isn't within the bytes
     */
    public byte[] getInt8Array(int position) {
        var values = new byte[arrayLength(position, Byte.BYTES)];
        System.arraycopy(bytes, position + offsetSize, values, 0, values.length);

        return values;
    }

    /**
     * {@return a copy of the {@code uint8} array at {@code position}, widened to {@code short}s}
     *
     * @param position the array's position in the bytes, where its length is
     * @throws DecodeException if the array isn't within the bytes
     */
    public short[] getUint8Array(int position) {
        var values = new short[arrayLength(position, Byte.BYTES)];
        for (int i = 0, j = position + offsetSize; i < values.length; i++, j++) {
            values[i] = (short) Byte.toUnsignedInt(bytes[j]);
        }

        return values;
    }

    /**
     * {@return a copy of the {@code int16} array at {@code position}}
     *
     * @param position the array's position in the bytes, where its length is
     * @throws DecodeException if the array isn't within the bytes
     */
    public short[] getInt16Array(int position) {
        var values = new short[arrayLength(position, Short.BYTES)];
        elementsAt(position, values.length * Short.BYTES).asShortBuffer().get(values);

        return values;
    }

    /**
     * {@return a copy of the {@code uint16} array at {@code position}, widened to {@code int}s}
     *
     * @param position the array's position in the bytes, where its length is
     * @throws DecodeException if the array isn't within the bytes
     */
    public int[] getUint16Array(int position) {
        var values = new int[arrayLength(position, Short.BYTES)];
        for (int i = 0, j = position + offsetSize; i < values.length; i++, j += Short.BYTES) {
            values[i] = Short.toUnsignedInt(view.getShort(j));
        }

        return values;
    }

    /**
     * {@return a copy of the {@code int32} array at {@code position}}
     *
     * @param position the array's position in the bytes, where its length is
     * @throws DecodeException if the array isn't within the bytes
     */
    public int[] getInt32Array(int position) {
        var values = new int[arrayLength(position, Integer.BYTES)];
        elementsAt(position, values.length * Integer.BYTES).asIntBuffer().get(values);

        return values;
    }

    /**
     * {@return a copy of the {@code uint32} array at {@code position}, widened to {@code long}s}
     *
     * @param position the array's position in the bytes, where its length is
     * @throws DecodeException if the array isn't within the bytes
     */
    public long[] getUint32Array(int position) {
        var values = new long[arrayLength(position, Integer.BYTES)];
        for (int i = 0, j = position + offsetSize; i < values.length; i++, j += Integer.BYTES) {
            values[i] = Integer.toUnsignedLong(view.getInt(j));
        }

        return values;
    }

    /**
     * {@return a copy of the {@code int64} array at {@code position}}
     *
     * @param position the array's position in the bytes, where its length is
     * @throws DecodeException if the array isn't within the bytes
     */
    public long[] getInt64Array(int position) {
        var values = new long[arrayLength(position, Long.BYTES)];
        elementsAt(position, values.length * Long.BYTES).asLongBuffer().get(values);

        return values;
    }

    /**
     * {@return a copy of the {@code uint64} array at {@code position}, as {@code long}s with the
     * same bits}
     *
     * @param position the array's position in the bytes, where its length is
     * @throws DecodeException if the array isn't within the bytes
     */
    public long[] getUint64Array(int position) {
        return getInt64Array(position);
    }

    /**
     * {@return a copy of the {@code float32} array at {@code position}}
     *
     * @param position the array's position in the bytes, where its length is
     * @throws DecodeException if the array isn't within the bytes
     */
    public float[] getFloat32Array(int position) {
        var values = new float[arrayLength(position, Float.BYTES)];
        elementsAt(position, values.length * Float.BYTES).asFloatBuffer().get(values);

        return values;
    }

    /**
     * {@return a copy of the {@code float64} array at {@code position}}
     *
     * @param position the array's position in the bytes, where its length is
     * @throws DecodeException if the array isn't within the bytes
     */
    public double[] getFloat64Array(int position) {
        var values = new double[arrayLength(position, Double.BYTES)];
        elementsAt(position, values.length * Double.BYTES).asDoubleBuffer().get(values);

        return values;
    }

    /**
     * Reads the element count at {@code position}, after checking that the elements after it lie
     * within the bytes, so that nothing is allocated for a count the bytes can't hold.
     */
    private int arrayLength(int position, int elementSize) {
        var length = getUnsigned(position);
        var start = (long) position + offsetSize;
        var byteLength = length * elementSize;

        if (start + byteLength > bytes.length) {
            throw DecodeException.outOfRange(start, byteLength, bytes.length);
        }

        return (int) length;
    }

    /**
     * {@return a little-endian view of the {@code byteLength} bytes after the length at
     * {@code position}}
     */
    private java.nio.ByteBuffer elementsAt(int position, int byteLength) {
        return view.slice(position + offsetSize, byteLength).order(ByteOrder.LITTLE_ENDIAN);
    }

    /**
     * Writes an {@code int8} at the buffer's position, and moves past it.
     *
     * @param value the value to write
     */
    public void setInt8(byte value) {
        view.put(position, value);
        position += Byte.BYTES;
    }

    /**
     * Writes a {@code uint8}, the low 8 bits of {@code value}, at the buffer's position, and moves
     * past it.
     *
     * @param value the value to write
     */
    public void setUint8(short value) {
        setInt8((byte) value);
    }

    /**
     * Writes an {@code int16} at the buffer's position, and moves past it.
     *
     * @param value the value to write
     */
    public void setInt16(short value) {
        view.putShort(position, value);
        position += Short.BYTES;
    }

    /**
     * Writes a {@code uint16}, the low 16 bits of {@code value}, at the buffer's position, and
     * moves past it.
     *
     * @param value the value to write
     */
    public void setUint16(int value) {
        setInt16((short) value);
    }

    /**
     * Writes an {@code int32} at the buffer's position, and moves past it.
     *
     * @param value the value to write
     */
    public void setInt32(int value) {
        view.putInt(position, value);
        position += Integer.BYTES;
    }

    /**
     * Writes a {@code uint32}, the low 32 bits of {@code value}, at the buffer's position, and
     * moves past it.
     *
     * @param value the value to write
     */
    public void setUint32(long value) {
        setInt32((int) value);
    }

    /**
     * Writes an {@code int64} at the buffer's position, and moves past it.
     *
     * @param value the value to write
     */
    public void setInt64(long value) {
        view.putLong(position, value);
        position += Long.BYTES;
    }

    /**
     * Writes a {@code uint64} from a {@code long} with its bits.
     *
     * @param value the value to write
     */
    public void setUint64(long value) {
        setInt64(value);
    }

    /**
     * Writes a {@code float32} at the buffer's position, and moves past it.
     *
     * @param value the value to write
     */
    public void setFloat32(float value) {
        view.putFloat(position, value);
        position += Float.BYTES;
    }

    /**
     * Writes a {@code float64} at the buffer's position, and moves past it.
     *
     * @param value the value to write
     */
    public void setFloat64(double value) {
        view.putDouble(position, value);
        position += Double.BYTES;
    }

    // The array setters write the length and then the elements, after checking that both fit

    /**
     * Writes the length of {@code values} and then each as an {@code int8}.
     *
     * @param values the elements to write
     */
    public void setInt8Array(byte[] values) {
        startArray(values.length, Byte.BYTES);
        System.arraycopy(values, 0, bytes, position, values.length);
        position += values.length;
    }

    /**
     * Writes the length of {@code values} and then the low 8 bits of each as a {@code uint8}.
     *
     * @param values the elements to write
     */
    public void setUint8Array(short[] values) {
        startArray(values.length, Byte.BYTES);
        for (var value : values) {
            bytes[position++] = (byte) value;
        }
    }

    /**
     * Writes the length of {@code values} and then each as an {@code int16}.
     *
     * @param values the elements to write
     */
    public void setInt16Array(short[] values) {
        startArray(values.length, Short.BYTES);
        nextElements(values.length * Short.BYTES).asShortBuffer().put(values);
        position += values.length * Short.BYTES;
    }

    /**
     * Writes the length of {@code values} and then the low 16 bits of each as a {@code uint16}.
     *
     * @param values the elements to write
     */
    public void setUint16Array(int[] values) {
        startArray(values.length, Short.BYTES);
        for (var value : values) {
            view.putShort(position, (short) value);
            position += Short.BYTES;
        }
    }

    /**
     * Writes the length of {@code values} and then each as an {@code int32}.
     *
     * @param values the elements to write
     */
    public void setInt32Array(int[] values) {
        startArray(values.length, Integer.BYTES);
        nextElements(values.length * Integer.BYTES).asIntBuffer().put(values);
        position += values.length * Integer.BYTES;
    }

    /**
     * Writes the length of {@code values} and then the low 32 bits of each as a {@code uint32}.
     *
     * @param values the elements to write
     */
    public void setUint32Array(long[] values) {
        startArray(values.length, Integer.BYTES);
        for (var value : values) {
            view.putInt(position, (int) value);
            position += Integer.BYTES;
        }
    }

    /**
     * Writes the length of {@code values} and then each as an {@code int64}.
     *
     * @param values the elements to write
     */
    public void setInt64Array(long[] values) {
        startArray(values.length, Long.BYTES);
        nextElements(values.length * Long.BYTES).asLongBuffer().put(values);
        position += values.length * Long.BYTES;
    }

    /**
     * Writes the length of {@code values} and then each as a {@code uint64}, from its bits.
     *
     * @param values the elements to write
     */
    public void setUint64Array(long[] values) {
        setInt64Array(values);
    }

    /**
     * Writes the length of {@code values} and then each as a {@code float32}.
     *
     * @param values the elements to write
     */
    public void setFloat32Array(float[] values) {
        startArray(values.length, Float.BYTES);
        nextElements(values.length * Float.BYTES).asFloatBuffer().put(values);
        position += values.length * Float.BYTES;
    }

    /**
     * Writes the length of {@code values} and then each as a {@code float64}.
     *
     * @param values the elements to write
     */
    public void setFloat64Array(double[] values) {
        startArray(values.length, Double.BYTES);
        nextElements(values.length * Double.BYTES).asDoubleBuffer().put(values);
        position += values.length * Double.BYTES;
    }

    /**
     * Checks that {@code length} elements of {@code elementSize} bytes fit after their length
     * prefix, and writes the prefix.
     */
    private void startArray(int length, int elementSize) {
        assertWritable(offsetSize + (long) length * elementSize);
        setUnsigned(length);
    }

    /** {@return a little-endian view of the next {@code byteLength} bytes, which startArray() checked} */
    private java.nio.ByteBuffer nextElements(int byteLength) {
        return view.slice(position, byteLength).order(ByteOrder.LITTLE_ENDIAN);
    }

    /**
     * Writes {@code bytes} as they are, without a length prefix.
     *
     * @param bytes the bytes to write
     */
    public void setBytes(byte[] bytes) {
        assertWritable(bytes.length);

        System.arraycopy(bytes, 0, this.bytes, position, bytes.length);
        position += bytes.length;
    }

    /**
     * Writes {@code byteLength} zeros, such as to align the field after them.
     *
     * @param byteLength the number of zeros
     */
    public void setPadding(int byteLength) {
        assertWritable(byteLength);

        Arrays.fill(bytes, position, position + byteLength, (byte) 0);
        position += byteLength;
    }

    /**
     * {@return the UTF-8 string at {@code position}} A leading U+FEFF is kept, since it's part of
     * the string rather than a byte order mark.
     *
     * @param position the string's position in the bytes, where its length is
     * @throws DecodeException if the string isn't within the bytes or isn't valid UTF-8
     */
    public String getString(int position) {
        var length = arrayLength(position, Byte.BYTES);

        try {
            return Utf8.decode(bytes, position + offsetSize, length);
        } catch (CharacterCodingException ex) {
            throw new DecodeException(String.format("The string at %d isn't valid UTF-8", position), ex);
        }
    }

    /**
     * Writes a string's UTF-8 bytes after their length.
     *
     * @param utf8 the string's UTF-8 bytes
     */
    public void setString(byte[] utf8) {
        setInt8Array(utf8);
    }

    /**
     * {@return the position of each item of the vector at {@code position}, after its length}
     *
     * @param position the vector's position in the bytes, where its item count is
     * @throws DecodeException if the vector isn't within the bytes
     */
    public int[] getVectorOffsets(int position) {
        // Every item needs at least its length prefix, which bounds the count
        var count = arrayLength(position, offsetSize);
        var offsets = new int[count];

        for (int i = 0, offset = position + offsetSize; i < count; i++) {
            var length = arrayLength(offset, Byte.BYTES);
            offsets[i] = offset + offsetSize;
            offset += offsetSize + length;
        }

        return offsets;
    }

    /**
     * {@return a copy of each item of the vector at {@code position}}
     *
     * @param position the vector's position in the bytes, where its item count is
     * @throws DecodeException if the vector isn't within the bytes
     */
    public byte[][] getVector(int position) {
        var offsets = getVectorOffsets(position);

        var vector = new byte[offsets.length][];
        for (int i = 0; i < offsets.length; i++) {
            vector[i] = getInt8Array(offsets[i] - offsetSize);
        }

        return vector;
    }

    /**
     * Writes the item count, and then each item's length and bytes.
     *
     * @param vector the bytes of each item
     */
    public void setVector(byte[][] vector) {
        long byteLength = offsetSize + (long) vector.length * offsetSize;
        assertFits(vector.length);
        for (var item : vector) {
            assertFits(item.length);
            byteLength += item.length;
        }
        assertWritable(byteLength);

        setUnsigned(vector.length);
        for (var item : vector) {
            setUnsigned(item.length);
            setBytes(item);
        }
    }

    /** Writes a length or count as wide as the table's offsets */
    void setLength(int length) {
        setUnsigned(length);
    }

    /**
     * Reads an unsigned value as wide as the table's offsets, such as a length or an offset.
     *
     * @throws DecodeException if it isn't within the bytes
     */
    private long getUnsigned(long position) {
        var index = (int) position;
        if (index != position) {
            throw DecodeException.outOfRange(position, offsetSize, bytes.length);
        }

        // The JDK's buffer checks the rest of the range, as it does for the scalar getters. A check
        // of its own as well made reading a vector of 100 nested tables 10% slower.
        try {
            return switch (offsetSize) {
                case Byte.BYTES -> Byte.toUnsignedLong(view.get(index));

                case Short.BYTES -> Short.toUnsignedLong(view.getShort(index));

                default -> Integer.toUnsignedLong(view.getInt(index));
            };
        } catch (IndexOutOfBoundsException ex) {
            throw outOfRange(index, offsetSize, ex);
        }
    }

    /**
     * Writes an unsigned value as wide as the table's offsets, such as a length or an offset.
     *
     * @throws IllegalArgumentException if it doesn't fit in that width
     */
    private void setUnsigned(long value) {
        assertFits(value);

        switch (offsetSize) {
            case Byte.BYTES -> view.put(position, (byte) value);

            case Short.BYTES -> view.putShort(position, (short) value);

            default -> view.putInt(position, (int) value);
        }

        position += offsetSize;
    }

    private void assertFits(long value) {
        if (value < 0 || value > maxOffset) {
            throw new IllegalArgumentException(
                String.format("%d doesn't fit in %d-byte offsets", value, offsetSize)
            );
        }
    }

    private void assertWritable(long byteLength) {
        if (position + byteLength > bytes.length) {
            throw new IndexOutOfBoundsException(
                String.format(
                    "Write exceeds the supplied buffer: %d bytes at %d of %d",
                    byteLength,
                    position,
                    bytes.length
                )
            );
        }
    }

    /**
     * Converts an offset to a position, which the read at it checks against the bytes, as in
     * TypeScript. Checking it against them here as well made resolving the field offsets of 100
     * nested tables 15% slower.
     */
    private int toPosition(long offset) {
        if (offset > Integer.MAX_VALUE) {
            throw new DecodeException(
                String.format(
                    "Offset %d points outside the supplied buffer of %d bytes",
                    offset,
                    bytes.length
                )
            );
        }

        return (int) offset;
    }

    private DecodeException outOfRange(int position, int byteLength, IndexOutOfBoundsException cause) {
        var exception = DecodeException.outOfRange(position, byteLength, bytes.length);
        exception.initCause(cause);

        return exception;
    }
}
