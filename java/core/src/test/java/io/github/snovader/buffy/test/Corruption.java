package io.github.snovader.buffy.test;

import net.jqwik.api.*;

import java.util.*;

/**
 * A corruption of bytes of any length, which fits itself to the bytes that it's applied to: a
 * truncation that cuts at least one byte from the end, or bytes written over them, as many as fit.
 * jqwik draws small numbers more often, so most truncations cut a few bytes, which leaves a field
 * that ends the table short.
 */
sealed interface Corruption {
    /** {@return a corrupted copy of the bytes} */
    byte[] applyTo(byte[] bytes);

    static Arbitrary<Corruption> corruptions() {
        var counts = Arbitraries.integers().greaterOrEqual(0);
        var overwrites = Arbitraries.bytes().array(byte[].class).ofMinSize(1).ofMaxSize(4);

        return Arbitraries.oneOf(
            counts.map(Truncation::new),
            Combinators.combine(counts, overwrites).as(Overwrite::new)
        );
    }

    /** Cuts 1 + {@code cut} bytes from the end, modulo their length */
    record Truncation(int cut) implements Corruption {
        @Override
        public byte[] applyTo(byte[] bytes) {
            var length = bytes.length == 0 ? 0 : bytes.length - 1 - cut % bytes.length;
            return Arrays.copyOf(bytes, length);
        }
    }

    /** Writes {@code overwrite} at {@code at}, modulo the bytes' length, as far as the bytes reach */
    @SuppressWarnings("ArrayRecordComponent") // Applied, not compared
    record Overwrite(int at, byte[] overwrite) implements Corruption {
        @Override
        public byte[] applyTo(byte[] bytes) {
            var corrupted = bytes.clone();
            if (bytes.length > 0) {
                var position = at % bytes.length;
                var length = Math.min(overwrite.length, bytes.length - position);
                System.arraycopy(overwrite, 0, corrupted, position, length);
            }

            return corrupted;
        }

        @Override
        public String toString() {
            return String.format("Overwrite[at=%d, overwrite=%s]", at, Arrays.toString(overwrite));
        }
    }
}
