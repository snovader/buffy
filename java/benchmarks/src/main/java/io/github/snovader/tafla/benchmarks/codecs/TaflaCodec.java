package io.github.snovader.tafla.benchmarks.codecs;

import io.github.snovader.tafla.*;
import io.github.snovader.tafla.benchmarks.*;

import java.util.*;

/**
 * tafla's tables, nested from their builders. The fixed-size record is written with
 * {@link ByteBuffer}'s setters and read with its getters at fixed positions, since Java has no
 * {@code StructWriter}.
 */
public final class TaflaCodec {
    private static final TableLayout DOCUMENT_LAYOUT = TableLayout.builder().sized(true).versioned(true).build();
    private static final int DOCUMENT_VERSION = 1;

    private static final int SAMPLE_SIZE = Integer.BYTES + 2 * Float.BYTES;

    private TaflaCodec() {}

    public static Codec create() {
        return new Codec("tafla", new Records(), new Documents(), new Lists(), new Samples());
    }

    private static final class Records implements Workload<RecordInput> {
        @Override
        public byte[] serialize(RecordInput input) {
            var builder = new TableBuilder(4);
            builder.setFloat64(input.x());
            builder.setFloat64(input.y());
            builder.setInt64(input.id());
            builder.setString(input.name());

            return builder.bytes();
        }

        @Override
        public RecordInput deserialize(byte[] bytes) {
            var buffer = new ByteBuffer(bytes);

            return new RecordInput(
                buffer.getFloat64(buffer.rootOffset(0)),
                buffer.getFloat64(buffer.rootOffset(1)),
                buffer.getInt64(buffer.rootOffset(2)),
                buffer.getString(buffer.rootOffset(3))
            );
        }
    }

    private static final class Documents implements DocumentWorkload {
        @Override
        public byte[] serialize(DocumentInput input) {
            var columns = new TableBuilder(4);
            columns.setInt64(input.base());
            columns.setUint32Array(input.keys());
            columns.setFloat32Array(input.a());
            columns.setFloat32Array(input.b());

            var document = new TableBuilder(4, DOCUMENT_LAYOUT, DOCUMENT_VERSION);
            document.setFloat64(input.x());
            document.setFloat64(input.y());
            document.setString(input.name());
            document.setTable(columns);

            return document.bytes();
        }

        @Override
        public DocumentInput deserialize(byte[] bytes) {
            var buffer = openDocument(bytes);
            var columns = buffer.rootOffset(3);

            return new DocumentInput(
                buffer.getFloat64(buffer.rootOffset(0)),
                buffer.getFloat64(buffer.rootOffset(1)),
                buffer.getString(buffer.rootOffset(2)),
                buffer.getInt64(buffer.fieldOffset(columns, 0)),
                buffer.getUint32Array(buffer.fieldOffset(columns, 1)),
                buffer.getFloat32Array(buffer.fieldOffset(columns, 2)),
                buffer.getFloat32Array(buffer.fieldOffset(columns, 3))
            );
        }

        @Override
        public double sumA(byte[] bytes) {
            var buffer = openDocument(bytes);
            var a = buffer.fieldOffset(buffer.rootOffset(3), 2);

            // The elements follow their count, which is as wide as the layout's 32-bit offsets. An
            // int count makes this a counted loop, which C2 optimizes; a long one made summing 100
            // elements 2.8 times slower.
            var length = Math.toIntExact(buffer.getUint32(a));
            var elements = a + Integer.BYTES;

            double sum = 0;
            for (int i = 0; i < length; i++) {
                sum += buffer.getFloat32(elements + i * Float.BYTES);
            }

            return sum;
        }

        /** @throws IllegalStateException if the document is in another version */
        private static ByteBuffer openDocument(byte[] bytes) {
            var buffer = new ByteBuffer(bytes, DOCUMENT_LAYOUT);
            if (buffer.getVersion() != DOCUMENT_VERSION) {
                throw new IllegalStateException("Unsupported document version " + buffer.getVersion());
            }

            return buffer;
        }
    }

    private static final class Lists implements Workload<ListInput> {
        @Override
        public byte[] serialize(ListInput input) {
            var items = input.items();
            var tables = new ArrayList<TableBuilder>(items.size());
            for (var item : items) {
                tables.add(itemTable(item));
            }

            var builder = new TableBuilder(1);
            builder.setVector(tables);

            return builder.bytes();
        }

        @Override
        public ListInput deserialize(byte[] bytes) {
            var buffer = new ByteBuffer(bytes);
            var offsets = buffer.getVectorOffsets(buffer.rootOffset(0));

            var items = new ItemInput[offsets.length];
            for (int i = 0; i < offsets.length; i++) {
                var item = offsets[i];
                var pair = buffer.fieldOffset(item, 1);

                items[i] = new ItemInput(
                    buffer.getFloat32(buffer.fieldOffset(item, 0)),
                    buffer.getFloat32(buffer.fieldOffset(pair, 0)),
                    buffer.getFloat32(buffer.fieldOffset(pair, 1))
                );
            }

            return new ListInput(Arrays.asList(items));
        }

        private static TableBuilder itemTable(ItemInput input) {
            var pair = new TableBuilder(2);
            pair.setFloat32(input.first());
            pair.setFloat32(input.second());

            var item = new TableBuilder(2);
            item.setFloat32(input.value());
            item.setTable(pair);

            return item;
        }
    }

    private static final class Samples implements Workload<SampleInput> {
        @Override
        public byte[] serialize(SampleInput input) {
            var buffer = ByteBuffer.allocate(SAMPLE_SIZE);
            buffer.setUint32(input.key());
            buffer.setFloat32(input.a());
            buffer.setFloat32(input.b());

            return buffer.bytes();
        }

        @Override
        public SampleInput deserialize(byte[] bytes) {
            var buffer = new ByteBuffer(bytes);

            return new SampleInput(
                buffer.getUint32(0),
                buffer.getFloat32(Integer.BYTES),
                buffer.getFloat32(Integer.BYTES + Float.BYTES)
            );
        }
    }
}
