package io.github.snovader.buffy.test;

import io.github.snovader.buffy.*;

import java.util.*;

/**
 * The type of a scalar field and of an array's elements, which descriptions name as
 * {@code cases.json} does, such as {@code "uint16"} and {@code "uint16[]"}.
 *
 * <p>Descriptions hold every value as a {@code long}: an integer as its value, a {@code uint64} as
 * its bits, as {@link ByteBuffer} reads it, and a float as its raw bits, a float32's as an
 * {@code int} widened to a {@code long}. Comparing values then compares floats by their bits,
 * telling -0.0 from 0.0, and NaN from any other value, NaNs included.</p>
 */
enum ScalarType {
    INT8(Byte.BYTES),
    UINT8(Byte.BYTES),
    INT16(Short.BYTES),
    UINT16(Short.BYTES),
    INT32(Integer.BYTES),
    UINT32(Integer.BYTES),
    INT64(Long.BYTES),
    UINT64(Long.BYTES),
    FLOAT32(Float.BYTES),
    FLOAT64(Double.BYTES);

    private final int size;

    ScalarType(int size) {
        this.size = size;
    }

    /** {@return the type's name in {@code cases.json}, such as {@code "uint16"}} */
    String jsonName() {
        return name().toLowerCase(Locale.ROOT);
    }

    /** {@return the byte length of a value} */
    int size() {
        return size;
    }

    void set(TableBuilder builder, long value) {
        switch (this) {
            case INT8 -> builder.setInt8((byte) value);
            case UINT8 -> builder.setUint8((short) value);
            case INT16 -> builder.setInt16((short) value);
            case UINT16 -> builder.setUint16((int) value);
            case INT32 -> builder.setInt32((int) value);
            case UINT32 -> builder.setUint32(value);
            case INT64 -> builder.setInt64(value);
            case UINT64 -> builder.setUint64(value);
            case FLOAT32 -> builder.setFloat32(Float.intBitsToFloat((int) value));
            case FLOAT64 -> builder.setFloat64(Double.longBitsToDouble(value));
        }
    }

    void setArray(TableBuilder builder, long[] values) {
        switch (this) {
            case INT8 -> builder.setInt8Array(toBytes(values));
            case UINT8 -> builder.setUint8Array(toShorts(values));
            case INT16 -> builder.setInt16Array(toShorts(values));
            case UINT16 -> builder.setUint16Array(toInts(values));
            case INT32 -> builder.setInt32Array(toInts(values));
            case UINT32 -> builder.setUint32Array(values);
            case INT64 -> builder.setInt64Array(values);
            case UINT64 -> builder.setUint64Array(values);
            case FLOAT32 -> builder.setFloat32Array(toFloats(values));
            case FLOAT64 -> builder.setFloat64Array(toDoubles(values));
        }
    }

    /** Writes a value at the buffer's position with its own setter, rather than a builder's */
    void write(ByteBuffer buffer, long value) {
        switch (this) {
            case INT8 -> buffer.setInt8((byte) value);
            case UINT8 -> buffer.setUint8((short) value);
            case INT16 -> buffer.setInt16((short) value);
            case UINT16 -> buffer.setUint16((int) value);
            case INT32 -> buffer.setInt32((int) value);
            case UINT32 -> buffer.setUint32(value);
            case INT64 -> buffer.setInt64(value);
            case UINT64 -> buffer.setUint64(value);
            case FLOAT32 -> buffer.setFloat32(Float.intBitsToFloat((int) value));
            case FLOAT64 -> buffer.setFloat64(Double.longBitsToDouble(value));
        }
    }

    /** Writes an array at the buffer's position with its own setter, rather than a builder's */
    void writeArray(ByteBuffer buffer, long[] values) {
        switch (this) {
            case INT8 -> buffer.setInt8Array(toBytes(values));
            case UINT8 -> buffer.setUint8Array(toShorts(values));
            case INT16 -> buffer.setInt16Array(toShorts(values));
            case UINT16 -> buffer.setUint16Array(toInts(values));
            case INT32 -> buffer.setInt32Array(toInts(values));
            case UINT32 -> buffer.setUint32Array(values);
            case INT64 -> buffer.setInt64Array(values);
            case UINT64 -> buffer.setUint64Array(values);
            case FLOAT32 -> buffer.setFloat32Array(toFloats(values));
            case FLOAT64 -> buffer.setFloat64Array(toDoubles(values));
        }
    }

    long read(ByteBuffer buffer, int position) {
        return switch (this) {
            case INT8 -> buffer.getInt8(position);
            case UINT8 -> buffer.getUint8(position);
            case INT16 -> buffer.getInt16(position);
            case UINT16 -> buffer.getUint16(position);
            case INT32 -> buffer.getInt32(position);
            case UINT32 -> buffer.getUint32(position);
            case INT64 -> buffer.getInt64(position);
            case UINT64 -> buffer.getUint64(position);
            case FLOAT32 -> Float.floatToRawIntBits(buffer.getFloat32(position));
            case FLOAT64 -> Double.doubleToRawLongBits(buffer.getFloat64(position));
        };
    }

    long[] readArray(ByteBuffer buffer, int position) {
        return switch (this) {
            case INT8 -> fromBytes(buffer.getInt8Array(position));
            case UINT8 -> fromShorts(buffer.getUint8Array(position));
            case INT16 -> fromShorts(buffer.getInt16Array(position));
            case UINT16 -> fromInts(buffer.getUint16Array(position));
            case INT32 -> fromInts(buffer.getInt32Array(position));
            case UINT32 -> buffer.getUint32Array(position);
            case INT64 -> buffer.getInt64Array(position);
            case UINT64 -> buffer.getUint64Array(position);
            case FLOAT32 -> fromFloats(buffer.getFloat32Array(position));
            case FLOAT64 -> fromDoubles(buffer.getFloat64Array(position));
        };
    }

    /** Puts a value with the JDK's own buffer, in its byte order, to compare buffy's bytes with */
    void put(java.nio.ByteBuffer target, long value) {
        switch (this) {
            case INT8, UINT8 -> target.put((byte) value);
            case INT16, UINT16 -> target.putShort((short) value);
            case INT32, UINT32 -> target.putInt((int) value);
            case INT64, UINT64 -> target.putLong(value);
            case FLOAT32 -> target.putFloat(Float.intBitsToFloat((int) value));
            case FLOAT64 -> target.putDouble(Double.longBitsToDouble(value));
        }
    }

    // Arrays of values as descriptions hold them, converted to and from the Java arrays that
    // buffy's setters and getters take, with floats as their raw bits

    static byte[] toBytes(long[] values) {
        var array = new byte[values.length];
        for (int i = 0; i < values.length; i++) {
            array[i] = (byte) values[i];
        }

        return array;
    }

    static short[] toShorts(long[] values) {
        var array = new short[values.length];
        for (int i = 0; i < values.length; i++) {
            array[i] = (short) values[i];
        }

        return array;
    }

    static int[] toInts(long[] values) {
        var array = new int[values.length];
        for (int i = 0; i < values.length; i++) {
            array[i] = (int) values[i];
        }

        return array;
    }

    static float[] toFloats(long[] values) {
        var array = new float[values.length];
        for (int i = 0; i < values.length; i++) {
            array[i] = Float.intBitsToFloat((int) values[i]);
        }

        return array;
    }

    static double[] toDoubles(long[] values) {
        var array = new double[values.length];
        for (int i = 0; i < values.length; i++) {
            array[i] = Double.longBitsToDouble(values[i]);
        }

        return array;
    }

    static long[] fromBytes(byte[] array) {
        var values = new long[array.length];
        for (int i = 0; i < array.length; i++) {
            values[i] = array[i];
        }

        return values;
    }

    static long[] fromShorts(short[] array) {
        var values = new long[array.length];
        for (int i = 0; i < array.length; i++) {
            values[i] = array[i];
        }

        return values;
    }

    static long[] fromInts(int[] array) {
        var values = new long[array.length];
        for (int i = 0; i < array.length; i++) {
            values[i] = array[i];
        }

        return values;
    }

    static long[] fromFloats(float[] array) {
        var values = new long[array.length];
        for (int i = 0; i < array.length; i++) {
            values[i] = Float.floatToRawIntBits(array[i]);
        }

        return values;
    }

    static long[] fromDoubles(double[] array) {
        var values = new long[array.length];
        for (int i = 0; i < array.length; i++) {
            values[i] = Double.doubleToRawLongBits(array[i]);
        }

        return values;
    }
}
