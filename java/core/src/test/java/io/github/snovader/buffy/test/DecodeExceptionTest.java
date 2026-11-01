package io.github.snovader.buffy.test;

import io.github.snovader.buffy.*;
import net.jqwik.api.*;
import net.jqwik.api.constraints.*;

import java.nio.charset.*;
import java.util.*;
import java.util.function.*;

import static org.junit.jupiter.api.Assertions.*;

public class DecodeExceptionTest {
    @Property
    void should_read_each_field_of_a_truncated_table_or_throw_DecodeException(
        @ForAll @Size(min = 1, max = 8) List<@From("fields") Field> fields,
        @ForAll boolean sized
    ) {
        var layout = TableLayout.builder().sized(sized).build();
        var bytes = Field.builderOf(fields, 0, layout).bytes();

        for (int length = 0; length < bytes.length; length++) {
            var buffer = new ByteBuffer(Arrays.copyOf(bytes, length), layout);

            for (int i = 0; i < fields.size(); i++) {
                var field = fields.get(i);
                var result = read(buffer, field.type(), i);

                if (result instanceof Exception exception) {
                    assertInstanceOf(DecodeException.class, exception);
                } else {
                    assertTrue(field.matches(result));
                }
            }
        }
    }

    @Property
    void should_throw_only_DecodeException_for_any_bytes(
        @ForAll @Size(max = 64) byte[] bytes,
        @ForAll Field.Type type,
        @ForAll @IntRange(max = 16) int index,
        @ForAll boolean sized,
        @ForAll AddressableSpace offsetSize
    ) {
        var layout = TableLayout.builder().sized(sized).offsetSize(offsetSize).build();
        var result = read(new ByteBuffer(bytes, layout), type, index);

        if (result instanceof Exception exception) {
            assertInstanceOf(DecodeException.class, exception);
        }
    }

    @Example
    void should_throw_DecodeException_from_scalar_getters_outside_the_bytes() {
        var buffer = new ByteBuffer(new byte[8]);
        List<Consumer<ByteBuffer>> reads = List.of(
            b -> b.getFloat64(4),
            b -> b.getInt64(-1),
            b -> b.getUint32(5),
            b -> b.getInt8(8),
            b -> b.getUint16(7)
        );

        for (var read : reads) {
            var exception = assertThrows(DecodeException.class, () -> read.accept(buffer));
            assertTrue(Objects.requireNonNull(exception.getMessage()).startsWith("Read exceeds the supplied buffer"));
        }
    }

    @Example
    void should_name_the_read_and_the_bytes_available() {
        var exception = assertThrows(DecodeException.class, () -> new ByteBuffer(new byte[8]).getFloat64(4));

        assertEquals("Read exceeds the supplied buffer: 8 bytes at 4 of 8", exception.getMessage());
    }

    @Example
    void should_throw_DecodeException_for_invalid_strings() {
        var invalid = List.of(
            new byte[] {(byte) 0x80}, // A lone continuation byte
            new byte[] {(byte) 0xE2, (byte) 0x82}, // A truncated sequence
            new byte[] {(byte) 0xC0, (byte) 0xAF}, // An overlong encoding
            new byte[] {(byte) 0xED, (byte) 0xA0, (byte) 0x80}, // An encoded surrogate
            new byte[] {(byte) 0xEF, (byte) 0xBF, (byte) 0xBD, (byte) 0xFF} // U+FFFD, then an invalid byte
        );

        for (var utf8 : invalid) {
            var buffer = ByteBuffer.allocate(Integer.BYTES + utf8.length);
            buffer.setString(utf8);

            var exception = assertThrows(DecodeException.class, () -> buffer.getString(0));
            assertInstanceOf(CharacterCodingException.class, exception.getCause());
        }
    }

    @Property
    void should_read_back_any_well_formed_string_with_a_leading_U_FEFF(@ForAll("wellFormed") String string) {
        for (var value : List.of(string, "\uFEFF" + string, "\uFFFD" + string)) {
            var builder = new TableBuilder(1);
            builder.setString(value);

            var buffer = new ByteBuffer(builder.bytes());
            assertEquals(value, buffer.getString(buffer.rootOffset(0)));
        }
    }

    @Example
    void should_throw_DecodeException_for_an_offset_beyond_the_largest_int() {
        // An offset of 2^32 - 1, which once threw an AssertionError
        var bytes = new byte[] {-1, -1, -1, -1};

        assertThrows(DecodeException.class, () -> new ByteBuffer(bytes).rootOffset(0));
        assertThrows(DecodeException.class, () -> new ByteBuffer(bytes).fieldOffset(0, 0));
        assertEquals(4294967295L, new ByteBuffer(bytes, TableLayout.builder().sized(true).build()).getSize());
    }

    /** Reads field {@code index} as {@code type}, returning the exception it throws instead */
    private static Object read(ByteBuffer buffer, Field.Type type, int index) {
        try {
            return new Field(type, 0).readFrom(buffer, buffer.rootOffset(index));
        } catch (RuntimeException ex) {
            return ex;
        }
    }

    @Provide
    Arbitrary<Field> fields() {
        return Field.any();
    }

    /** Strings of any code points, including those outside the Basic Multilingual Plane */
    @Provide
    Arbitrary<String> wellFormed() {
        return Arbitraries.integers()
            .between(0, Character.MAX_CODE_POINT)
            .filter(codePoint -> codePoint < Character.MIN_SURROGATE || codePoint > Character.MAX_SURROGATE)
            .list()
            .ofMaxSize(16)
            .map(codePoints -> codePoints.stream()
                .collect(StringBuilder::new, StringBuilder::appendCodePoint, StringBuilder::append)
                .toString());
    }
}
