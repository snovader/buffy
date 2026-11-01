package io.github.snovader.tafla;

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
     * {@return the byte length of {@code string} in UTF-8, as {@link #encode} writes it}: a lone
     * surrogate takes the 3 bytes of U+FFFD, and a surrogate pair 4. It's a {@code long}, since a
     * string of {@code char}s of 3 bytes each may be longer than an {@code int} counts.
     */
    static long length(String string) {
        long length = string.length();

        for (int i = 0; i < string.length(); i++) {
            var unit = string.charAt(i);
            if (unit < 0x80) {
                continue;
            }

            if (unit < 0x800) {
                length += 1;
            } else if (Character.isHighSurrogate(unit) && isLowSurrogateAt(string, i + 1)) {
                // The pair's two units take 4 bytes
                length += 2;
                i++;
            } else {
                length += 2;
            }
        }

        return length;
    }

    /**
     * Writes {@code string} as UTF-8 into {@code bytes} from {@code offset}, with lone surrogates
     * replaced by U+FFFD, as {@link #encode} writes it, without allocating.
     *
     * @return the number of bytes written
     * @throws IndexOutOfBoundsException if the bytes end before the string does, which leaves the
     *     bytes before their end written
     */
    static int encodeInto(String string, byte[] bytes, int offset) {
        var position = offset;

        for (int i = 0; i < string.length(); i++) {
            var unit = string.charAt(i);

            if (unit < 0x80) {
                bytes[position++] = (byte) unit;
            } else if (unit < 0x800) {
                bytes[position++] = (byte) (0xc0 | (unit >> 6));
                bytes[position++] = continuation(unit);
            } else if (Character.isHighSurrogate(unit) && isLowSurrogateAt(string, i + 1)) {
                var codePoint = Character.toCodePoint(unit, string.charAt(++i));
                bytes[position++] = (byte) (0xf0 | (codePoint >> 18));
                bytes[position++] = continuation(codePoint >> 12);
                bytes[position++] = continuation(codePoint >> 6);
                bytes[position++] = continuation(codePoint);
            } else {
                var character = Character.isSurrogate(unit) ? REPLACEMENT_CHARACTER : unit;
                bytes[position++] = (byte) (0xe0 | (character >> 12));
                bytes[position++] = continuation(character >> 6);
                bytes[position++] = continuation(character);
            }
        }

        return position - offset;
    }

    private static boolean isLowSurrogateAt(String string, int index) {
        return index < string.length() && Character.isLowSurrogate(string.charAt(index));
    }

    /** {@return the continuation byte of a sequence that holds the low 6 of {@code bits}} */
    private static byte continuation(int bits) {
        return (byte) (0x80 | (bits & 0x3f));
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
