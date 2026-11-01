package io.github.snovader.buffy;

import java.nio.charset.*;

/** Encodes and decodes strings as the TypeScript and Python implementations do */
final class Utf8 {
    private static final char REPLACEMENT_CHARACTER = '\uFFFD';

    private Utf8() {}

    /**
     * Encodes {@code string}, with lone surrogates replaced by U+FFFD, as TypeScript's
     * {@code TextEncoder} replaces them. {@link String#getBytes} would write each as {@code '?'}.
     */
    static byte[] encode(String string) {
        for (int i = 0; i < string.length(); i++) {
            if (Character.isSurrogate(string.charAt(i))) {
                return replaceLoneSurrogates(string).getBytes(StandardCharsets.UTF_8);
            }
        }

        return string.getBytes(StandardCharsets.UTF_8);
    }

    /**
     * Decodes {@code length} bytes from {@code offset}, keeping a leading U+FEFF, which is part of
     * the string rather than a byte order mark.
     *
     * @throws CharacterCodingException if the bytes aren't valid UTF-8
     */
    static String decode(byte[] bytes, int offset, int length) throws CharacterCodingException {
        // The String constructor replaces each invalid sequence with U+FFFD, and decodes faster than
        // a CharsetDecoder, so a string without U+FFFD was valid. A string with it is decoded again
        // strictly, to tell a U+FFFD in the bytes from a replaced sequence.
        var string = new String(bytes, offset, length, StandardCharsets.UTF_8);
        if (string.indexOf(REPLACEMENT_CHARACTER) < 0) {
            return string;
        }

        var decoder = StandardCharsets.UTF_8.newDecoder();
        return decoder.decode(java.nio.ByteBuffer.wrap(bytes, offset, length)).toString();
    }

    /** A surrogate without its pair is a code point of its own, in the surrogates' range */
    private static String replaceLoneSurrogates(String string) {
        var wellFormed = new StringBuilder(string.length());
        string.codePoints().forEach(codePoint -> wellFormed.appendCodePoint(
            isSurrogate(codePoint) ? REPLACEMENT_CHARACTER : codePoint
        ));

        return wellFormed.toString();
    }

    private static boolean isSurrogate(int codePoint) {
        return codePoint >= Character.MIN_SURROGATE && codePoint <= Character.MAX_SURROGATE;
    }
}
