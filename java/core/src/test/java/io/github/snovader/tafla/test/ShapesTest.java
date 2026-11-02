package io.github.snovader.tafla.test;

import io.github.snovader.tafla.*;
import io.github.snovader.tafla.test.generated.shapes.*;
import net.jqwik.api.*;
import net.jqwik.api.constraints.*;

import static io.github.snovader.tafla.test.DecodeAssertions.*;
import static io.github.snovader.tafla.test.GeneratedDescriptions.*;
import static io.github.snovader.tafla.test.ShapesTables.*;
import static org.junit.jupiter.api.Assertions.*;

/**
 * The code generated from {@code schemas/shapes.tafla} writes and reads tables with tafla. Values
 * read back compare with the values written by their descriptions, which compare floats by their
 * bits and arrays by their elements.
 */
public class ShapesTest {
    // Round trips

    @Property
    void a_point_reads_back_its_fields(@ForAll("points") Point.Fields fields) {
        var bytes = Point.serialize(fields);

        assertEquals(describePoint(fields), describePoint(readPoint(new Point(bytes))));
    }

    @Property
    void a_document_reads_back_its_fields_and_those_of_its_nested_columns(@ForAll("documents") Document.Fields fields) {
        var bytes = Document.serialize(fields);

        assertEquals(describeDocument(fields), describeDocument(readDocument(new Document(bytes))));
    }

    @Property
    void an_item_list_reads_back_its_items_and_their_pairs(@ForAll("itemLists") ItemList.Fields fields) {
        var bytes = ItemList.serialize(fields);

        assertEquals(describeItemList(fields), describeItemList(readItemList(new ItemList(bytes))));
    }

    // Tables built by hand

    @Property
    void a_point_is_written_as_TableBuilder_writes_it(@ForAll("points") Point.Fields fields) {
        var byHand = Tables.build(describePoint(fields)).bytes();

        assertArrayEquals(byHand, Point.serialize(fields), () -> describePoint(fields).toString());
    }

    @Property
    void a_document_is_written_sized_and_versioned_as_TableBuilder_writes_it(
        @ForAll("documents") Document.Fields fields
    ) {
        var byHand = Tables.build(describeDocument(fields)).bytes();

        assertArrayEquals(byHand, Document.serialize(fields), () -> describeDocument(fields).toString());
    }

    @Property
    void an_item_list_is_written_as_TableBuilder_writes_it(@ForAll("itemLists") ItemList.Fields fields) {
        var byHand = Tables.build(describeItemList(fields)).bytes();

        assertArrayEquals(byHand, ItemList.serialize(fields), () -> describeItemList(fields).toString());
    }

    // Decode exceptions

    @Property
    void a_document_of_another_version_throws_DecodeException(
        @ForAll("documents") Document.Fields fields,
        @ForAll @LongRange(max = 0xffff_ffffL) long version
    ) {
        Assume.that(version != DOCUMENT_VERSION);
        var bytes = Tables.build(describeDocument(fields).withVersion(version)).bytes();

        assertThrows(DecodeException.class, () -> new Document(bytes));
    }

    @Property
    void a_corrupted_point_reads_as_values_or_throws_DecodeException(
        @ForAll("points") Point.Fields fields,
        @ForAll("corruptions") Corruption corruption
    ) {
        var bytes = corruption.applyTo(Point.serialize(fields));

        assertValuesOrDecodeException(() -> readPoint(new Point(bytes)));
    }

    @Property
    void a_corrupted_document_reads_as_values_or_throws_DecodeException(
        @ForAll("documents") Document.Fields fields,
        @ForAll("corruptions") Corruption corruption
    ) {
        var bytes = corruption.applyTo(Document.serialize(fields));

        assertValuesOrDecodeException(() -> readDocument(new Document(bytes)));
    }

    @Property
    void a_corrupted_item_list_reads_as_values_or_throws_DecodeException(
        @ForAll("itemLists") ItemList.Fields fields,
        @ForAll("corruptions") Corruption corruption
    ) {
        var bytes = corruption.applyTo(ItemList.serialize(fields));

        assertValuesOrDecodeException(() -> readItemList(new ItemList(bytes)));
    }

    @Property
    void any_bytes_read_as_any_message_or_throw_DecodeException(@ForAll @Size(max = 64) byte[] bytes) {
        assertValuesOrDecodeException(() -> readPoint(new Point(bytes)));
        assertValuesOrDecodeException(() -> readDocument(new Document(bytes)));
        assertValuesOrDecodeException(() -> readItemList(new ItemList(bytes)));
    }

    // The struct

    @Property
    void a_sample_is_written_in_its_12_bytes_little_endian_at_fixed_offsets(@ForAll("samples") Sample sample) {
        var bytes = sample.serialize();

        assertEquals(12, Sample.SIZE);
        assertArrayEquals(structBytes(describeSample(sample)), bytes);
    }

    @Property
    void a_sample_reads_back_at_any_offset_in_longer_bytes(
        @ForAll("samples") Sample sample,
        @ForAll @Size(max = 16) byte[] before,
        @ForAll @Size(max = 8) byte[] after
    ) {
        var bytes = java.nio.ByteBuffer.allocate(before.length + Sample.SIZE + after.length)
            .put(before)
            .put(sample.serialize())
            .put(after)
            .array();

        assertEquals(describeSample(sample), describeSample(Sample.read(bytes, before.length)));
    }

    @Property
    void reading_a_sample_throws_DecodeException_where_its_bytes_end_past_the_bytes_or_the_view(
        @ForAll @Size(max = 32) byte[] bytes,
        @ForAll("offsets") int offset
    ) {
        // The view ends where `bytes` do, within longer bytes, as a view of a struct array does
        var view = java.nio.ByteBuffer.wrap(java.util.Arrays.copyOf(bytes, bytes.length + Sample.SIZE))
            .slice(0, bytes.length)
            .order(java.nio.ByteOrder.LITTLE_ENDIAN);

        if (offset >= 0 && (long) offset + Sample.SIZE <= bytes.length) {
            assertDoesNotThrow(() -> Sample.read(bytes, offset));
            assertDoesNotThrow(() -> Sample.read(view, offset));
        } else {
            assertThrows(DecodeException.class, () -> Sample.read(bytes, offset));
            assertThrows(DecodeException.class, () -> Sample.read(view, offset));
        }
    }

    @Provide
    Arbitrary<Point.Fields> points() {
        return pointFields();
    }

    @Provide
    Arbitrary<Document.Fields> documents() {
        return documentFields();
    }

    @Provide
    Arbitrary<ItemList.Fields> itemLists() {
        return itemListFields();
    }

    @Provide
    Arbitrary<Sample> samples() {
        return sampleFields();
    }

    @Provide
    Arbitrary<Corruption> corruptions() {
        return Corruption.corruptions();
    }

    /** {@return offsets mostly within or around a few dozen bytes, and sometimes any int} */
    @Provide
    Arbitrary<Integer> offsets() {
        return Arbitraries.frequencyOf(
            Tuple.of(4, Arbitraries.integers().between(-8, 40)),
            Tuple.of(1, Arbitraries.integers())
        );
    }
}
