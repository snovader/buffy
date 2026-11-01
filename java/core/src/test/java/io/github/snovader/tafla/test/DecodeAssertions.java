package io.github.snovader.tafla.test;

import io.github.snovader.tafla.*;

/** Assertions about reading bytes from untrusted sources */
final class DecodeAssertions {
    private DecodeAssertions() {}

    /**
     * Runs a read of bytes that may not decode, which may return values or throw
     * {@link DecodeException}, and nothing else: any other exception fails the test
     */
    static void assertValuesOrDecodeException(Runnable read) {
        try {
            read.run();
        } catch (DecodeException expected) {
            // The bytes don't hold what the read expects
        }
    }
}
