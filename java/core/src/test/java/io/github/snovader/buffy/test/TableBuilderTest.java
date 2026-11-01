package io.github.snovader.buffy.test;

import io.github.snovader.buffy.*;
import net.jqwik.api.*;
import net.jqwik.api.constraints.*;

import java.util.*;

import static io.github.snovader.buffy.test.Field.*;
import static org.junit.jupiter.api.Assertions.*;

public class TableBuilderTest {
    private static final TableLayout SIZED = TableLayout.builder().sized(true).build();

    @Example
    void should_reject_a_field_beyond_the_number_the_table_holds() {
        var builder = new TableBuilder(1);
        builder.setUint8((short) 1);

        assertThrows(IllegalStateException.class, () -> builder.setUint8((short) 2));
        assertArrayEquals(new byte[] {4, 0, 0, 0, 1}, builder.bytes());
    }

    @Property
    void should_reject_writing_before_every_field_is_set(@ForAll @IntRange(max = 4) int set) {
        var builder = new TableBuilder(set + 1);
        for (int i = 0; i < set; i++) {
            builder.setFloat64(i);
        }

        var exception = assertThrows(IllegalStateException.class, builder::bytes);
        assertEquals(
            String.format("Only %d of the table's %d fields were set", set, set + 1),
            exception.getMessage()
        );
    }

    @Example
    void should_reject_setting_a_field_after_writing() {
        var builder = new TableBuilder(0);
        builder.bytes();

        assertThrows(IllegalStateException.class, () -> builder.setInt8((byte) 1));
    }

    @Example
    void should_write_once_and_return_the_same_bytes() {
        var builder = new TableBuilder(1);
        builder.setString("once");

        assertSame(builder.bytes(), builder.bytes());
    }

    @Example
    void should_write_array_elements_as_they_are_when_writing() {
        var values = new float[] {1, 2};
        var builder = new TableBuilder(1);
        builder.setFloat32Array(values);
        values[0] = 3;

        var buffer = new ByteBuffer(builder.bytes());
        assertArrayEquals(new float[] {3, 2}, buffer.getFloat32Array(buffer.rootOffset(0)));
    }

    @Example
    void should_write_the_version_after_the_length() {
        // Length, version (7), offset of the field and the field (42)
        assertVersionedTable(AddressableSpace.SIZEOF_8, true, new byte[] {4, 7, 3, 42});
        assertVersionedTable(AddressableSpace.SIZEOF_16, true, new byte[] {7, 0, 7, 0, 6, 0, 42});
        assertVersionedTable(
            AddressableSpace.SIZEOF_32,
            true,
            new byte[] {13, 0, 0, 0, 7, 0, 0, 0, 12, 0, 0, 0, 42}
        );
        assertVersionedTable(AddressableSpace.SIZEOF_32, false, new byte[] {7, 0, 0, 0, 8, 0, 0, 0, 42});
    }

    private static void assertVersionedTable(AddressableSpace size, boolean sized, byte[] expected) {
        var layout = TableLayout.builder().sized(sized).versioned(true).offsetSize(size).build();
        var builder = new TableBuilder(1, layout, 7);
        builder.setInt8((byte) 42);
        var bytes = builder.bytes();

        var buffer = new ByteBuffer(bytes, layout);

        assertArrayEquals(expected, bytes);
        assertEquals(7, buffer.getVersion());
        assertEquals(42, buffer.getInt8(buffer.rootOffset(0)));
    }

    @Example
    void should_write_the_largest_version_that_its_slot_holds() {
        var layout = TableLayout.builder().versioned(true).build();
        var buffer = new ByteBuffer(new TableBuilder(0, layout, 4294967295L).bytes(), layout);

        assertEquals(4294967295L, buffer.getVersion());
    }

    @Example
    void should_reject_a_version_that_its_slot_cannot_hold() {
        var oneByte = versioned(AddressableSpace.SIZEOF_8);
        var twoBytes = versioned(AddressableSpace.SIZEOF_16);
        var fourBytes = versioned(AddressableSpace.SIZEOF_32);

        assertThrows(IllegalArgumentException.class, () -> new TableBuilder(1, oneByte, 256));
        assertThrows(IllegalArgumentException.class, () -> new TableBuilder(1, twoBytes, 65536));
        assertThrows(IllegalArgumentException.class, () -> new TableBuilder(1, fourBytes, 1L << 32));
        assertThrows(IllegalArgumentException.class, () -> new TableBuilder(1, fourBytes, -1));
    }

    @Example
    void should_need_a_version_exactly_when_the_layout_has_a_slot_for_it() {
        var exception = assertThrows(
            IllegalArgumentException.class,
            () -> new TableBuilder(1, versioned(AddressableSpace.SIZEOF_32))
        );
        assertEquals("A versioned layout needs a version", exception.getMessage());

        exception = assertThrows(IllegalArgumentException.class, () -> new TableBuilder(1, SIZED, 1));
        assertEquals("The layout has no version slot", exception.getMessage());
    }

    @Example
    void should_reject_reading_the_version_of_an_unversioned_table() {
        var buffer = new ByteBuffer(new TableBuilder(0, SIZED).bytes(), SIZED);

        assertThrows(IllegalStateException.class, buffer::getVersion);
    }

    @Example
    void should_write_a_lone_surrogate_as_the_replacement_character() {
        var builder = new TableBuilder(1);
        builder.setString("a\uD800b\uDC00\uD83D\uDE00");

        var buffer = new ByteBuffer(builder.bytes());
        assertEquals("a\uFFFDb\uFFFD\uD83D\uDE00", buffer.getString(buffer.rootOffset(0)));
    }

    @Property
    void should_fit_a_field_up_to_the_largest_table_and_reject_one_element_more(
        @ForAll("limitedSizes") AddressableSpace size,
        @ForAll boolean sized,
        @ForAll("growingFields") GrowingField field
    ) {
        var layout = TableLayout.builder().sized(sized).offsetSize(size).build();
        var max = (int) size.maxValue();

        var empty = new TableBuilder(1, layout);
        field.ofLength(0, size).setOn(empty);
        var length = (max - empty.bytes().length) / field.unit(size);

        var longest = new TableBuilder(1, layout);
        var longestField = field.ofLength(length, size);
        longestField.setOn(longest);

        var buffer = new ByteBuffer(longest.bytes(), layout);
        assertTrue(buffer.bytes().length <= max);
        assertTrue(buffer.bytes().length > max - field.unit(size));
        assertTrue(longestField.matches(longestField.readFrom(buffer, buffer.rootOffset(0))));

        // The setter throws and leaves the builder as it was
        var tooLong = new TableBuilder(1, layout);
        assertThrows(IllegalArgumentException.class, () -> field.ofLength(length + 1, size).setOn(tooLong));
        field.ofLength(0, size).setOn(tooLong);
        assertArrayEquals(empty.bytes(), tooLong.bytes());
    }

    @Property
    void should_reject_the_field_that_makes_the_table_longer_than_its_offsets_address(
        @ForAll("limitedSizes") AddressableSpace size
    ) {
        var layout = TableLayout.builder().offsetSize(size).build();
        var max = (int) size.maxValue();

        // Two offsets, the array's length prefix and bytes, and then one byte
        var fits = new TableBuilder(2, layout);
        fits.setInt8Array(new byte[max - 3 * size.valueOf() - 1]);
        fits.setUint8((short) 7);

        var buffer = new ByteBuffer(fits.bytes(), layout);
        assertEquals(max, buffer.bytes().length);
        assertEquals(7, buffer.getUint8(buffer.rootOffset(1)));

        var tooLong = new TableBuilder(2, layout);
        tooLong.setInt8Array(new byte[max - 3 * size.valueOf()]);
        assertThrows(IllegalArgumentException.class, () -> tooLong.setUint8((short) 7));
    }

    @Property
    void should_reject_a_header_longer_than_the_offsets_address(
        @ForAll("limitedSizes") AddressableSpace size,
        @ForAll boolean sized
    ) {
        var layout = TableLayout.builder().sized(sized).offsetSize(size).build();
        var fields = (int) (size.maxValue() / size.valueOf()) - (sized ? 1 : 0);

        assertDoesNotThrow(() -> new TableBuilder(fields, layout));
        assertThrows(IllegalArgumentException.class, () -> new TableBuilder(fields + 1, layout));
    }

    @Example
    void should_reject_a_table_of_32_bit_offsets_longer_than_a_java_array_before_allocating_it() {
        // One mebibyte, repeated past 2 GiB
        var item = new byte[1 << 20];
        var vector = new byte[2048][];
        Arrays.fill(vector, item);

        var builder = new TableBuilder(1);
        assertThrows(IllegalArgumentException.class, () -> builder.setVector(vector));

        builder.setVector(new byte[][] {{1}});
        var buffer = new ByteBuffer(builder.bytes());
        assertArrayEquals(new byte[][] {{1}}, buffer.getVector(buffer.rootOffset(0)));
    }

    @Provide
    Arbitrary<AddressableSpace> limitedSizes() {
        return Arbitraries.of(AddressableSpace.SIZEOF_8, AddressableSpace.SIZEOF_16);
    }

    @Provide
    Arbitrary<GrowingField> growingFields() {
        return Arbitraries.of(GrowingField.values());
    }

    /** A field of any length, and the bytes that each element adds */
    enum GrowingField {
        BYTE_ARRAY,
        TYPED_ARRAY,
        STRING,
        VECTOR_COUNT,
        VECTOR_ITEM;

        int unit(AddressableSpace size) {
            return switch (this) {
                case TYPED_ARRAY -> Double.BYTES;
                case VECTOR_COUNT -> size.valueOf();
                default -> Byte.BYTES;
            };
        }

        Field ofLength(int length, AddressableSpace size) {
            return switch (this) {
                case BYTE_ARRAY -> new Field(Field.Type.INT8_ARRAY, new byte[length]);
                case TYPED_ARRAY -> new Field(Field.Type.FLOAT64_ARRAY, new double[length]);
                case STRING -> new Field(Field.Type.STRING, "a".repeat(length));
                case VECTOR_COUNT -> {
                    var vector = new byte[length][];
                    Arrays.fill(vector, new byte[0]);
                    yield new Field(Field.Type.VECTOR, vector);
                }
                case VECTOR_ITEM -> new Field(Field.Type.VECTOR, new byte[][] {new byte[length]});
            };
        }
    }

    @Property
    void should_write_the_same_bytes_as_nesting_the_tables_bytes_at_its_arrays_alignment(
        @ForAll @Size(max = 6) List<@From("fields") Field> parentFields,
        @ForAll @Size(max = 6) List<@From("fields") Field> nestedFields
    ) {
        var withBuilder = builderOf(parentFields, 1, TableLayout.DEFAULT);
        withBuilder.setTable(builderOf(nestedFields));

        var withBytes = builderOf(parentFields, 1, TableLayout.DEFAULT);
        withBytes.setTable(builderOf(nestedFields).bytes(), alignmentOf(nestedFields));

        assertArrayEquals(withBytes.bytes(), withBuilder.bytes());
    }

    @Property
    void should_read_back_tables_nested_two_deep(
        @ForAll @Size(max = 6) List<@From("fields") Field> outerFields,
        @ForAll @Size(max = 6) List<@From("fields") Field> middleFields,
        @ForAll @Size(max = 6) List<@From("fields") Field> innerFields
    ) {
        var middle = builderOf(middleFields, 1, TableLayout.DEFAULT);
        middle.setTable(builderOf(innerFields));

        var outer = builderOf(outerFields, 1, SIZED);
        outer.setTable(middle);

        var buffer = new ByteBuffer(outer.bytes(), SIZED);
        var middlePosition = buffer.rootOffset(outerFields.size());
        var innerPosition = buffer.fieldOffset(middlePosition, middleFields.size());

        for (int i = 0; i < innerFields.size(); i++) {
            var field = innerFields.get(i);
            assertTrue(field.matches(field.readFrom(buffer, buffer.fieldOffset(innerPosition, i))));
        }
    }

    @Property
    void should_write_a_vector_of_builders_as_a_vector_of_their_bytes(
        @ForAll @Size(max = 5) List<@Size(max = 6) List<@From("fields") Field>> items
    ) {
        var withBuilders = new TableBuilder(1);
        withBuilders.setVector(items.stream().map(Field::builderOf).toList());

        var withBytes = new TableBuilder(1);
        withBytes.setVector(items.stream().map(item -> builderOf(item).bytes()).toArray(byte[][]::new));

        assertArrayEquals(withBytes.bytes(), withBuilders.bytes());
    }

    @Example
    void should_pad_a_builder_without_arrays_by_nothing() {
        var nested = new TableBuilder(1);
        nested.setFloat64(1.5);

        var parent = new TableBuilder(2);
        parent.setUint8((short) 1);
        parent.setTable(nested);

        assertEquals(2 * Integer.BYTES + 1, new ByteBuffer(parent.bytes()).rootOffset(1));
    }

    @Example
    void should_reject_a_builder_with_fields_left_to_set_and_leave_the_table_as_it_was() {
        var nested = new TableBuilder(2);
        nested.setUint8((short) 1);

        var parent = new TableBuilder(2);
        var exception = assertThrows(IllegalArgumentException.class, () -> parent.setTable(nested));
        assertEquals("Only 1 of the nested table's 2 fields were set", exception.getMessage());
        assertThrows(IllegalArgumentException.class, () -> parent.setVector(List.of(nested)));

        nested.setUint8((short) 2);
        parent.setTable(nested);
        parent.setVector(List.of(nested));

        var buffer = new ByteBuffer(parent.bytes());
        var table = buffer.rootOffset(0);
        assertEquals(2, buffer.getUint8(buffer.fieldOffset(table, 1)));
        assertArrayEquals(new byte[][] {nested.bytes()}, buffer.getVector(buffer.rootOffset(1)));
    }

    @Example
    void should_reject_a_builder_that_makes_its_parent_too_long_and_leave_it_as_it_was() {
        var nested = new TableBuilder(1);
        nested.setInt8Array(new byte[255 - 8]);

        var parent = new TableBuilder(1, TableLayout.builder().offsetSize(AddressableSpace.SIZEOF_8).build());
        assertThrows(IllegalArgumentException.class, () -> parent.setTable(nested));
        assertThrows(IllegalArgumentException.class, () -> parent.setVector(List.of(nested)));

        parent.setVector(List.of());
        assertArrayEquals(new byte[] {1, 0}, parent.bytes());
    }

    @Example
    void should_write_a_builder_nested_several_times_or_written_itself_the_same_each_time() {
        var nested = new TableBuilder(2);
        nested.setFloat64Array(new double[] {1, 2});
        nested.setString("nested");

        // Written straight into the parent, and then copied once written itself
        var first = new TableBuilder(2);
        first.setTable(nested);
        first.setVector(List.of(nested, nested));
        var written = first.bytes();

        var own = nested.bytes();
        var second = new TableBuilder(2);
        second.setTable(nested);
        second.setVector(List.of(nested, nested));

        assertArrayEquals(written, second.bytes());

        var buffer = new ByteBuffer(second.bytes());
        assertArrayEquals(new byte[][] {own, own}, buffer.getVector(buffer.rootOffset(1)));
    }

    @Property
    void should_copy_a_builder_of_other_offsets_in_a_vector_as_its_own_bytes(
        @ForAll AddressableSpace nestedSize,
        @ForAll AddressableSpace parentSize
    ) {
        var layout = TableLayout.builder().offsetSize(nestedSize).build();
        var nested = new TableBuilder(2, layout);
        nested.setString("another width");
        nested.setUint16Array(new int[] {1, 2});

        var parentLayout = TableLayout.builder().offsetSize(parentSize).build();
        var parent = new TableBuilder(1, parentLayout);
        parent.setVector(List.of(nested));

        var buffer = new ByteBuffer(parent.bytes(), parentLayout);
        var item = buffer.getVector(buffer.rootOffset(0))[0];
        assertArrayEquals(nested.bytes(), item);

        var read = new ByteBuffer(item, layout);
        assertEquals("another width", read.getString(read.rootOffset(0)));
    }

    @Example
    void should_write_a_nested_builders_array_elements_as_they_are_when_its_parent_is_written() {
        var values = new float[] {1, 2};
        var nested = new TableBuilder(1);
        nested.setFloat32Array(values);

        var parent = new TableBuilder(1);
        parent.setTable(nested);
        values[0] = 3;

        var buffer = new ByteBuffer(parent.bytes());
        var read = buffer.getFloat32Array(buffer.fieldOffset(buffer.rootOffset(0), 0));
        assertArrayEquals(new float[] {3, 2}, read);
    }

    @Example
    void should_write_a_vector_as_it_was_set() {
        var items = new ArrayList<TableBuilder>();
        items.add(new TableBuilder(0));

        var parent = new TableBuilder(1);
        parent.setVector(items);
        items.add(new TableBuilder(0));

        var buffer = new ByteBuffer(parent.bytes());
        assertEquals(1, buffer.getVectorOffsets(buffer.rootOffset(0)).length);
    }

    @Provide
    Arbitrary<Field> fields() {
        return Field.any();
    }

    private static TableLayout versioned(AddressableSpace size) {
        return TableLayout.builder().versioned(true).offsetSize(size).build();
    }
}
