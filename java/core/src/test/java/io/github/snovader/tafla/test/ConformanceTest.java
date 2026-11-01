package io.github.snovader.tafla.test;

import io.github.snovader.tafla.*;
import jakarta.json.*;
import net.jqwik.api.*;

import java.io.*;
import java.nio.charset.*;
import java.nio.file.*;
import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Reads and rebuilds the tables of tafla's conformance suite, which the TypeScript runtime writes.
 * See {@code conformance/README.md} at the root of the repository for the format of their
 * descriptions in {@code cases.json}, which {@link DescriptionJson} reads.
 */
public class ConformanceTest {
    private static final Path CONFORMANCE = Path.of(System.getProperty("tafla.conformance"));
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

    /** {@return the description of the valid case's table} */
    private static TableDescription tableOf(Case validCase) {
        return DescriptionJson.readTable(validCase.description().getJsonObject("table"));
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
        var table = tableOf(validCase);

        assertEquals(table, TableReader.strict(validCase.bytes(), table.layout()).readTable(table));
    }

    @Property(generation = GenerationMode.EXHAUSTIVE, shrinking = ShrinkingMode.OFF)
    void should_rebuild_every_valid_table_byte_for_byte(@ForAll("validCases") Case validCase) {
        assertArrayEquals(validCase.bytes(), Tables.build(tableOf(validCase)).bytes());
    }

    @Property(generation = GenerationMode.EXHAUSTIVE, shrinking = ShrinkingMode.OFF)
    void should_read_invalid_tables_up_to_the_field_that_throws(@ForAll("invalidCases") Case invalidCase) {
        var description = invalidCase.description();
        var validCase = cases("valid").stream()
            .filter(candidate -> candidate.name().equals(description.getString("case")))
            .findFirst()
            .orElseThrow();

        var table = tableOf(validCase);
        var fields = table.fields();
        var field = description.getInt("field");
        var reader = TableReader.strict(invalidCase.bytes(), table.layout());

        for (int index = 0; index < field; index++) {
            assertEquals(fields.get(index), reader.readRootField(index, fields.get(index)), "field " + index);
        }

        assertThrows(DecodeException.class, () -> reader.readRootField(field, fields.get(field)));
    }
}
