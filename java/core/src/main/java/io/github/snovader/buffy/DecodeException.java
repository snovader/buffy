package io.github.snovader.buffy;

/**
 * Thrown when bytes can't be read as the table, array, string or value that a read expects. Every
 * read checks its range against the supplied bytes before it returns, so a truncated or corrupted
 * buffer throws this rather than returning values from outside them. Strings must also be valid
 * UTF-8.
 *
 * <p>Checks can't tell a value that was corrupted within range from a real one, such as an offset
 * that points at another field. Formats that need that guarantee can add a checksum.</p>
 *
 * <p>It's unchecked, since a table is read field by field, from accessors that would otherwise
 * all have to declare it.</p>
 */
public class DecodeException extends RuntimeException {
    /**
     * Creates the exception of a read that failed.
     *
     * @param message what was read, where, and why it failed
     */
    public DecodeException(String message) {
        super(message);
    }

    /**
     * Creates the exception of a read that failed with another exception, such as the
     * {@link java.nio.charset.CharacterCodingException} of a string that isn't valid UTF-8.
     *
     * @param message what was read, where, and why it failed
     * @param cause the exception that the read failed with
     */
    public DecodeException(String message, Throwable cause) {
        super(message, cause);
    }

    /** Builds the error for a read outside the supplied bytes */
    static DecodeException outOfRange(long position, long byteLength, int available) {
        return new DecodeException(
            String.format(
                "Read exceeds the supplied buffer: %d bytes at %d of %d",
                byteLength,
                position,
                available
            )
        );
    }
}
