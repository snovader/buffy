package io.github.snovader.buffy.benchmarks.test;

import io.github.snovader.buffy.benchmarks.*;
import net.jqwik.api.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Checks that the generator makes the TypeScript comparison's values, which these were printed
 * from, so that the languages' benchmarks time the same work
 */
public class WorkloadsTest {
    @Example
    void should_generate_the_typescript_document_of_100_rows() {
        var document = Workloads.documentInput(100);

        assertEquals(14_727_226, document.keys()[0]);
        assertEquals(10_335_322, document.keys()[99]);
        assertEquals(825_833_448L, sum(document.keys()));
        assertEquals(0xc2219b29, Float.floatToRawIntBits(document.a()[0]));
        assertEquals(0xc163647e, Float.floatToRawIntBits(document.b()[99]));
        assertEquals(1_493_961_445, hashColumns(document));
    }

    @Example
    void should_generate_the_typescript_document_of_100000_rows() {
        var document = Workloads.documentInput(100_000);

        assertEquals(13_320_945, document.keys()[0]);
        assertEquals(11_621_954, document.keys()[99_999]);
        assertEquals(839_050_642_924L, sum(document.keys()));
        assertEquals(0xc1d4d7f6, Float.floatToRawIntBits(document.a()[0]));
        assertEquals(0x3e8c91f5, Float.floatToRawIntBits(document.b()[99_999]));
        assertEquals(1_138_446_770, hashColumns(document));
    }

    @Example
    void should_generate_the_typescript_vector_of_nested_tables() {
        var items = Workloads.listInput(Workloads.LIST_ITEMS).items();

        assertEquals(100, items.size());
        assertEquals(item(0x41d2acb6, 0xbfcedb15, 0x41e32e23), items.get(0));
        assertEquals(item(0x4193d915, 0x3f979e05, 0x418f26e1), items.get(99));
    }

    private static ItemInput item(int value, int first, int second) {
        return new ItemInput(
            Float.intBitsToFloat(value),
            Float.intBitsToFloat(first),
            Float.intBitsToFloat(second)
        );
    }

    private static long sum(long[] values) {
        long sum = 0;
        for (var value : values) {
            sum += value;
        }

        return sum;
    }

    /** Hashes the float columns' bits, row by row, as the TypeScript script that printed these did */
    private static int hashColumns(DocumentInput document) {
        int hash = 0;
        for (int i = 0; i < document.a().length; i++) {
            hash = hash * 31 + Float.floatToRawIntBits(document.a()[i]);
            hash = hash * 31 + Float.floatToRawIntBits(document.b()[i]);
        }

        return hash;
    }
}
