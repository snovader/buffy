package io.github.snovader.buffy.test;

import java.util.*;

/** The description of a field in the shape of {@code cases.json}, by its {@code type} */
sealed interface FieldDescription {
    /** An integer or float, held as {@link ScalarType} describes */
    record ScalarField(ScalarType type, long value) implements FieldDescription {
        @Override
        public String toString() {
            return DescriptionJson.write(this).toString();
        }
    }

    /**
     * An array of integers or floats, each held as {@link ScalarType} describes. It compares its
     * elements, and nothing changes them: the array is passed to setters as it is, to be written
     * when the table is.
     */
    @SuppressWarnings("ArrayRecordComponent")
    record ArrayField(ScalarType type, long[] values) implements FieldDescription {
        @Override
        public boolean equals(Object other) {
            return other instanceof ArrayField array && type == array.type && Arrays.equals(values, array.values);
        }

        @Override
        public int hashCode() {
            return 31 * type.hashCode() + Arrays.hashCode(values);
        }

        @Override
        public String toString() {
            return DescriptionJson.write(this).toString();
        }
    }

    record StringField(String value) implements FieldDescription {
        @Override
        public String toString() {
            return DescriptionJson.write(this).toString();
        }
    }

    /**
     * A table nested with {@code setTable()}
     *
     * @param alignment the alignment passed to {@code setTable()}, absent for its default
     */
    record TableField(Source from, TableDescription table, OptionalInt alignment) implements FieldDescription {
        @Override
        public String toString() {
            return DescriptionJson.write(this).toString();
        }
    }

    record VectorField(List<NestedTable> items) implements FieldDescription {
        public VectorField {
            items = List.copyOf(items);
        }

        @Override
        public String toString() {
            return DescriptionJson.write(this).toString();
        }
    }

    /** A table nested from its builder or from the bytes that its builder writes */
    record NestedTable(Source from, TableDescription table) {
        @Override
        public String toString() {
            return DescriptionJson.write(this).toString();
        }
    }

    /** What a nested table is passed to its parent's builder as */
    enum Source {
        BUILDER,
        BYTES;

        String jsonName() {
            return name().toLowerCase(Locale.ROOT);
        }
    }
}
