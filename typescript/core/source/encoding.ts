const encoder = new TextEncoder();
// Fatal, so that invalid UTF-8 throws instead of becoming U+FFFD, and keeping
// a leading U+FEFF, which is part of the string rather than a byte order mark
const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });

export function encode(string: string): Uint8Array {
    return encoder.encode(string);
}

/** @throws {TypeError} If `bytes` isn't valid UTF-8. */
export function decode(bytes: Uint8Array): string {
    return decoder.decode(bytes);
}

/**
 * Writes `string` as UTF-8 at the start of `bytes`, with lone surrogates
 * replaced by U+FFFD, as `encode()` writes them.
 *
 * @returns The number of bytes written.
 * @throws {RangeError} If `bytes` are too short for the string.
 */
export function encodeInto(string: string, bytes: Uint8Array): number {
    const { read, written } = encoder.encodeInto(string, bytes);
    if (read !== string.length) {
        throw new RangeError(
            `A string of ${string.length} UTF-16 code units doesn't fit in ${bytes.length} bytes`
        );
    }

    return written;
}

/**
 * The byte length of `string` in UTF-8, as `encode()` writes it: a lone
 * surrogate takes the 3 bytes of U+FFFD, and a surrogate pair 4 bytes
 */
export function utf8Length(string: string): number {
    let length = string.length;

    for (let i = 0; i < string.length; i++) {
        const unit = string.charCodeAt(i);
        if (unit < 0x80) {
            continue;
        }

        if (unit < 0x800) {
            length += 1;
        } else if (
            unit < 0xdc00 &&
            unit >= 0xd800 &&
            (string.charCodeAt(i + 1) & 0xfc00) === 0xdc00
        ) {
            // The pair's two units take 4 bytes
            length += 2;
            i++;
        } else {
            length += 2;
        }
    }

    return length;
}
