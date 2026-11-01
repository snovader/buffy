package io.github.snovader.tafla.test;

import io.github.snovader.tafla.*;
import io.github.snovader.tafla.test.FieldDescription.*;

import java.util.*;

/** Builds tables as their descriptions set them out, as every implementation's conformance test does */
final class Tables {
    private Tables() {}

    /** {@return a builder of the table with all its fields set} */
    static TableBuilder build(TableDescription table) {
        var builder = createBuilder(table);
        for (var field : table.fields()) {
            setField(builder, field);
        }

        return builder;
    }

    /** {@return a builder of the table's layout and version, with none of its fields set yet} */
    static TableBuilder createBuilder(TableDescription table) {
        var fields = table.fields().size();
        var layout = table.layout().tableLayout();
        var version = table.version();

        return version.isPresent()
            ? new TableBuilder(fields, layout, version.getAsLong())
            : new TableBuilder(fields, layout);
    }

    /** Sets the field on the builder with the setter of its type, building a nested table first */
    static void setField(TableBuilder builder, FieldDescription field) {
        switch (field) {
            case ScalarField scalar -> scalar.type().set(builder, scalar.value());
            case ArrayField array -> array.type().setArray(builder, array.values());
            case StringField string -> builder.setString(string.value());
            case TableField table -> setTable(builder, table);
            case VectorField vector -> setVector(builder, vector.items());
        }
    }

    private static void setTable(TableBuilder builder, TableField field) {
        var nested = build(field.table());
        var alignment = field.alignment();

        switch (field.from()) {
            case BUILDER -> {
                if (alignment.isPresent()) {
                    builder.setTable(nested, alignment.getAsInt());
                } else {
                    builder.setTable(nested);
                }
            }
            case BYTES -> {
                if (alignment.isPresent()) {
                    builder.setTable(nested.bytes(), alignment.getAsInt());
                } else {
                    builder.setTable(nested.bytes());
                }
            }
        }
    }

    /**
     * Sets a vector from its items' builders when they're all builders. {@link TableBuilder} takes a
     * vector of builders or one of bytes, so a vector with bytes is set from every item's bytes, which
     * a builder writes as its parent would.
     */
    private static void setVector(TableBuilder builder, List<NestedTable> items) {
        var builders = items.stream().map(item -> build(item.table())).toList();

        if (items.stream().allMatch(item -> item.from() == Source.BUILDER)) {
            builder.setVector(builders);
        } else {
            builder.setVector(builders.stream().map(TableBuilder::bytes).toArray(byte[][]::new));
        }
    }
}
