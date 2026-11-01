package io.github.snovader.buffy.benchmarks;

import org.jspecify.annotations.*;

import java.util.*;

/**
 * A document: a sized, versioned table of two float64s and a string, with a nested table of an
 * int64 and three columns of the same length. The keys are uint32s, held in {@code long}s as buffy
 * reads them.
 *
 * <p>Documents are equal when their columns hold the same values, with floats compared by their
 * bits as {@link Arrays#equals(float[], float[])} compares them.</p>
 */
// The columns are arrays because the codecs read and write them as arrays, and equals() and
// hashCode() compare their elements
@SuppressWarnings("ArrayRecordComponent")
public record DocumentInput(
    double x,
    double y,
    String name,
    long base,
    long[] keys,
    float[] a,
    float[] b
) {
    @Override
    public boolean equals(@Nullable Object other) {
        return other instanceof DocumentInput document
            && Double.compare(x, document.x) == 0
            && Double.compare(y, document.y) == 0
            && name.equals(document.name)
            && base == document.base
            && Arrays.equals(keys, document.keys)
            && Arrays.equals(a, document.a)
            && Arrays.equals(b, document.b);
    }

    @Override
    public int hashCode() {
        return Objects.hash(x, y, name, base, Arrays.hashCode(keys), Arrays.hashCode(a), Arrays.hashCode(b));
    }

    /** {@return the document's fields, with the length of its columns rather than their values} */
    @Override
    public String toString() {
        return String.format(
            "DocumentInput[x=%s, y=%s, name=%s, base=%d, rows=%d]",
            x,
            y,
            name,
            base,
            a.length
        );
    }
}
