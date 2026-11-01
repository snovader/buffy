import type { TypedArray } from './types.ts';

export function toUint8Array(values: TypedArray): Uint8Array {
    if (values instanceof Uint8Array) {
        return values;
    }

    return new Uint8Array(values.buffer, values.byteOffset, values.byteLength);
}
