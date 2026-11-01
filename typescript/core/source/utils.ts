import { outOfRange } from './errors.ts';

// Little-endian reads straight from a Uint8Array, for small records at fixed
// offsets, such as stream frames, that aren't worth a ByteBuffer. Each throws
// a DecodeError if the value doesn't lie within `bytes`.

const int32s = new Int32Array(1);
const float32s = new Float32Array(int32s.buffer);

export function readUint8(bytes: Uint8Array, offset: number): number {
    assertReadable(bytes, offset, 1);
    return bytes[offset];
}

export function readInt32(bytes: Uint8Array, offset: number): number {
    assertReadable(bytes, offset, 4);
    return (
        bytes[offset] |
        (bytes[offset + 1] << 8) |
        (bytes[offset + 2] << 16) |
        (bytes[offset + 3] << 24)
    );
}

export function readUint32(bytes: Uint8Array, offset: number): number {
    return readInt32(bytes, offset) >>> 0;
}

export function readInt64(bytes: Uint8Array, offset: number): bigint {
    assertReadable(bytes, offset, 8);
    return BigInt.asIntN(
        64,
        BigInt(readUint32(bytes, offset)) +
            (BigInt(readUint32(bytes, offset + 4)) << 32n)
    );
}

export function readFloat32(bytes: Uint8Array, offset: number): number {
    int32s[0] = readInt32(bytes, offset);
    return float32s[0];
}

function assertReadable(
    bytes: Uint8Array,
    offset: number,
    byteLength: number
): void {
    if (
        !Number.isSafeInteger(offset) ||
        offset < 0 ||
        offset > bytes.length - byteLength
    ) {
        throw outOfRange(offset, byteLength, bytes.length);
    }
}
