package io.github.snovader.buffy.test;

import io.github.snovader.buffy.*;
import jakarta.json.*;
import net.jqwik.api.*;

import java.io.*;
import java.lang.reflect.Array;
import java.nio.charset.*;
import java.nio.file.*;
import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Reads and rebuilds the tables of buffy's conformance suite, which the TypeScript runtime writes.
 * See {@code conformance/README.md} at the root of the repository for the format of their
 * descriptions in {@code cases.json}.
 */
public class ConformanceTest {
    private static final Path CONFORMANCE = Path.of(System.getProperty("buffy.conformance"));
    private static final JsonObject SUITE = readSuite();

    /** A case of the suite, which shows as its name when a property fails */
    record Case(JsonObject description) {
        String name() {
            return description.getString("name");
        }

        byte[] bytes() {
            try {
                return Files.readAllBytes(CONFORMANCE.resolve(description.getString("file")));
            } catch (IOException ex) {
                throw new UncheckedIOException(ex);
            }
        }

        @Override
        public String toString() {
            return name();
        }
    }

    private static JsonObject readSuite() {
        try (var reader = Json.createReader(
            Files.newBufferedReader(CONFORMANCE.resolve("cases.json"), StandardCharsets.UTF_8)
        )) {
            return reader.readObject();
        } catch (IOException ex) {
            throw new UncheckedIOException(ex);
        }
    }

    private static List<Case> cases(String kind) {
        return SUITE.getJsonArray(kind).stream().map(value -> new Case(value.asJsonObject())).toList();
    }

    @Provide
    Arbitrary<Case> validCases() {
        return Arbitraries.of(cases("valid"));
    }

    @Provide
    Arbitrary<Case> invalidCases() {
        return Arbitraries.of(cases("invalid"));
    }

    @Property(generation = GenerationMode.EXHAUSTIVE, shrinking = ShrinkingMode.OFF)
    void should_read_every_valid_table(@ForAll("validCases") Case validCase) {
        var table = validCase.description().getJsonObject("table");
        var reader = new TableReader(validCase.bytes(), table.getJsonObject("layout"));

        reader.readTable(table, validCase.name());
    }

    @Property(generation = GenerationMode.EXHAUSTIVE, shrinking = ShrinkingMode.OFF)
    void should_rebuild_every_valid_table_byte_for_byte(@ForAll("validCases") Case validCase) {
        var table = validCase.description().getJsonObject("table");

        assertArrayEquals(validCase.bytes(), build(table).bytes());
    }

    @Property(generation = GenerationMode.EXHAUSTIVE, shrinking = ShrinkingMode.OFF)
    void should_read_invalid_tables_up_to_the_field_that_throws(@ForAll("invalidCases") Case invalidCase) {
        var description = invalidCase.description();
        var table = cases("valid").stream()
            .filter(validCase -> validCase.name().equals(description.getString("case")))
            .findFirst()
            .orElseThrow()
            .description()
            .getJsonObject("table");

        var fields = table.getJsonArray("fields");
        var field = description.getInt("field");
        var reader = new TableReader(invalidCase.bytes(), table.getJsonObject("layout"));

        for (int index = 0; index < field; index++) {
            reader.readRootField(index, fields.getJsonObject(index), invalidCase.name() + "/" + index);
        }

        assertThrows(
            DecodeException.class,
            () -> reader.readRootField(field, fields.getJsonObject(field), invalidCase.name() + "/" + field)
        );
    }

    /** {@return a builder of the table as its description sets it out} */
    private static TableBuilder build(JsonObject table) {
        var layout = layoutOf(table.getJsonObject("layout"));
        var fields = table.getJsonArray("fields");

        var builder = layout.versioned()
            ? new TableBuilder(fields.size(), layout, table.getJsonNumber("version").longValueExact())
            : new TableBuilder(fields.size(), layout);

        for (var field : fields) {
            setField(builder, field.asJsonObject());
        }

        return builder;
    }

    private static void setField(TableBuilder builder, JsonObject field) {
        switch (field.getString("type")) {
            case "table" -> setTable(builder, field);
            case "vector" -> setVector(builder, field.getJsonArray("items"));
            default -> fieldOf(field).setOn(builder);
        }
    }

    private static void setTable(TableBuilder builder, JsonObject field) {
        var nested = build(field.getJsonObject("table"));
        var fromBuilder = field.getString("from").equals("builder");

        if (!field.containsKey("alignment")) {
            if (fromBuilder) {
                builder.setTable(nested);
            } else {
                builder.setTable(nested.bytes());
            }
            return;
        }

        var alignment = field.getInt("alignment");
        if (fromBuilder) {
            builder.setTable(nested, alignment);
        } else {
            builder.setTable(nested.bytes(), alignment);
        }
    }

    /**
     * Sets a vector from its items' builders when they're all builders. {@link TableBuilder} takes a
     * vector of builders or one of bytes, so a vector with bytes is set from every item's bytes, which
     * a builder writes as its parent would.
     */
    private static void setVector(TableBuilder builder, JsonArray items) {
        var builders = items.stream()
            .map(item -> build(item.asJsonObject().getJsonObject("table")))
            .toList();
        var fromBuilders = items.stream()
            .allMatch(item -> item.asJsonObject().getString("from").equals("builder"));

        if (fromBuilders) {
            builder.setVector(builders);
        } else {
            builder.setVector(builders.stream().map(TableBuilder::bytes).toArray(byte[][]::new));
        }
    }

    private static TableLayout layoutOf(JsonObject layout) {
        var offsetSize = switch (layout.getInt("offsetSize")) {
            case 1 -> AddressableSpace.SIZEOF_8;
            case 2 -> AddressableSpace.SIZEOF_16;
            case 4 -> AddressableSpace.SIZEOF_32;
            default -> throw new IllegalArgumentException("Offsets can't be " + layout.get("offsetSize") + " bytes wide");
        };

        return TableLayout.builder()
            .sized(layout.getBoolean("sized"))
            .versioned(layout.getBoolean("versioned"))
            .offsetSize(offsetSize)
            .build();
    }

    /** {@return a scalar, array or string field with its value as {@link Field} holds it} */
    private static Field fieldOf(JsonObject field) {
        var type = Field.Type.valueOf(field.getString("type").replace("[]", "_array").toUpperCase(Locale.ROOT));

        if (type == Field.Type.STRING) {
            return new Field(type, field.getString("value"));
        }

        if (!type.name().endsWith("_ARRAY")) {
            return new Field(type, scalar(type, Objects.requireNonNull(field.get("value"))));
        }

        var elementType = Field.Type.valueOf(type.name().replace("_ARRAY", ""));
        var values = field.getJsonArray("value");
        // An array of the primitive type that the scalar's value unboxes to
        var elements = Array.newInstance(primitiveOf(elementType), values.size());
        for (int i = 0; i < values.size(); i++) {
            Array.set(elements, i, scalar(elementType, values.get(i)));
        }

        return new Field(type, elements);
    }

    /**
     * {@return a scalar from the JSON that holds 64-bit integers as strings and the floats that JSON
     * has no number for by name, boxed as {@link Field} holds it}
     */
    private static Object scalar(Field.Type type, JsonValue value) {
        return switch (type) {
            case INT8 -> Byte.valueOf((byte) integer(value, Byte.MIN_VALUE, Byte.MAX_VALUE));
            case UINT8 -> Short.valueOf((short) integer(value, 0, 0xff));
            case INT16 -> Short.valueOf((short) integer(value, Short.MIN_VALUE, Short.MAX_VALUE));
            case UINT16 -> Integer.valueOf((int) integer(value, 0, 0xffff));
            case INT32 -> Integer.valueOf((int) integer(value, Integer.MIN_VALUE, Integer.MAX_VALUE));
            case UINT32 -> Long.valueOf(integer(value, 0, 0xffff_ffffL));
            case INT64 -> Long.valueOf(Long.parseLong(((JsonString) value).getString()));
            case UINT64 -> Long.valueOf(Long.parseUnsignedLong(((JsonString) value).getString()));
            case FLOAT32 -> Float.valueOf((float) float64(value));
            case FLOAT64 -> Double.valueOf(float64(value));
            default -> throw new IllegalArgumentException(type + " isn't a scalar");
        };
    }

    private static Class<?> primitiveOf(Field.Type type) {
        return switch (type) {
            case INT8 -> byte.class;
            case UINT8, INT16 -> short.class;
            case UINT16, INT32 -> int.class;
            case UINT32, INT64, UINT64 -> long.class;
            case FLOAT32 -> float.class;
            case FLOAT64 -> double.class;
            default -> throw new IllegalArgumentException(type + " isn't a scalar");
        };
    }

    private static long integer(JsonValue value, long min, long max) {
        var integer = ((JsonNumber) value).longValueExact();
        if (integer < min || integer > max) {
            throw new IllegalArgumentException(String.format("%d isn't within [%d, %d]", integer, min, max));
        }

        return integer;
    }

    private static double float64(JsonValue value) {
        if (value instanceof JsonNumber number) {
            return number.doubleValue();
        }

        var name = ((JsonString) value).getString();
        return switch (name) {
            case "NaN" -> Double.NaN;
            case "Infinity" -> Double.POSITIVE_INFINITY;
            case "-Infinity" -> Double.NEGATIVE_INFINITY;
            case "-0" -> -0.0;
            default -> throw new IllegalArgumentException("Not a number: " + name);
        };
    }

    /**
     * Reads the bytes of a table as descriptions of its fields set them out, and asserts that they
     * hold the described values.
     *
     * <p>A nested table, from {@link TableBuilder#setTable} or in a vector, is read in place when it
     * has no header and its offsets are as wide as its parent's. Any other is read as a table of its
     * own: a vector item from its bytes, and a field from its position to the end of its parent.</p>
     */
    private static final class TableReader {
        private final ByteBuffer buffer;
        private final AddressableSpace offsetSize;

        TableReader(byte[] bytes, JsonObject layout) {
            var tableLayout = layoutOf(layout);

            this.buffer = new ByteBuffer(bytes, tableLayout);
            this.offsetSize = tableLayout.offsetSize();
        }

        /** Reads the header and every field of the table, which must have the reader's layout */
        void readTable(JsonObject table, String path) {
            var layout = table.getJsonObject("layout");

            if (layout.getBoolean("sized")) {
                assertEquals(buffer.bytes().length, buffer.getSize(), path + ": size");
            }

            if (layout.getBoolean("versioned")) {
                var version = table.getJsonNumber("version").longValueExact();
                assertEquals(version, buffer.getVersion(), path + ": version");
            }

            var fields = table.getJsonArray("fields");
            for (int index = 0; index < fields.size(); index++) {
                readRootField(index, fields.getJsonObject(index), path + "/" + index);
            }
        }

        void readRootField(int index, JsonObject field, String path) {
            readField(buffer.rootOffset(index), field, path);
        }

        private void readField(int position, JsonObject field, String path) {
            switch (field.getString("type")) {
                case "table" -> readNested(position, field.getJsonObject("table"), path);
                case "vector" -> readVector(position, field.getJsonArray("items"), path);
                default -> {
                    var expected = fieldOf(field);
                    var read = expected.readFrom(buffer, position);

                    assertTrue(
                        expected.matches(read),
                        () -> String.format("%s: expected %s, read %s", path, field.get("value"), describe(read))
                    );
                }
            }
        }

        /** Reads a table nested with {@link TableBuilder#setTable} */
        private void readNested(int position, JsonObject table, String path) {
            if (readsInPlace(table)) {
                readInPlace(position, table, path);
                return;
            }

            // Its length isn't written, so it's read up to the end of its parent. A position past
            // the end leaves no bytes, which the first read throws for.
            var bytes = buffer.bytes();
            var rest = Arrays.copyOfRange(bytes, Math.min(position, bytes.length), bytes.length);
            new TableReader(rest, table.getJsonObject("layout")).readTable(table, path);
        }

        private void readVector(int position, JsonArray items, String path) {
            var offsets = buffer.getVectorOffsets(position);
            assertEquals(items.size(), offsets.length, path + ": items");

            var vector = buffer.getVector(position);
            for (int index = 0; index < offsets.length; index++) {
                var table = items.getJsonObject(index).getJsonObject("table");
                var itemPath = path + "/" + index;

                if (readsInPlace(table)) {
                    readInPlace(offsets[index], table, itemPath);
                } else {
                    new TableReader(vector[index], table.getJsonObject("layout")).readTable(table, itemPath);
                }
            }
        }

        private boolean readsInPlace(JsonObject table) {
            var layout = table.getJsonObject("layout");
            return !layout.getBoolean("sized")
                && !layout.getBoolean("versioned")
                && layout.getInt("offsetSize") == offsetSize.valueOf();
        }

        private void readInPlace(int position, JsonObject table, String path) {
            var fields = table.getJsonArray("fields");
            for (int index = 0; index < fields.size(); index++) {
                readField(buffer.fieldOffset(position, index), fields.getJsonObject(index), path + "/" + index);
            }
        }

        private static String describe(Object read) {
            // A primitive array shows its elements in an array of arrays
            return Arrays.deepToString(new Object[] {read});
        }
    }
}
