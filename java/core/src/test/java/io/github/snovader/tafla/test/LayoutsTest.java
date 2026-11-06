package io.github.snovader.tafla.test;

import io.github.snovader.tafla.*;
import io.github.snovader.tafla.test.generated.layouts.*;
import net.jqwik.api.*;
import net.jqwik.api.constraints.*;

import java.util.*;
import java.util.function.*;

import static io.github.snovader.tafla.test.DecodeAssertions.*;
import static io.github.snovader.tafla.test.FieldValues.*;
import static io.github.snovader.tafla.test.GeneratedDescriptions.*;
import static org.junit.jupiter.api.Assertions.*;

/**
 * The code generated from {@code schemas/layouts.tafla}, a message of the same fields in each of
 * the twelve layouts, writes and reads tables of each layout with tafla: each offset type, uint8,
 * uint16 and uint32, with and without the table's length and a version in its header.
 */
public class LayoutsTest {
    /** The fields of every message of the schema, which a test writes with any of them */
    @SuppressWarnings("ArrayRecordComponent") // Written, not compared
    record LayoutFields(long id, String name, int[] values) {
        @Override
        public String toString() {
            return String.format("LayoutFields[id=%d, name=%s, values=%s]", id, name, Arrays.toString(values));
        }
    }

    /**
     * A message of the schema, which shows as its name when a property fails: how a test writes it
     * and reads every field back with its generated class, and how it describes its table, in the
     * layout and version that the schema gives it, to build it by hand
     */
    record LayoutMessage<M>(
        String name,
        LayoutDescription layout,
        OptionalLong version,
        Function<LayoutFields, byte[]> serializer,
        Function<byte[], M> constructor,
        Function<M, LayoutFields> reader
    ) {
        byte[] serialize(LayoutFields fields) {
            return serializer.apply(fields);
        }

        LayoutFields read(byte[] bytes) {
            return reader.apply(constructor.apply(bytes));
        }

        TableDescription describe(LayoutFields fields) {
            var fieldDescriptions = List.<FieldDescription>of(
                uint32(fields.id()),
                string(fields.name()),
                uint16Array(fields.values())
            );

            return new TableDescription(layout, version, fieldDescriptions);
        }

        @Override
        public String toString() {
            return name;
        }
    }

    private static final List<LayoutMessage<?>> MESSAGES = List.of(
        new LayoutMessage<>(
            "Offset8",
            new LayoutDescription(false, false, OffsetSize.UINT8),
            OptionalLong.empty(),
            fields -> Offset8.serialize(new Offset8.Fields(fields.id(), fields.name(), fields.values())),
            Offset8::new,
            message -> new LayoutFields(message.id(), message.name(), message.values())
        ),
        new LayoutMessage<>(
            "Offset8Sized",
            new LayoutDescription(true, false, OffsetSize.UINT8),
            OptionalLong.empty(),
            fields -> Offset8Sized.serialize(new Offset8Sized.Fields(fields.id(), fields.name(), fields.values())),
            Offset8Sized::new,
            message -> new LayoutFields(message.id(), message.name(), message.values())
        ),
        new LayoutMessage<>(
            "Offset8Versioned",
            new LayoutDescription(false, true, OffsetSize.UINT8),
            OptionalLong.of(255),
            fields -> Offset8Versioned.serialize(
                new Offset8Versioned.Fields(fields.id(), fields.name(), fields.values())
            ),
            Offset8Versioned::new,
            message -> new LayoutFields(message.id(), message.name(), message.values())
        ),
        new LayoutMessage<>(
            "Offset8SizedVersioned",
            new LayoutDescription(true, true, OffsetSize.UINT8),
            OptionalLong.of(0),
            fields -> Offset8SizedVersioned.serialize(
                new Offset8SizedVersioned.Fields(fields.id(), fields.name(), fields.values())
            ),
            Offset8SizedVersioned::new,
            message -> new LayoutFields(message.id(), message.name(), message.values())
        ),
        new LayoutMessage<>(
            "Offset16",
            new LayoutDescription(false, false, OffsetSize.UINT16),
            OptionalLong.empty(),
            fields -> Offset16.serialize(new Offset16.Fields(fields.id(), fields.name(), fields.values())),
            Offset16::new,
            message -> new LayoutFields(message.id(), message.name(), message.values())
        ),
        new LayoutMessage<>(
            "Offset16Sized",
            new LayoutDescription(true, false, OffsetSize.UINT16),
            OptionalLong.empty(),
            fields -> Offset16Sized.serialize(new Offset16Sized.Fields(fields.id(), fields.name(), fields.values())),
            Offset16Sized::new,
            message -> new LayoutFields(message.id(), message.name(), message.values())
        ),
        new LayoutMessage<>(
            "Offset16Versioned",
            new LayoutDescription(false, true, OffsetSize.UINT16),
            OptionalLong.of(65_535),
            fields -> Offset16Versioned.serialize(
                new Offset16Versioned.Fields(fields.id(), fields.name(), fields.values())
            ),
            Offset16Versioned::new,
            message -> new LayoutFields(message.id(), message.name(), message.values())
        ),
        new LayoutMessage<>(
            "Offset16SizedVersioned",
            new LayoutDescription(true, true, OffsetSize.UINT16),
            OptionalLong.of(1),
            fields -> Offset16SizedVersioned.serialize(
                new Offset16SizedVersioned.Fields(fields.id(), fields.name(), fields.values())
            ),
            Offset16SizedVersioned::new,
            message -> new LayoutFields(message.id(), message.name(), message.values())
        ),
        new LayoutMessage<>(
            "Offset32",
            new LayoutDescription(false, false, OffsetSize.UINT32),
            OptionalLong.empty(),
            fields -> Offset32.serialize(new Offset32.Fields(fields.id(), fields.name(), fields.values())),
            Offset32::new,
            message -> new LayoutFields(message.id(), message.name(), message.values())
        ),
        new LayoutMessage<>(
            "Offset32Sized",
            new LayoutDescription(true, false, OffsetSize.UINT32),
            OptionalLong.empty(),
            fields -> Offset32Sized.serialize(new Offset32Sized.Fields(fields.id(), fields.name(), fields.values())),
            Offset32Sized::new,
            message -> new LayoutFields(message.id(), message.name(), message.values())
        ),
        new LayoutMessage<>(
            "Offset32Versioned",
            new LayoutDescription(false, true, OffsetSize.UINT32),
            OptionalLong.of(4_294_967_295L),
            fields -> Offset32Versioned.serialize(
                new Offset32Versioned.Fields(fields.id(), fields.name(), fields.values())
            ),
            Offset32Versioned::new,
            message -> new LayoutFields(message.id(), message.name(), message.values())
        ),
        new LayoutMessage<>(
            "Offset32SizedVersioned",
            new LayoutDescription(true, true, OffsetSize.UINT32),
            OptionalLong.of(2),
            fields -> Offset32SizedVersioned.serialize(
                new Offset32SizedVersioned.Fields(fields.id(), fields.name(), fields.values())
            ),
            Offset32SizedVersioned::new,
            message -> new LayoutFields(message.id(), message.name(), message.values())
        )
    );

    /** {@return the messages of uint8 and uint16 offsets, whose limits a test can reach} */
    private static List<LayoutMessage<?>> narrowMessages() {
        return MESSAGES.stream()
            .filter(message -> message.layout().offsetSize() != OffsetSize.UINT32)
            .toList();
    }

    /** A versioned message and another version than its own, which it rejects */
    record OtherVersion(LayoutMessage<?> message, long version) {}

    @Property
    void every_layout_reads_back_its_fields(
        @ForAll("messages") LayoutMessage<?> message,
        @ForAll("fields") LayoutFields fields
    ) {
        var bytes = message.serialize(fields);

        assertEquals(message.describe(fields), message.describe(message.read(bytes)));
    }

    @Property
    void every_layout_is_written_with_its_header_and_offsets_as_TableBuilder_writes_it(
        @ForAll("messages") LayoutMessage<?> message,
        @ForAll("fields") LayoutFields fields
    ) {
        var byHand = Tables.build(message.describe(fields)).bytes();

        assertArrayEquals(byHand, message.serialize(fields), () -> message.describe(fields).toString());
    }

    @Example
    void layouts_of_8_and_16_bit_offsets_read_back_fields_at_offsets_with_the_top_bit_set() {
        for (var message : narrowMessages()) {
            var offsetSize = message.layout().offsetSize();

            // A name that ends past 2^7 or 2^15 bytes, before the values
            var name = "a".repeat((int) (offsetSize.maxValue() / 2) + 1);
            var fields = new LayoutFields(0xffff_ffffL, name, new int[] {0, 0xffff});
            var bytes = message.serialize(fields);

            assertArrayEquals(Tables.build(message.describe(fields)).bytes(), bytes, message.name());
            assertEquals(message.describe(fields), message.describe(message.read(bytes)), message.name());
        }
    }

    @Example
    void layouts_of_8_and_16_bit_offsets_write_tables_as_long_as_they_address_and_throw_IllegalArgumentException_past_them() {
        for (var message : narrowMessages()) {
            var max = message.layout().offsetSize().maxValue();
            var withoutName = Tables.build(message.describe(named(""))).bytes().length;

            // Names that make the table a few bytes shorter than the longest it can be, to a few longer
            var longest = 0;
            var rejected = 0;
            for (var length = max - withoutName - 3; length <= max - withoutName + 3; length++) {
                var fields = named("a".repeat((int) length));

                byte[] byHand;
                try {
                    byHand = Tables.build(message.describe(fields)).bytes();
                } catch (IllegalArgumentException tooLong) {
                    assertThrows(IllegalArgumentException.class, () -> message.serialize(fields), message.name());
                    rejected++;
                    continue;
                }

                assertArrayEquals(byHand, message.serialize(fields), message.name());
                longest = Math.max(longest, byHand.length);
            }

            // The padding before the uint16s takes a byte or none, so the longest table is either
            // as long as the offsets address or a byte shorter
            assertTrue(longest >= max - 1, message.name() + "'s longest table is " + longest);
            assertTrue(rejected > 0, message.name());
        }
    }

    /** {@return the fields of a message named {@code name}, with the largest id and values} */
    private static LayoutFields named(String name) {
        return new LayoutFields(0xffff_ffffL, name, new int[] {0, 0xffff});
    }

    @Property
    void a_versioned_layout_throws_DecodeException_reading_a_table_of_another_version(
        @ForAll("otherVersions") OtherVersion otherVersion,
        @ForAll("fields") LayoutFields fields
    ) {
        var message = otherVersion.message();
        var bytes = Tables.build(message.describe(fields).withVersion(otherVersion.version())).bytes();

        assertThrows(DecodeException.class, () -> message.constructor().apply(bytes));
    }

    @Property
    void a_corrupted_message_of_any_layout_reads_as_values_or_throws_DecodeException(
        @ForAll("messages") LayoutMessage<?> message,
        @ForAll("fields") LayoutFields fields,
        @ForAll("corruptions") Corruption corruption
    ) {
        var bytes = corruption.applyTo(message.serialize(fields));

        assertValuesOrDecodeException(() -> message.read(bytes));
    }

    @Property
    void any_bytes_read_as_a_message_of_any_layout_or_throw_DecodeException(@ForAll @Size(max = 64) byte[] bytes) {
        for (var message : MESSAGES) {
            assertValuesOrDecodeException(() -> message.read(bytes));
        }
    }

    @Provide
    Arbitrary<LayoutMessage<?>> messages() {
        return Arbitraries.of(MESSAGES);
    }

    @Provide
    Arbitrary<LayoutFields> fields() {
        return Combinators.combine(uint32s(), strings(), uint16Arrays()).as(LayoutFields::new);
    }

    /** {@return versioned messages, each with any version that fits its slot but its own} */
    @Provide
    Arbitrary<OtherVersion> otherVersions() {
        var versioned = MESSAGES.stream().filter(message -> message.version().isPresent()).toList();
        return Arbitraries.of(versioned).flatMap(LayoutsTest::otherVersionsOf);
    }

    private static Arbitrary<OtherVersion> otherVersionsOf(LayoutMessage<?> message) {
        var own = message.version().orElseThrow();
        var versions = Arbitraries.longs().between(0, message.layout().offsetSize().maxValue());

        return versions.filter(version -> version != own).map(version -> new OtherVersion(message, version));
    }

    @Provide
    Arbitrary<Corruption> corruptions() {
        return Corruption.corruptions();
    }
}
