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
