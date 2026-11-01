package io.github.snovader.buffy.test;

import java.util.*;

/**
 * The description of a table in the shape of the conformance suite's {@code cases.json}, see
 * {@code conformance/README.md} at the root of the repository. The conformance test reads it from
 * the suite, and {@link Descriptions} generates it.
 *
 * @param version the version in the header, present exactly when the layout is versioned
 * @param fields the fields in the order that they're set, which is the order of their offsets
 */
record TableDescription(LayoutDescription layout, OptionalLong version, List<FieldDescription> fields) {
    TableDescription {
        fields = List.copyOf(fields);
    }

    /** {@return this table with {@code field} set after its fields} */
    TableDescription withField(FieldDescription field) {
        var withField = new ArrayList<>(fields);
        withField.add(field);

        return new TableDescription(layout, version, withField);
    }

    @Override
    public String toString() {
        return DescriptionJson.write(this).toString();
    }
}
