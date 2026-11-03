import { expect, vi } from 'vitest';
import { fc, it } from '@fast-check/vitest';
import { ByteBuffer } from 'tafla';
import { growing } from './arrayLikes.ts';
import { ARRAY_FIELDS, fitsItsLayout, SLICES, sliceOf, tables } from './conformance/arbitraries.ts';
import { elementTypeOf, type TableDescription } from './conformance/description.ts';
import { readScalar, readUnsigned } from './conformance/format.ts';
import {
    buildTable,
    ELEMENT_SIZES,
    nestedInPlace,
    readTable,
    writeTable
} from './conformance/tables.ts';

// Runs tafla as if on a big-endian host. This host is little-endian, so the
// arrays it writes must come out in reverse byte order per element, which is
// little-endian on a big-endian host. The factory is synchronous, since an
// async one leaves the constants undefined while TableLayout.DEFAULT is built.
vi.mock('../source/constants.ts', () => ({
    SIZEOF_8: 1,
    SIZEOF_16: 2,
    SIZEOF_32: 4,
    SIZEOF_64: 8,
    IS_LITTLE_ENDIAN: false
}));

it.prop([fc.array(fc.double()), fc.boolean()])(
    'should byte-swap each converted element on a big-endian host',
    (values, typed) => {
        const bytes = new Uint8Array(4 + values.length * 4);
        new ByteBuffer(bytes).setFloat32Array(typed ? Float32Array.from(values) : values);

        const native = new Uint8Array(Float32Array.from(values).buffer);
        const swapped = native.map((_, i) => native[i - (i % 4) + 3 - (i % 4)]);

        expect(new DataView(bytes.buffer).getUint32(0, true)).toBe(values.length);
        expect(bytes.subarray(4)).toStrictEqual(swapped);
    }
);

it.prop([fc.array(fc.double(), { minLength: 1, maxLength: 8 }), fc.nat(), SLICES])(
    'should throw a RangeError for an array that grows after it is counted on a big-endian host, writing nothing',
    (values, cut, slice) => {
        const counted = cut % values.length;
        const bytes = sliceOf(new Uint8Array(4 + values.length * 8), slice);
        const before = new Uint8Array(bytes.buffer.slice(0));

        expect(() => new ByteBuffer(bytes).setFloat64Array(growing(values, counted))).toThrow(
            RangeError
        );
        expect(new Uint8Array(bytes.buffer)).toStrictEqual(before);
    }
);

it.prop([ARRAY_FIELDS])(
    'should byte-swap the elements of every array type on a big-endian host',
    field => {
        const table: TableDescription = {
            layout: { sized: false, versioned: false, offsetSize: 4 },
            fields: [field]
        };
        const bytes = buildTable(table).bytes();

        // Its lengths are written with DataView, little-endian on any host
        const position = readUnsigned(bytes, 0, 4);
        const type = elementTypeOf(field.type);
        const values = field.value.map((_, index) =>
            readScalar(bytes, type, position + 4 + index * ELEMENT_SIZES[type], false)
        );

        expect(values).toStrictEqual(field.value);
    }
);

it.prop([tables(), SLICES])(
    'should read back the values of a table built from any description on a big-endian host',
    (table, slice) => {
        const bytes = sliceOf(buildTable(table).bytes(), slice);

        expect(readTable(bytes, table)).toStrictEqual(table);
    }
);

it.prop([tables().map(nestedInPlace).filter(fitsItsLayout)])(
    'should write the bytes that TableBuilder builds on a big-endian host with TableWriter',
    table => {
        expect(writeTable(table)).toStrictEqual(buildTable(table).bytes());
    }
);
