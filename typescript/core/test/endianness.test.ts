import { describe, expect } from 'vitest';
import { fc, it } from '@fast-check/vitest';
import { IS_LITTLE_ENDIAN } from '#constants';
import { toUint8Array } from '#converters';
import { byteOrderSwap, byteOrderSwapped } from '#endianness';
import type { TypedArray, TypedArrayConstructor } from '#types';

const typedArrays = fc.oneof(
    fc.tuple(fc.constant('getInt8'), fc.int8Array()),
    fc.tuple(fc.constant('getUint8'), fc.uint8Array()),
    fc.tuple(fc.constant('getInt16'), fc.int16Array()),
    fc.tuple(fc.constant('getUint16'), fc.uint16Array()),
    fc.tuple(fc.constant('getInt32'), fc.int32Array()),
    fc.tuple(fc.constant('getUint32'), fc.uint32Array()),
    fc.tuple(fc.constant('getBigInt64'), fc.bigInt64Array()),
    fc.tuple(fc.constant('getBigUint64'), fc.bigUint64Array()),
    fc.tuple(fc.constant('getFloat32'), fc.float32Array()),
    fc.tuple(fc.constant('getFloat64'), fc.float64Array())
);

type Getter = Extract<keyof DataView, `get${string}`>;

describe('byteOrderSwap', () => {
    it.prop([typedArrays])('should byte swap the given typed array', ([getter, array]) => {
        const destination = new Uint8Array(array.byteLength);
        byteOrderSwap(toUint8Array(array), destination, array.BYTES_PER_ELEMENT);

        expectSwapped(getter, destination, array);
    });

    it('should reject an unsupported element size', () => {
        expect(() => byteOrderSwap(new Uint8Array(3), new Uint8Array(3), 3)).toThrow(
            'Unsupported "BYTES_PER_ELEMENT" 3'
        );
    });
});

describe('byteOrderSwapped', () => {
    it.prop([typedArrays, fc.integer({ min: 0, max: 15 })])(
        'should copy the elements at any byte offset with their byte order swapped',
        ([getter, array], byteOffset) => {
            const buffer = new ArrayBuffer(byteOffset + array.byteLength + 8);
            new Uint8Array(buffer).set(toUint8Array(array), byteOffset);

            const TypedArray = array.constructor as TypedArrayConstructor;
            const swapped = byteOrderSwapped(TypedArray, buffer, byteOffset, array.length);

            expect(swapped).toBeInstanceOf(TypedArray);
            expect(swapped.length).toBe(array.length);
            expect(swapped.buffer).not.toBe(buffer);
            expectSwapped(getter, toUint8Array(swapped), array);
        }
    );

    it.each([
        ['Int8Array', Int8Array, [1, 2, 3, 4, 5, 6, 7, 8]],
        ['Uint16Array', Uint16Array, [2, 1, 4, 3, 6, 5, 8, 7]],
        ['Float32Array', Float32Array, [4, 3, 2, 1, 8, 7, 6, 5]],
        ['BigInt64Array', BigInt64Array, [8, 7, 6, 5, 4, 3, 2, 1]]
    ] as const)(
        'should reverse the bytes of each %s element at an unaligned offset',
        (_, TypedArray, expected) => {
            // Bytes 1 to 8 after a byte that isn't part of the elements
            const buffer = new Uint8Array([0, 1, 2, 3, 4, 5, 6, 7, 8]).buffer;
            const length = 8 / TypedArray.BYTES_PER_ELEMENT;

            const swapped = byteOrderSwapped(TypedArray, buffer, 1, length);

            expect([...toUint8Array(swapped)]).toStrictEqual(expected);
        }
    );

    it.each([
        ['Float32', Float32Array],
        ['Float64', Float64Array]
    ] as const)(
        'should keep -0 and NaN when a big-endian host reads little-endian %s elements',
        (type, TypedArray) => {
            const values = [-0, Number.NaN, 0, Number.NEGATIVE_INFINITY, -1.5];
            const elementSize = TypedArray.BYTES_PER_ELEMENT;
            const byteOffset = 3;

            const littleEndian = new DataView(
                new ArrayBuffer(byteOffset + values.length * elementSize),
                byteOffset
            );
            for (const [i, value] of values.entries()) {
                littleEndian[`set${type}`](i * elementSize, value, true);
            }

            const swapped = byteOrderSwapped(
                TypedArray,
                littleEndian.buffer,
                byteOffset,
                values.length
            );

            // A big-endian host reads the elements of its typed arrays as big-endian
            const bigEndian = new DataView(swapped.buffer);
            for (const [i, value] of values.entries()) {
                expect(bigEndian[`get${type}`](i * elementSize, false)).toBe(value);
            }
        }
    );
});

/**
 * Expects `bytes`, read in the opposite byte order to the host's, to hold the
 * elements of `array`. `toBe` distinguishes -0 from 0.
 */
function expectSwapped(getter: Getter, bytes: Uint8Array, array: TypedArray): void {
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const get = view[getter].bind(view);

    for (let i = 0, j = 0; i < array.length; i++, j += array.BYTES_PER_ELEMENT) {
        expect(get(j, !IS_LITTLE_ENDIAN)).toBe(array[i]);
    }
}
