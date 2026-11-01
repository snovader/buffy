import { expect } from 'vitest';
import { fc, it } from '@fast-check/vitest';
import { toUint8Array } from '#converters';

const typedArrays = fc.oneof(
    fc.int8Array(),
    fc.uint8Array(),
    fc.int16Array(),
    fc.uint16Array(),
    fc.int32Array(),
    fc.uint32Array(),
    fc.bigInt64Array(),
    fc.bigUint64Array(),
    fc.float32Array(),
    fc.float64Array()
);

it.prop([typedArrays])('should convert the given array to a Uint8Array', array => {
    const converted = toUint8Array(array);

    expect(converted).toStrictEqual(
        new Uint8Array(array.buffer, array.byteOffset, array.byteLength)
    );
});
