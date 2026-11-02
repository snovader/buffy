package io.github.snovader.tafla.test;

import io.github.snovader.tafla.*;
import io.github.snovader.tafla.test.FieldDescription.*;
import net.jqwik.api.*;
import net.jqwik.api.constraints.*;

import java.nio.ByteOrder;
import java.nio.ReadOnlyBufferException;
import java.nio.charset.*;
import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

public class ByteBufferTest {
    @Property
    void a_buffer_reads_back_the_value_that_its_setter_writes(
        @ForAll("values") FieldDescription field,
        @ForAll("layouts") LayoutDescription layout
    ) {
        assertReadsBack(field, layout);
    }

    @Property(generation = GenerationMode.EXHAUSTIVE)
    void a_buffer_reads_back_the_limits_of_every_type_as_scalars_and_in_arrays(@ForAll("limits") ScalarField limit) {
        var layout = LayoutDescription.headerless(AddressableSpace.SIZEOF_32);

        assertReadsBack(limit, layout);
        assertReadsBack(new ArrayField(limit.type(), new long[] {limit.value()}), layout);
    }

    @Property
    void unsigned_arrays_write_low_bits_and_preserve_the_following_field(
        @ForAll @Size(max = 65) long[] values,
        @ForAll AddressableSpace offsetSize
    ) {
        var layout = TableLayout.builder().offsetSize(offsetSize).build();
        for (var type : List.of(ScalarType.UINT8, ScalarType.UINT16, ScalarType.UINT32)) {
            var length = 1 + offsetSize.valueOf() + values.length * type.size() + Integer.BYTES;
            var buffer = new ByteBuffer(new byte[length], layout);
            buffer.setInt8((byte) 42);
            type.writeArray(buffer, values);
            buffer.setInt32(0x12345678);

            var expectedBytes = java.nio.ByteBuffer.allocate(length).order(ByteOrder.LITTLE_ENDIAN);
            expectedBytes.put((byte) 42);
            for (int b = 0; b < offsetSize.valueOf(); b++) {
                expectedBytes.put((byte) (values.length >>> (b * Byte.SIZE)));
            }

            var expectedValues = new long[values.length];
            var mask = (1L << (type.size() * Byte.SIZE)) - 1;
            for (int i = 0; i < values.length; i++) {
                type.put(expectedBytes, values[i]);
                expectedValues[i] = values[i] & mask;
            }
            expectedBytes.putInt(0x12345678);

            assertArrayEquals(expectedBytes.array(), buffer.bytes());
            assertArrayEquals(expectedValues, type.readArray(buffer, 1));
        }
    }

    private static void assertReadsBack(FieldDescription field, LayoutDescription layout) {
        var bytes = new byte[(int) Descriptions.maxLength(field, layout.width())];
        var buffer = new ByteBuffer(bytes, layout.tableLayout());

        write(buffer, field);
        assertEquals(field, readAtStart(buffer, field));

        // After a count of 1 or 2 bytes, the elements aren't aligned
        if (field instanceof ArrayField array) {
            assertArrayEquals(array.values(), array.type().readArrayView(buffer, 0));
        }
    }

    /** Writes a scalar, array or string from the buffer's position with the buffer's own setter */
    private static void write(ByteBuffer buffer, FieldDescription field) {
        switch (field) {
            case ScalarField scalar -> scalar.type().write(buffer, scalar.value());
            case ArrayField array -> array.type().writeArray(buffer, array.values());
            case StringField string -> buffer.setString(string.value().getBytes(StandardCharsets.UTF_8));
            default -> throw new IllegalArgumentException("Not a value: " + field);
        }
    }

    /** {@return the scalar, array or string of the field's type at the buffer's start} */
    private static FieldDescription readAtStart(ByteBuffer buffer, FieldDescription field) {
        return switch (field) {
            case ScalarField scalar -> new ScalarField(scalar.type(), scalar.type().read(buffer, 0));
            case ArrayField array -> new ArrayField(array.type(), array.type().readArray(buffer, 0));
            case StringField ignored -> new StringField(buffer.getString(0));
            default -> throw new IllegalArgumentException("Not a value: " + field);
        };
    }

    @Provide
    Arbitrary<FieldDescription> values() {
        return Descriptions.values();
    }

    @Provide
    Arbitrary<LayoutDescription> layouts() {
        return Descriptions.layouts();
    }

    @Provide
    Arbitrary<ScalarField> limits() {
        var limits = new ArrayList<ScalarField>();
        for (var type : ScalarType.values()) {
            for (var value : Descriptions.limits(type)) {
                limits.add(new ScalarField(type, value));
            }
        }

        return Arbitraries.of(limits);
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
        assertThrows(DecodeException.class, () -> buffer.getUint8ArrayView(0));
        assertThrows(DecodeException.class, () -> buffer.getUint32ArrayView(0));
        assertThrows(DecodeException.class, () -> buffer.getString(0));
        assertThrows(DecodeException.class, () -> buffer.getVectorOffsets(0));
    }

    @Example
    void should_reject_a_size_whose_byte_length_overflows_an_int() {
        // 2^29 + 1 doubles take 2^32 + 8 bytes, which an int wraps around to 8
        var buffer = new ByteBuffer(withSize((1 << 29) + 1, Double.BYTES));

        assertThrows(DecodeException.class, () -> buffer.getFloat64Array(0));
        assertThrows(DecodeException.class, () -> buffer.getFloat64ArrayView(0));
    }

    @Example
    void array_views_are_read_only_and_show_later_writes_to_the_bytes() {
        var buffer = ByteBuffer.allocate(Integer.BYTES + 2 * Float.BYTES);
        buffer.setFloat32Array(new float[] {1, 2});
        var view = buffer.getFloat32ArrayView(0);

        assertTrue(view.isReadOnly());
        assertThrows(ReadOnlyBufferException.class, () -> view.put(0, 3));

        // 1.0f is 0x3f800000, and 0x40800000 is 4.0f
        buffer.bytes()[Integer.BYTES + 3] = 0x40;
        assertEquals(4, view.get(0));
        assertEquals(2, view.get(1));
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
        assertThrows(IndexOutOfBoundsException.class, () -> buffer.setUint8Array(new short[9]));
        assertThrows(IndexOutOfBoundsException.class, () -> buffer.setUint16Array(new int[5]));
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

    @Example
    void should_reject_reading_the_version_of_an_unversioned_table() {
        var layout = TableLayout.builder().sized(true).build();
        var buffer = new ByteBuffer(new TableBuilder(0, layout).bytes(), layout);

        assertThrows(IllegalStateException.class, buffer::getVersion);
    }

    // A 32-bit size followed by `length` bytes
    private static byte[] withSize(int size, int length) {
        var bytes = new byte[Integer.BYTES + length];
        java.nio.ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).putInt(0, size);
        return bytes;
    }
}
