package io.github.snovader.buffy.test;

import io.github.snovader.buffy.*;
import net.jqwik.api.*;
import net.jqwik.api.constraints.*;

import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

public class TableLayoutTest {
    @Example
    void should_default_to_no_header_slots_and_32_bit_offsets() {
        for (var layout : List.of(TableLayout.DEFAULT, TableLayout.builder().build())) {
            assertFalse(layout.sized());
            assertFalse(layout.versioned());
            assertEquals(AddressableSpace.SIZEOF_32, layout.offsetSize());
            assertEquals(0, layout.headerSize());
        }
    }

    @Example
    void should_have_a_header_slot_of_the_offset_size_for_the_length_and_the_version() {
        assertEquals(0, headerSize(false, false, AddressableSpace.SIZEOF_8));
        assertEquals(1, headerSize(true, false, AddressableSpace.SIZEOF_8));
        assertEquals(2, headerSize(false, true, AddressableSpace.SIZEOF_16));
        assertEquals(4, headerSize(true, true, AddressableSpace.SIZEOF_16));
        assertEquals(8, headerSize(true, true, AddressableSpace.SIZEOF_32));
    }

    @Property
    void should_read_the_fields_of_a_nested_table(
        @ForAll @Size(max = 6) List<@From("fields") Field> parentFields,
        @ForAll @Size(max = 6) List<@From("fields") Field> nestedFields
    ) {
        var nested = Field.builderOf(nestedFields);

        var layout = TableLayout.builder().sized(true).build();
        var parent = Field.builderOf(parentFields, 1, layout);
        parent.setTable(nested.bytes());

        var buffer = new ByteBuffer(parent.bytes(), layout);
        var tablePosition = buffer.rootOffset(parentFields.size());

        for (int i = 0; i < nestedFields.size(); i++) {
            var field = nestedFields.get(i);
            assertTrue(field.matches(field.readFrom(buffer, buffer.fieldOffset(tablePosition, i))));
        }
    }

    @Property
    void should_start_a_nested_table_at_a_multiple_of_the_alignment(
        @ForAll String string,
        @ForAll("alignments") int alignment
    ) {
        var builder = new TableBuilder(2);
        builder.setString(string);
        builder.setTable(new byte[] {1, 2, 3}, alignment);

        var bytes = builder.bytes();
        var position = new ByteBuffer(bytes).rootOffset(1);

        assertEquals(0, position % alignment);
        assertArrayEquals(new byte[] {1, 2, 3}, Arrays.copyOfRange(bytes, position, bytes.length));
    }

    @Example
    void should_reject_an_alignment_other_than_an_element_size() {
        var builder = new TableBuilder(1);

        assertThrows(IllegalArgumentException.class, () -> builder.setTable(new byte[0], 0));
        assertThrows(IllegalArgumentException.class, () -> builder.setTable(new byte[0], 3));
        assertThrows(IllegalArgumentException.class, () -> builder.setTable(new byte[0], 16));
    }

    private static int headerSize(boolean sized, boolean versioned, AddressableSpace offsetSize) {
        return TableLayout.builder()
            .sized(sized)
            .versioned(versioned)
            .offsetSize(offsetSize)
            .build()
            .headerSize();
    }

    @Provide
    Arbitrary<Field> fields() {
        return Field.any();
    }

    @Provide
    Arbitrary<Integer> alignments() {
        return Arbitraries.of(1, 2, 4, 8);
    }
}
