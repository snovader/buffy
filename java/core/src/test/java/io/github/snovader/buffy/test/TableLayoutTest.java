package io.github.snovader.buffy.test;

import io.github.snovader.buffy.*;
import net.jqwik.api.*;

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

    @Property(generation = GenerationMode.EXHAUSTIVE)
    void the_header_has_a_slot_of_the_offset_size_for_the_length_and_for_the_version(
        @ForAll boolean sized,
        @ForAll boolean versioned,
        @ForAll AddressableSpace offsetSize
    ) {
        var layout = TableLayout.builder().sized(sized).versioned(versioned).offsetSize(offsetSize).build();
        var slots = (sized ? 1 : 0) + (versioned ? 1 : 0);

        assertEquals(slots * offsetSize.valueOf(), layout.headerSize());
    }
}
