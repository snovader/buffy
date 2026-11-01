package io.github.snovader.buffy.test;

import io.github.snovader.buffy.*;
import net.jqwik.api.*;

import java.util.*;

/** A field of each type that a table holds, which sets itself on a builder and reads itself back */
record Field(Field.Type type, Object value) {
    enum Type {
        INT8,
        UINT8,
        INT16,
        UINT16,
        INT32,
        UINT32,
        INT64,
        UINT64,
        FLOAT32,
        FLOAT64,
        INT8_ARRAY,
        UINT8_ARRAY,
        INT16_ARRAY,
        UINT16_ARRAY,
        INT32_ARRAY,
        UINT32_ARRAY,
        INT64_ARRAY,
        UINT64_ARRAY,
        FLOAT32_ARRAY,
        FLOAT64_ARRAY,
        STRING,
        VECTOR;

        /** {@return the size of the type's array elements, or 1 for any other type} */
        int alignment() {
            return switch (this) {
                case INT16_ARRAY, UINT16_ARRAY -> Short.BYTES;
                case INT32_ARRAY, UINT32_ARRAY, FLOAT32_ARRAY -> Integer.BYTES;
                case INT64_ARRAY, UINT64_ARRAY, FLOAT64_ARRAY -> Long.BYTES;
                default -> Byte.BYTES;
            };
        }
    }

    private static final int MAX_LENGTH = 16;

    /** {@return any field, with arrays, strings and vectors of up to 16 elements} */
    static Arbitrary<Field> any() {
        return Arbitraries.oneOf(List.of(
            Arbitraries.bytes().map(v -> new Field(Type.INT8, v)),
            Arbitraries.shorts().between((short) 0, (short) 255).map(v -> new Field(Type.UINT8, v)),
            Arbitraries.shorts().map(v -> new Field(Type.INT16, v)),
            Arbitraries.integers().between(0, 65535).map(v -> new Field(Type.UINT16, v)),
            Arbitraries.integers().map(v -> new Field(Type.INT32, v)),
            Arbitraries.longs().between(0, 4294967295L).map(v -> new Field(Type.UINT32, v)),
            Arbitraries.longs().map(v -> new Field(Type.INT64, v)),
            Arbitraries.longs().map(v -> new Field(Type.UINT64, v)),
            Arbitraries.floats().map(v -> new Field(Type.FLOAT32, v)),
            Arbitraries.doubles().map(v -> new Field(Type.FLOAT64, v)),
            arrays(),
            Arbitraries.strings().ofMaxLength(MAX_LENGTH).map(v -> new Field(Type.STRING, v)),
            Arbitraries.bytes()
                .array(byte[].class)
                .ofMaxSize(MAX_LENGTH)
                .array(byte[][].class)
                .ofMaxSize(4)
                .map(v -> new Field(Type.VECTOR, v))
        ));
    }

    /** {@return an array field of any element type} */
    static Arbitrary<Field> arrays() {
        return Arbitraries.oneOf(List.of(
            Arbitraries.bytes().array(byte[].class).ofMaxSize(MAX_LENGTH).map(v -> new Field(Type.INT8_ARRAY, v)),
            Arbitraries.shorts()
                .between((short) 0, (short) 255)
                .array(short[].class)
                .ofMaxSize(MAX_LENGTH)
                .map(v -> new Field(Type.UINT8_ARRAY, v)),
            Arbitraries.shorts().array(short[].class).ofMaxSize(MAX_LENGTH).map(v -> new Field(Type.INT16_ARRAY, v)),
            Arbitraries.integers()
                .between(0, 65535)
                .array(int[].class)
                .ofMaxSize(MAX_LENGTH)
                .map(v -> new Field(Type.UINT16_ARRAY, v)),
            Arbitraries.integers().array(int[].class).ofMaxSize(MAX_LENGTH).map(v -> new Field(Type.INT32_ARRAY, v)),
            Arbitraries.longs()
                .between(0, 4294967295L)
                .array(long[].class)
                .ofMaxSize(MAX_LENGTH)
                .map(v -> new Field(Type.UINT32_ARRAY, v)),
            Arbitraries.longs().array(long[].class).ofMaxSize(MAX_LENGTH).map(v -> new Field(Type.INT64_ARRAY, v)),
            Arbitraries.longs().array(long[].class).ofMaxSize(MAX_LENGTH).map(v -> new Field(Type.UINT64_ARRAY, v)),
            Arbitraries.floats().array(float[].class).ofMaxSize(MAX_LENGTH).map(v -> new Field(Type.FLOAT32_ARRAY, v)),
            Arbitraries.doubles().array(double[].class).ofMaxSize(MAX_LENGTH).map(v -> new Field(Type.FLOAT64_ARRAY, v))
        ));
    }

    /** {@return a builder of {@code layout} with {@code fields} set and {@code more} left to set} */
    static TableBuilder builderOf(List<Field> fields, int more, TableLayout layout) {
        var builder = new TableBuilder(fields.size() + more, layout);
        for (var field : fields) {
            field.setOn(builder);
        }

        return builder;
    }

    static TableBuilder builderOf(List<Field> fields) {
        return builderOf(fields, 0, TableLayout.DEFAULT);
    }

    /** {@return the largest element size of the fields' arrays, or 1 without any} */
    static int alignmentOf(List<Field> fields) {
        return fields.stream().mapToInt(field -> field.type().alignment()).max().orElse(Byte.BYTES);
    }

    void setOn(TableBuilder builder) {
        switch (type) {
            case INT8 -> builder.setInt8((byte) value);
            case UINT8 -> builder.setUint8((short) value);
            case INT16 -> builder.setInt16((short) value);
            case UINT16 -> builder.setUint16((int) value);
            case INT32 -> builder.setInt32((int) value);
            case UINT32 -> builder.setUint32((long) value);
            case INT64 -> builder.setInt64((long) value);
            case UINT64 -> builder.setUint64((long) value);
            case FLOAT32 -> builder.setFloat32((float) value);
            case FLOAT64 -> builder.setFloat64((double) value);
            case INT8_ARRAY -> builder.setInt8Array((byte[]) value);
            case UINT8_ARRAY -> builder.setUint8Array((short[]) value);
            case INT16_ARRAY -> builder.setInt16Array((short[]) value);
            case UINT16_ARRAY -> builder.setUint16Array((int[]) value);
            case INT32_ARRAY -> builder.setInt32Array((int[]) value);
            case UINT32_ARRAY -> builder.setUint32Array((long[]) value);
            case INT64_ARRAY -> builder.setInt64Array((long[]) value);
            case UINT64_ARRAY -> builder.setUint64Array((long[]) value);
            case FLOAT32_ARRAY -> builder.setFloat32Array((float[]) value);
            case FLOAT64_ARRAY -> builder.setFloat64Array((double[]) value);
            case STRING -> builder.setString((String) value);
            case VECTOR -> builder.setVector((byte[][]) value);
        }
    }

    /** {@return the value of this field's type at {@code position}} */
    Object readFrom(ByteBuffer buffer, int position) {
        return switch (type) {
            case INT8 -> buffer.getInt8(position);
            case UINT8 -> buffer.getUint8(position);
            case INT16 -> buffer.getInt16(position);
            case UINT16 -> buffer.getUint16(position);
            case INT32 -> buffer.getInt32(position);
            case UINT32 -> buffer.getUint32(position);
            case INT64 -> buffer.getInt64(position);
            case UINT64 -> buffer.getUint64(position);
            case FLOAT32 -> buffer.getFloat32(position);
            case FLOAT64 -> buffer.getFloat64(position);
            case INT8_ARRAY -> buffer.getInt8Array(position);
            case UINT8_ARRAY -> buffer.getUint8Array(position);
            case INT16_ARRAY -> buffer.getInt16Array(position);
            case UINT16_ARRAY -> buffer.getUint16Array(position);
            case INT32_ARRAY -> buffer.getInt32Array(position);
            case UINT32_ARRAY -> buffer.getUint32Array(position);
            case INT64_ARRAY -> buffer.getInt64Array(position);
            case UINT64_ARRAY -> buffer.getUint64Array(position);
            case FLOAT32_ARRAY -> buffer.getFloat32Array(position);
            case FLOAT64_ARRAY -> buffer.getFloat64Array(position);
            case STRING -> buffer.getString(position);
            case VECTOR -> buffer.getVector(position);
        };
    }

    /**
     * {@return whether {@code read} is this field's value, element by element, with floats compared
     * as {@link Float#equals} compares them, which tells -0.0 from 0.0}
     */
    boolean matches(Object read) {
        return Objects.deepEquals(value, read);
    }
}
