package io.github.snovader.buffy.test;

import io.github.snovader.buffy.*;
import net.jqwik.api.*;
import net.jqwik.api.constraints.*;

import java.nio.ByteOrder;
import java.nio.charset.*;
import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

public class ByteBufferTest {
    @Property
    void should_round_trip_int8(@ForAll byte value) {
        var buffer = ByteBuffer.allocate(Byte.BYTES);
        buffer.setInt8(value);
        assertEquals(value, buffer.getInt8(0));
    }

    @Property
    void should_round_trip_uint8(@ForAll @ShortRange(max = 255 /* 2 ** 8 - 1 */) short value) {
        var buffer = ByteBuffer.allocate(Byte.BYTES);
        buffer.setUint8(value);
        assertEquals(value, buffer.getUint8(0));
    }

    @Property
    void should_round_trip_int16(@ForAll short value) {
        var buffer = ByteBuffer.allocate(Short.BYTES);
        buffer.setInt16(value);
        assertEquals(value, buffer.getInt16(0));
    }

    @Property
    void should_round_trip_uint16(@ForAll @IntRange(max = 65535 /* 2 ** 16 - 1 */) int value) {
        var buffer = ByteBuffer.allocate(Short.BYTES);
        buffer.setUint16(value);
        assertEquals(value, buffer.getUint16(0));
    }

    @Property
    void should_round_trip_int32(@ForAll int value) {
        var buffer = ByteBuffer.allocate(Integer.BYTES);
        buffer.setInt32(value);
        assertEquals(value, buffer.getInt32(0));
    }

    @Property
    void should_round_trip_uint32(@ForAll @LongRange(max = 4294967295L /* 2 ** 32 - 1 */) long value) {
        var buffer = ByteBuffer.allocate(Integer.BYTES);
        buffer.setUint32(value);
        assertEquals(value, buffer.getUint32(0));
    }

    @Property
    void should_round_trip_int64(@ForAll long value) {
        var buffer = ByteBuffer.allocate(Long.BYTES);
        buffer.setInt64(value);
        assertEquals(value, buffer.getInt64(0));
    }

    @Property
    void should_round_trip_uint64_as_its_bits(@ForAll long value) {
        var buffer = ByteBuffer.allocate(Long.BYTES);
        buffer.setUint64(value);
        assertEquals(value, buffer.getUint64(0));
    }

    @Property
    void should_round_trip_float32(@ForAll float value) {
        var buffer = ByteBuffer.allocate(Float.BYTES);
        buffer.setFloat32(value);
        assertEquals(value, buffer.getFloat32(0));
    }

    @Property
    void should_round_trip_float64(@ForAll double value) {
        var buffer = ByteBuffer.allocate(Double.BYTES);
        buffer.setFloat64(value);
        assertEquals(value, buffer.getFloat64(0));
    }

    @Property
    void should_round_trip_int8_array(@ForAll byte[] values) {
        var buffer = ByteBuffer.allocate(Integer.BYTES + values.length);
        buffer.setInt8Array(values);
        assertArrayEquals(values, buffer.getInt8Array(0));
    }

    @Property
    void should_round_trip_uint8_array(@ForAll("uint8Arrays") short[] values) {
        var buffer = ByteBuffer.allocate(Integer.BYTES + values.length);
        buffer.setUint8Array(values);
        assertArrayEquals(values, buffer.getUint8Array(0));
    }

    @Property
    void should_round_trip_int16_array(@ForAll short[] values) {
        var buffer = ByteBuffer.allocate(Integer.BYTES + values.length * Short.BYTES);
        buffer.setInt16Array(values);
        assertArrayEquals(values, buffer.getInt16Array(0));
    }

    @Property
    void should_round_trip_uint16_array(@ForAll("uint16Arrays") int[] values) {
        var buffer = ByteBuffer.allocate(Integer.BYTES + values.length * Short.BYTES);
        buffer.setUint16Array(values);
        assertArrayEquals(values, buffer.getUint16Array(0));
    }

    @Property
    void should_round_trip_int32_array(@ForAll int[] values) {
        var buffer = ByteBuffer.allocate(Integer.BYTES + values.length * Integer.BYTES);
        buffer.setInt32Array(values);
        assertArrayEquals(values, buffer.getInt32Array(0));
    }

    @Property
    void should_round_trip_uint32_array(@ForAll("uint32Arrays") long[] values) {
        var buffer = ByteBuffer.allocate(Integer.BYTES + values.length * Integer.BYTES);
        buffer.setUint32Array(values);
        assertArrayEquals(values, buffer.getUint32Array(0));
    }

    @Property
    void should_round_trip_int64_array(@ForAll long[] values) {
        var buffer = ByteBuffer.allocate(Integer.BYTES + values.length * Long.BYTES);
        buffer.setInt64Array(values);
        assertArrayEquals(values, buffer.getInt64Array(0));
    }

    @Property
    void should_round_trip_uint64_array_as_its_bits(@ForAll long[] values) {
        var buffer = ByteBuffer.allocate(Integer.BYTES + values.length * Long.BYTES);
        buffer.setUint64Array(values);
        assertArrayEquals(values, buffer.getUint64Array(0));
    }

    @Property
    void should_round_trip_float32_array(@ForAll float[] values) {
        var buffer = ByteBuffer.allocate(Integer.BYTES + values.length * Float.BYTES);
        buffer.setFloat32Array(values);
        assertArrayEquals(values, buffer.getFloat32Array(0));
    }

    @Property
    void should_round_trip_float64_array(@ForAll double[] values) {
        var buffer = ByteBuffer.allocate(Integer.BYTES + values.length * Double.BYTES);
        buffer.setFloat64Array(values);
        assertArrayEquals(values, buffer.getFloat64Array(0));
    }

    @Property
    void should_round_trip_string(@ForAll String string) {
        var bytes = string.getBytes(StandardCharsets.UTF_8);
        var buffer = ByteBuffer.allocate(Integer.BYTES + bytes.length);
        buffer.setString(bytes);
        assertEquals(string, buffer.getString(0));
    }

    @Property
    void should_round_trip_vector(@ForAll @Size(max = 10) List<byte[]> items) {
        var vector = items.toArray(byte[][]::new);
        var byteLength = Integer.BYTES + items.stream().mapToInt(item -> Integer.BYTES + item.length).sum();
        var buffer = ByteBuffer.allocate(byteLength);
        buffer.setVector(vector);

        int[] offsets = buffer.getVectorOffsets(0);
        for (int i = 0; i < vector.length; i++) {
            assertArrayEquals(vector[i], Arrays.copyOfRange(buffer.bytes(), offsets[i], offsets[i] + vector[i].length));
        }
        assertArrayEquals(vector, buffer.getVector(0));
    }

    @Example
    void should_reject_a_size_beyond_the_buffer_before_allocating_for_it() {
        var buffer = new ByteBuffer(withSize(Integer.MAX_VALUE, 4));

        assertThrows(DecodeException.class, () -> buffer.getUint8Array(0));
        assertThrows(DecodeException.class, () -> buffer.getUint16Array(0));
        assertThrows(DecodeException.class, () -> buffer.getUint32Array(0));
        assertThrows(DecodeException.class, () -> buffer.getString(0));
        assertThrows(DecodeException.class, () -> buffer.getVectorOffsets(0));
    }

    @Example
    void should_reject_a_size_whose_byte_length_overflows_an_int() {
        // 2^29 + 1 doubles take 2^32 + 8 bytes, which an int wraps around to 8
        var buffer = new ByteBuffer(withSize((1 << 29) + 1, Double.BYTES));

        assertThrows(DecodeException.class, () -> buffer.getFloat64Array(0));
    }

    @Example
    void should_reject_a_size_beyond_the_largest_int() {
        var buffer = new ByteBuffer(withSize(-1, 4));

        assertThrows(DecodeException.class, () -> buffer.getInt8Array(0));
        assertThrows(DecodeException.class, () -> buffer.rootOffset(0));
        assertThrows(DecodeException.class, () -> buffer.fieldOffset(0, 0));
    }

    @Example
    void should_reject_a_vector_item_beyond_the_buffer() {
        // One item, whose size of 8 exceeds the 4 bytes after it
        var bytes = withSize(1, Integer.BYTES + Integer.BYTES);
        java.nio.ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).putInt(Integer.BYTES, 8);

        assertThrows(DecodeException.class, () -> new ByteBuffer(bytes).getVectorOffsets(0));
    }

    @Example
    void should_reject_a_16_bit_size_beyond_65535_and_write_nothing() {
        var layout = TableLayout.builder().offsetSize(AddressableSpace.SIZEOF_16).build();
        var buffer = new ByteBuffer(new byte[Short.BYTES + 65536], layout);

        assertThrows(IllegalArgumentException.class, () -> buffer.setInt8Array(new byte[65536]));
        assertThrows(IllegalArgumentException.class, () -> buffer.setVector(new byte[][] {new byte[65536]}));

        buffer.setInt8Array(new byte[] {7});
        assertArrayEquals(new byte[] {1, 0, 7}, Arrays.copyOf(buffer.bytes(), 3));
    }

    @Example
    void should_reject_writes_past_the_end_and_write_nothing() {
        var buffer = ByteBuffer.allocate(Integer.BYTES + 2 * Float.BYTES);

        assertThrows(IndexOutOfBoundsException.class, () -> buffer.setFloat32Array(new float[3]));
        assertThrows(IndexOutOfBoundsException.class, () -> buffer.setUint32Array(new long[3]));
        assertThrows(IndexOutOfBoundsException.class, () -> buffer.setString(new byte[9]));
        assertThrows(IndexOutOfBoundsException.class, () -> buffer.setVector(new byte[][] {new byte[5]}));
        assertThrows(IndexOutOfBoundsException.class, () -> buffer.setPadding(13));
        assertArrayEquals(new byte[12], buffer.bytes());

        buffer.setFloat32Array(new float[] {1, 2});
        assertArrayEquals(new float[] {1, 2}, buffer.getFloat32Array(0));
        assertThrows(IndexOutOfBoundsException.class, () -> buffer.setInt8((byte) 1));
    }

    @Example
    void should_read_the_size_of_a_sized_table_and_the_length_of_another() {
        var layout = TableLayout.builder().sized(true).build();
        var builder = new TableBuilder(1, layout);
        builder.setFloat64(1.5);
        var bytes = Arrays.copyOf(builder.bytes(), 20);

        assertEquals(16, new ByteBuffer(bytes, layout).getSize());
        assertEquals(20, new ByteBuffer(bytes).getSize());
    }

    // A 32-bit size followed by `length` bytes
    private static byte[] withSize(int size, int length) {
        var bytes = new byte[Integer.BYTES + length];
        java.nio.ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).putInt(0, size);
        return bytes;
    }

    @Provide
    Arbitrary<short[]> uint8Arrays() {
        return Arbitraries.shorts()
            .between((short) 0, (short) (Math.pow(2, 8) - 1))
            .array(short[].class);
    }

    @Provide
    Arbitrary<int[]> uint16Arrays() {
        return Arbitraries.integers()
            .between(0, (int) (Math.pow(2, 16) - 1))
            .array(int[].class);
    }

    @Provide
    Arbitrary<long[]> uint32Arrays() {
        return Arbitraries.longs()
            .between(0L, (long) (Math.pow(2, 32) - 1))
            .array(long[].class);
    }
}
