/**
 * Thrown when bytes can't be read as the table, array, string or value that a
 * read expects. Every read checks its range against the supplied bytes before
 * it returns, so a truncated or corrupted buffer throws this rather than
 * returning values from outside them. Strings must also be valid UTF-8.
 *
 * Checks can't tell a value that was corrupted within range from a real one,
 * such as an offset that points at another field. Formats that need that
 * guarantee can add a checksum.
 *
 * It extends `RangeError`, which tafla threw before it had its own class, so
 * existing handlers keep catching it.
 */
export class DecodeError extends RangeError {
    constructor(message: string, options?: ErrorOptions) {
        super(message, options);
        this.name = 'DecodeError';
    }
}

/** Builds the error for a read outside the supplied bytes */
export function outOfRange(
    position: number,
    byteLength: number,
    available: number
): DecodeError {
    return new DecodeError(
        `Read exceeds the supplied buffer: ${byteLength} bytes at ${position} of ${available}`
    );
}
