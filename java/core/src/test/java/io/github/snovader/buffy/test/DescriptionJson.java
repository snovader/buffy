package io.github.snovader.buffy.test;

import io.github.snovader.buffy.*;
import io.github.snovader.buffy.test.FieldDescription.*;
import jakarta.json.*;

import java.util.*;

/**
 * Reads descriptions from the JSON of {@code cases.json}, and writes them back to it, which is how
 * a description shows in a failed assertion or a property's counterexample. JSON holds 64-bit
 * integers as decimal strings, and the floats that it has no number for by name.
 */
final class DescriptionJson {
    /** The bits of the quiet NaN, which {@code "NaN"} names, as a float32 and a float64 */
    private static final int QUIET_NAN_32 = 0x7fc0_0000;
    private static final long QUIET_NAN_64 = 0x7ff8_0000_0000_0000L;

    private DescriptionJson() {}

    static TableDescription readTable(JsonObject table) {
        var layout = readLayout(table.getJsonObject("layout"));
        var version = layout.versioned()
            ? OptionalLong.of(table.getJsonNumber("version").longValueExact())
            : OptionalLong.empty();
        var fields = table.getJsonArray("fields").stream()
            .map(field -> readField(field.asJsonObject()))
            .toList();

        return new TableDescription(layout, version, fields);
    }

    private static LayoutDescription readLayout(JsonObject layout) {
        var offsetSize = switch (layout.getInt("offsetSize")) {
            case 1 -> AddressableSpace.SIZEOF_8;
            case 2 -> AddressableSpace.SIZEOF_16;
            case 4 -> AddressableSpace.SIZEOF_32;
            default -> throw new IllegalArgumentException("Offsets can't be " + layout.get("offsetSize") + " bytes wide");
        };

        return new LayoutDescription(layout.getBoolean("sized"), layout.getBoolean("versioned"), offsetSize);
    }

    private static FieldDescription readField(JsonObject field) {
        var type = field.getString("type");

        return switch (type) {
            case "string" -> new StringField(field.getString("value"));
            case "table" -> new TableField(
                readFrom(field),
                readTable(field.getJsonObject("table")),
                field.containsKey("alignment") ? OptionalInt.of(field.getInt("alignment")) : OptionalInt.empty()
            );
            case "vector" -> new VectorField(
                field.getJsonArray("items").stream().map(item -> readNestedTable(item.asJsonObject())).toList()
            );
            default -> type.endsWith("[]")
                ? readArray(scalarType(type.substring(0, type.length() - "[]".length())), field.getJsonArray("value"))
                : new ScalarField(scalarType(type), readValue(scalarType(type), Objects.requireNonNull(field.get("value"))));
        };
    }

    private static NestedTable readNestedTable(JsonObject item) {
        return new NestedTable(readFrom(item), readTable(item.getJsonObject("table")));
    }

    private static Source readFrom(JsonObject nested) {
        return Source.valueOf(nested.getString("from").toUpperCase(Locale.ROOT));
    }

    private static ScalarType scalarType(String jsonName) {
        return ScalarType.valueOf(jsonName.toUpperCase(Locale.ROOT));
    }

    private static ArrayField readArray(ScalarType type, JsonArray json) {
        var values = new long[json.size()];
        for (int i = 0; i < values.length; i++) {
            values[i] = readValue(type, json.get(i));
        }

        return new ArrayField(type, values);
    }

    private static long readValue(ScalarType type, JsonValue value) {
        return switch (type) {
            case INT8 -> integer(value, Byte.MIN_VALUE, Byte.MAX_VALUE);
            case UINT8 -> integer(value, 0, 0xff);
            case INT16 -> integer(value, Short.MIN_VALUE, Short.MAX_VALUE);
            case UINT16 -> integer(value, 0, 0xffff);
            case INT32 -> integer(value, Integer.MIN_VALUE, Integer.MAX_VALUE);
            case UINT32 -> integer(value, 0, 0xffff_ffffL);
            case INT64 -> Long.parseLong(((JsonString) value).getString());
            case UINT64 -> Long.parseUnsignedLong(((JsonString) value).getString());
            case FLOAT32 -> Float.floatToRawIntBits((float) float64(value));
            case FLOAT64 -> Double.doubleToRawLongBits(float64(value));
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
        // JsonNumber converts from a BigDecimal, which is exact
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

    static JsonObject write(TableDescription table) {
        var json = Json.createObjectBuilder().add("layout", write(table.layout()));
        table.version().ifPresent(version -> json.add("version", version));

        var fields = Json.createArrayBuilder();
        for (var field : table.fields()) {
            fields.add(write(field));
        }

        return json.add("fields", fields).build();
    }

    static JsonObject write(LayoutDescription layout) {
        return Json.createObjectBuilder()
            .add("sized", layout.sized())
            .add("versioned", layout.versioned())
            .add("offsetSize", layout.width())
            .build();
    }

    static JsonObject write(FieldDescription field) {
        return switch (field) {
            case ScalarField scalar -> Json.createObjectBuilder()
                .add("type", scalar.type().jsonName())
                .add("value", writeValue(scalar.type(), scalar.value()))
                .build();
            case ArrayField array -> Json.createObjectBuilder()
                .add("type", array.type().jsonName() + "[]")
                .add("value", writeValues(array.type(), array.values()))
                .build();
            case StringField string -> Json.createObjectBuilder()
                .add("type", "string")
                .add("value", string.value())
                .build();
            case TableField table -> writeTableField(table);
            case VectorField vector -> writeVector(vector);
        };
    }

    static JsonObject write(NestedTable item) {
        return Json.createObjectBuilder()
            .add("from", item.from().jsonName())
            .add("table", write(item.table()))
            .build();
    }

    private static JsonObject writeTableField(TableField table) {
        var json = Json.createObjectBuilder()
            .add("type", "table")
            .add("from", table.from().jsonName());
        table.alignment().ifPresent(alignment -> json.add("alignment", alignment));

        return json.add("table", write(table.table())).build();
    }

    private static JsonObject writeVector(VectorField vector) {
        var items = Json.createArrayBuilder();
        for (var item : vector.items()) {
            items.add(write(item));
        }

        return Json.createObjectBuilder().add("type", "vector").add("items", items).build();
    }

    private static JsonArray writeValues(ScalarType type, long[] values) {
        var json = Json.createArrayBuilder();
        for (var value : values) {
            json.add(writeValue(type, value));
        }

        return json.build();
    }

    private static JsonValue writeValue(ScalarType type, long value) {
        return switch (type) {
            case INT64 -> Json.createValue(Long.toString(value));
            case UINT64 -> Json.createValue(Long.toUnsignedString(value));
            case FLOAT32 -> Float.isNaN(Float.intBitsToFloat((int) value)) && (int) value != QUIET_NAN_32
                ? otherNaN(value & 0xffff_ffffL)
                : writeFloat(Float.intBitsToFloat((int) value));
            case FLOAT64 -> Double.isNaN(Double.longBitsToDouble(value)) && value != QUIET_NAN_64
                ? otherNaN(value)
                : writeFloat(Double.longBitsToDouble(value));
            default -> Json.createValue(value);
        };
    }

    /**
     * Names a NaN other than the quiet NaN, which {@code cases.json} has no name for, so that a
     * value read with other bits doesn't show as the NaN that was written
     */
    private static JsonValue otherNaN(long bits) {
        return Json.createValue(String.format("NaN 0x%x", bits));
    }

    /** Writes a float32 as the float64 that equals it, as {@code cases.json} does */
    private static JsonValue writeFloat(double value) {
        if (Double.isNaN(value)) {
            return Json.createValue("NaN");
        }

        if (Double.isInfinite(value)) {
            return Json.createValue(value > 0 ? "Infinity" : "-Infinity");
        }

        if (Double.doubleToRawLongBits(value) == Double.doubleToRawLongBits(-0.0)) {
            return Json.createValue("-0");
        }

        return Json.createValue(value);
    }
}
