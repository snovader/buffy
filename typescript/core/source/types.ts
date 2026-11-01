import type { SIZEOF_8, SIZEOF_16, SIZEOF_32, SIZEOF_64 } from './constants.ts';

export type AddressableSpace =
    | typeof SIZEOF_8
    | typeof SIZEOF_16
    | typeof SIZEOF_32;

export type ElementSize =
    | typeof SIZEOF_8
    | typeof SIZEOF_16
    | typeof SIZEOF_32
    | typeof SIZEOF_64;

export type TypedArray =
    | BigInt64Array
    | BigUint64Array
    | Float32Array
    | Float64Array
    | Int8Array
    | Int16Array
    | Int32Array
    | Uint8Array
    | Uint16Array
    | Uint32Array;

export interface TypedArrayConstructor {
    BYTES_PER_ELEMENT: number;

    // biome-ignore lint/suspicious/noExplicitAny: Necessary for inference
    new (length: number): any;
    // biome-ignore lint/suspicious/noExplicitAny: Necessary for inference
    new (buffer: ArrayBufferLike, byteOffset: number, length: number): any;
}
