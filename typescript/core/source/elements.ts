import {
    IS_LITTLE_ENDIAN,
    SIZEOF_16,
    SIZEOF_32,
    SIZEOF_64
} from './constants.ts';
import { toUint8Array } from './converters.ts';
import { byteOrderSwap } from './endianness.ts';
import type { TypedArray } from './types.ts';

// Writes the elements of an array, little-endian, at `position` in `bytes`,
// which must hold them: ByteBuffer and TableWriter write every array with
// these, after its count, which they write once they've checked that the
// elements fit. The view that an aligned array is written through starts at
// `position` in the bytes' buffer, and reaches past `bytes` when they're only
// part of it and too short. A typed array of the element type is copied as
// bytes, and any other array-like converted as the typed array's constructor
// would.
//
// Each is written out for one element type, so that each conversion and
// typed array construction sees one type and stays monomorphic, whatever
// other types the program writes. Each writes through a typed array view
// with `set()`, which converts any array-like in a built-in. A JS loop
// instead would share its type feedback between callers: once a loop had
// seen typed arrays and JS arrays, writing 27,028 floats from a `number[]`
// took 130–180 µs, against 14–20 µs for the built-in.

export function setInt8Elements(
    bytes: Uint8Array,
    position: number,
    values: ArrayLike<number> | Int8Array
): void {
    const byteOffset = bytes.byteOffset + position;
    new Int8Array(bytes.buffer, byteOffset, values.length).set(values);
}

export function setUint8Elements(
    bytes: Uint8Array,
    position: number,
    values: ArrayLike<number> | Uint8Array
): void {
    const byteOffset = bytes.byteOffset + position;
    new Uint8Array(bytes.buffer, byteOffset, values.length).set(values);
}

export function setInt16Elements(
    bytes: Uint8Array,
    position: number,
    values: ArrayLike<number> | Int16Array
): void {
    const byteOffset = bytes.byteOffset + position;

    if (IS_LITTLE_ENDIAN && byteOffset % SIZEOF_16 === 0) {
        new Int16Array(bytes.buffer, byteOffset, values.length).set(values);
    } else {
        copyElements(
            bytes,
            position,
            values instanceof Int16Array ? values : new Int16Array(values)
        );
    }
}

export function setUint16Elements(
    bytes: Uint8Array,
    position: number,
    values: ArrayLike<number> | Uint16Array
): void {
    const byteOffset = bytes.byteOffset + position;

    if (IS_LITTLE_ENDIAN && byteOffset % SIZEOF_16 === 0) {
        new Uint16Array(bytes.buffer, byteOffset, values.length).set(values);
    } else {
        copyElements(
            bytes,
            position,
            values instanceof Uint16Array ? values : new Uint16Array(values)
        );
    }
}

export function setInt32Elements(
    bytes: Uint8Array,
    position: number,
    values: ArrayLike<number> | Int32Array
): void {
    const byteOffset = bytes.byteOffset + position;

    if (IS_LITTLE_ENDIAN && byteOffset % SIZEOF_32 === 0) {
        new Int32Array(bytes.buffer, byteOffset, values.length).set(values);
    } else {
        copyElements(
            bytes,
            position,
            values instanceof Int32Array ? values : new Int32Array(values)
        );
    }
}

export function setUint32Elements(
    bytes: Uint8Array,
    position: number,
    values: ArrayLike<number> | Uint32Array
): void {
    const byteOffset = bytes.byteOffset + position;

    if (IS_LITTLE_ENDIAN && byteOffset % SIZEOF_32 === 0) {
        new Uint32Array(bytes.buffer, byteOffset, values.length).set(values);
    } else {
        copyElements(
            bytes,
            position,
            values instanceof Uint32Array ? values : new Uint32Array(values)
        );
    }
}

export function setInt64Elements(
    bytes: Uint8Array,
    position: number,
    values: ArrayLike<bigint> | BigInt64Array
): void {
    const byteOffset = bytes.byteOffset + position;

    if (IS_LITTLE_ENDIAN && byteOffset % SIZEOF_64 === 0) {
        new BigInt64Array(bytes.buffer, byteOffset, values.length).set(values);
    } else {
        copyElements(
            bytes,
            position,
            values instanceof BigInt64Array ? values : new BigInt64Array(values)
        );
    }
}

export function setUint64Elements(
    bytes: Uint8Array,
    position: number,
    values: ArrayLike<bigint> | BigUint64Array
): void {
    const byteOffset = bytes.byteOffset + position;

    if (IS_LITTLE_ENDIAN && byteOffset % SIZEOF_64 === 0) {
        new BigUint64Array(bytes.buffer, byteOffset, values.length).set(values);
    } else {
        copyElements(
            bytes,
            position,
            values instanceof BigUint64Array
                ? values
                : new BigUint64Array(values)
        );
    }
}

export function setFloat32Elements(
    bytes: Uint8Array,
    position: number,
    values: ArrayLike<number> | Float32Array
): void {
    const byteOffset = bytes.byteOffset + position;

    if (IS_LITTLE_ENDIAN && byteOffset % SIZEOF_32 === 0) {
        new Float32Array(bytes.buffer, byteOffset, values.length).set(values);
    } else {
        copyElements(
            bytes,
            position,
            values instanceof Float32Array ? values : new Float32Array(values)
        );
    }
}

export function setFloat64Elements(
    bytes: Uint8Array,
    position: number,
    values: ArrayLike<number> | Float64Array
): void {
    const byteOffset = bytes.byteOffset + position;

    if (IS_LITTLE_ENDIAN && byteOffset % SIZEOF_64 === 0) {
        new Float64Array(bytes.buffer, byteOffset, values.length).set(values);
    } else {
        copyElements(
            bytes,
            position,
            values instanceof Float64Array ? values : new Float64Array(values)
        );
    }
}

/**
 * Copies the elements' bytes in little-endian order, for a destination that a
 * view can't start at or a big-endian host
 */
function copyElements(
    bytes: Uint8Array,
    position: number,
    values: TypedArray
): void {
    const destination = bytes.subarray(position, position + values.byteLength);

    if (IS_LITTLE_ENDIAN) {
        destination.set(toUint8Array(values));
    } else {
        byteOrderSwap(
            toUint8Array(values),
            destination,
            values.BYTES_PER_ELEMENT
        );
    }
}
