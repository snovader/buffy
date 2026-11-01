package io.github.snovader.buffy.test;

import io.github.snovader.buffy.test.FieldDescription.*;
import net.jqwik.api.*;

import java.nio.*;
import java.nio.charset.*;
import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Tables follow the format's rules as {@code SPEC.md} states them. The properties read the bytes
 * that a builder writes with the JDK's little-endian buffer rather than buffy's reader, and check
 * the fields of the root table. A nested table holds the bytes that its builder writes on its own,
 * so the rules hold for nested tables as they do for the roots of the same descriptions.
 */
public class FormatTest {
    @Property
    void a_sized_tables_size_slot_holds_its_byte_length(@ForAll("sizedTables") TableDescription table) {
        var bytes = Tables.build(table).bytes();

        assertEquals(bytes.length, unsigned(bytes, 0, table.layout().width()));
    }

    @Property
    void a_versioned_tables_version_slot_holds_its_version(@ForAll("versionedTables") TableDescription table) {
        var bytes = Tables.build(table).bytes();
        var layout = table.layout();
        var slot = layout.sized() ? layout.width() : 0;

        assertEquals(table.version().orElseThrow(), unsigned(bytes, slot, layout.width()));
    }

    @Property
    void root_offsets_lie_within_the_table_in_field_order(@ForAll("tables") TableDescription table) {
        var bytes = Tables.build(table).bytes();
        var layout = table.layout();
        var fields = table.fields().size();

        // A table nested with no fields and no header has no bytes, so it can start where the table ends
        long previous = layout.headerSize() + (long) fields * layout.width();
        for (int index = 0; index < fields; index++) {
            var offset = rootOffset(bytes, layout, index);

            assertTrue(
                previous <= offset && offset <= bytes.length,
                String.format("Offset %d of field %d isn't within [%d, %d]", offset, index, previous, bytes.length)
            );
            previous = offset;
        }
    }

    @Property
    void every_array_starts_at_a_multiple_of_its_element_size_from_the_start_of_its_table(
        @ForAll("tables") TableDescription table
    ) {
        var bytes = Tables.build(table).bytes();
        var layout = table.layout();

        for (int index = 0; index < table.fields().size(); index++) {
            if (table.fields().get(index) instanceof ArrayField array) {
                var elements = rootOffset(bytes, layout, index) + layout.width();
                assertEquals(0, elements % array.type().size(), "The elements of field " + index);
            }
        }
    }

    @Property
    void multi_byte_values_are_little_endian(@ForAll("tables") TableDescription table) {
        var bytes = Tables.build(table).bytes();
        var layout = table.layout();

        for (int index = 0; index < table.fields().size(); index++) {
            var field = table.fields().get(index);
            var offset = rootOffset(bytes, layout, index);

            littleEndian(field, layout.width()).ifPresent(
                expected -> assertArrayEquals(expected, slice(bytes, offset, expected.length), field.toString())
            );
        }
    }

    // Tables of many nested tables, so fewer of them
    @Property(tries = 250)
    void a_nested_table_holds_the_bytes_of_its_builder_at_a_multiple_of_its_alignment(
        @ForAll("nestingTables") TableDescription table
    ) {
        var bytes = Tables.build(table).bytes();
        var layout = table.layout();

        for (int index = 0; index < table.fields().size(); index++) {
            if (table.fields().get(index) instanceof TableField field) {
                var offset = rootOffset(bytes, layout, index);
                var nested = Tables.build(field.table()).bytes();

                assertEquals(0, offset % alignmentOf(field), "The offset of field " + index);
                assertArrayEquals(nested, slice(bytes, offset, nested.length), "The bytes of field " + index);
            }
        }
    }

    // Tables of many nested tables, so fewer of them
    @Property(tries = 250)
    void a_vector_holds_its_item_count_and_then_each_items_length_and_bytes(
        @ForAll("nestingTables") TableDescription table
    ) {
        var bytes = Tables.build(table).bytes();
        var layout = table.layout();
        var width = layout.width();

        for (int index = 0; index < table.fields().size(); index++) {
            if (table.fields().get(index) instanceof VectorField vector) {
                var position = rootOffset(bytes, layout, index);
                assertEquals(vector.items().size(), unsigned(bytes, position, width), "The count of field " + index);
                position += width;

                for (var item : vector.items()) {
                    var itemBytes = Tables.build(item.table()).bytes();
                    assertEquals(itemBytes.length, unsigned(bytes, position, width), "The length of " + item);
                    position += width;

                    assertArrayEquals(itemBytes, slice(bytes, position, itemBytes.length), "The bytes of " + item);
                    position += itemBytes.length;
                }
            }
        }
    }

    @Provide
    Arbitrary<TableDescription> tables() {
        return Descriptions.tables();
    }

    @Provide
    Arbitrary<TableDescription> nestingTables() {
        return Descriptions.nestingTables();
    }

    @Provide
    Arbitrary<TableDescription> sizedTables() {
        return Descriptions.tables(LayoutDescription::sized);
    }

    @Provide
    Arbitrary<TableDescription> versionedTables() {
        return Descriptions.tables(LayoutDescription::versioned);
    }

    /**
     * {@return the alignment that {@code SPEC.md} gives a nested table}: the one passed to
     * {@code setTable()}, or by default 4 for bytes, and for a builder the largest element size of
     * its arrays and alignment of its own nested tables, or 1 if it has neither
     */
    private static int alignmentOf(TableField field) {
        if (field.alignment().isPresent()) {
            return field.alignment().getAsInt();
        }

        if (field.from() == Source.BYTES) {
            return Integer.BYTES;
        }

        var alignment = 1;
        for (var nested : field.table().fields()) {
            var fieldAlignment = switch (nested) {
                case ArrayField array -> array.type().size();
                case TableField table -> alignmentOf(table);
                default -> 1;
            };
            alignment = Math.max(alignment, fieldAlignment);
        }

        return alignment;
    }

    /**
     * {@return the bytes of a scalar, array or string, as the JDK writes them little-endian}, or
     * nothing for a nested table or vector
     */
    private static Optional<byte[]> littleEndian(FieldDescription field, int width) {
        return switch (field) {
            case ScalarField scalar -> Optional.of(littleEndian(scalar));
            case ArrayField array -> Optional.of(littleEndian(array, width));
            case StringField string -> Optional.of(littleEndian(string, width));
            case TableField ignored -> Optional.empty();
            case VectorField ignored -> Optional.empty();
        };
    }

    private static byte[] littleEndian(ScalarField scalar) {
        var bytes = new byte[scalar.type().size()];
        scalar.type().put(littleEndianView(bytes), scalar.value());

        return bytes;
    }

    /** {@return an array's count and then its elements} */
    private static byte[] littleEndian(ArrayField array, int width) {
        var bytes = new byte[width + array.values().length * array.type().size()];

        var view = littleEndianView(bytes);
        putUnsigned(view, width, array.values().length);
        for (var value : array.values()) {
            array.type().put(view, value);
        }

        return bytes;
    }

    /** {@return a string's count and then its UTF-8} */
    private static byte[] littleEndian(StringField string, int width) {
        var utf8 = string.value().getBytes(StandardCharsets.UTF_8);
        var bytes = new byte[width + utf8.length];

        var view = littleEndianView(bytes);
        putUnsigned(view, width, utf8.length);
        view.put(utf8);

        return bytes;
    }

    private static ByteBuffer littleEndianView(byte[] bytes) {
        return ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);
    }

    private static void putUnsigned(ByteBuffer bytes, int width, int value) {
        switch (width) {
            case Byte.BYTES -> bytes.put((byte) value);
            case Short.BYTES -> bytes.putShort((short) value);
            default -> bytes.putInt(value);
        }
    }

    /** {@return the offset of a root field, read as the format describes it} */
    private static int rootOffset(byte[] bytes, LayoutDescription layout, int index) {
        return (int) unsigned(bytes, layout.headerSize() + index * layout.width(), layout.width());
    }

    /** {@return the little-endian unsigned integer of {@code width} bytes at {@code position}} */
    private static long unsigned(byte[] bytes, int position, int width) {
        var view = littleEndianView(bytes);

        return switch (width) {
            case Byte.BYTES -> Byte.toUnsignedLong(view.get(position));
            case Short.BYTES -> Short.toUnsignedLong(view.getShort(position));
            default -> Integer.toUnsignedLong(view.getInt(position));
        };
    }

    /** {@return a copy of {@code length} bytes from {@code position}, which must be within the bytes} */
    private static byte[] slice(byte[] bytes, int position, int length) {
        Objects.checkFromIndexSize(position, length, bytes.length);
        return Arrays.copyOfRange(bytes, position, position + length);
    }
}
