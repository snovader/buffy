import { expect } from 'vitest';
import { fc, it } from '@fast-check/vitest';
import {
    DecodeError,
    readFloat32,
    readInt32,
    readInt64,
    readUint8,
    readUint32,
    StructWriter
} from '@buffy/core';
import { SLICES, sliceOf } from './conformance/arbitraries.ts';

type Setter = Extract<keyof StructWriter, `set${string}`>;

/** A struct's field: the setter that writes it and its value */
interface Field {
    setter: Setter;
    value: number | bigint;
}

interface Scalar {
    size: number;
    values: fc.Arbitrary<number | bigint>;
    /** Writes the value little-endian, as a reference */
    write: (view: DataView, position: number, value: never) => void;
}

const SCALARS: Readonly<Record<Setter, Scalar>> = {
    setInt8: {
        size: 1,
        values: fc.integer({ min: -0x80, max: 0x7f }),
        write: (view, position, value: number) => view.setInt8(position, value)
    },
    setUint8: {
        size: 1,
        values: fc.integer({ min: 0, max: 0xff }),
        write: (view, position, value: number) => view.setUint8(position, value)
    },
    setInt16: {
        size: 2,
        values: fc.integer({ min: -0x8000, max: 0x7fff }),
        write: (view, position, value: number) => view.setInt16(position, value, true)
    },
    setUint16: {
        size: 2,
        values: fc.integer({ min: 0, max: 0xffff }),
        write: (view, position, value: number) => view.setUint16(position, value, true)
    },
    setInt32: {
        size: 4,
        values: fc.integer({ min: -0x8000_0000, max: 0x7fff_ffff }),
        write: (view, position, value: number) => view.setInt32(position, value, true)
    },
    setUint32: {
        size: 4,
        values: fc.integer({ min: 0, max: 0xffff_ffff }),
        write: (view, position, value: number) => view.setUint32(position, value, true)
    },
    setInt64: {
        size: 8,
        values: fc.bigInt({ min: -(2n ** 63n), max: 2n ** 63n - 1n }),
        write: (view, position, value: bigint) => view.setBigInt64(position, value, true)
    },
    setUint64: {
        size: 8,
        values: fc.bigInt({ min: 0n, max: 2n ** 64n - 1n }),
        write: (view, position, value: bigint) => view.setBigUint64(position, value, true)
    },
    setFloat32: {
        size: 4,
        values: fc.float(),
        write: (view, position, value: number) => view.setFloat32(position, value, true)
    },
    setFloat64: {
        size: 8,
        values: fc.double(),
        write: (view, position, value: number) => view.setFloat64(position, value, true)
    }
};

/** Fields of each type, with the value of the same field of another struct */
const FIELD_PAIRS = fc.oneof(
    ...Object.entries(SCALARS).map(([setter, { values }]) =>
        fc.tuple(values, values).map(([first, second]): [Field, Field] => [
            { setter: setter as Setter, value: first },
            { setter: setter as Setter, value: second }
        ])
    )
);

it.prop([fc.array(FIELD_PAIRS, { maxLength: 8 })])(
    'should write scalars one after another, little-endian, and copy each struct',
    pairs => {
        const firsts = pairs.map(([first]) => first);
        const seconds = pairs.map(([, second]) => second);
        const writer = new StructWriter(new Uint8Array(sizeOf(firsts)));

        writeStruct(writer, firsts);
        const first = writer.bytes();
        writeStruct(writer, seconds);
        const second = writer.bytes();

        // bytes() rewinds, so the second struct starts at the start too, and
        // copies, so writing it left the first struct's bytes as they were
        expect(first).toStrictEqual(littleEndian(firsts));
        expect(second).toStrictEqual(littleEndian(seconds));
    }
);

it.prop([fc.array(FIELD_PAIRS, { minLength: 1, maxLength: 8 })])(
    'should write the next struct from the start after reset() discards an unfinished one',
    pairs => {
        const firsts = pairs.map(([first]) => first);
        const seconds = pairs.map(([, second]) => second);
        const writer = new StructWriter(new Uint8Array(sizeOf(firsts)));

        writeStruct(writer, firsts.slice(0, -1));
        writer.reset();
        writeStruct(writer, seconds);

        expect(writer.bytes()).toStrictEqual(littleEndian(seconds));
    }
);

function writeStruct(writer: StructWriter, fields: Field[]): void {
    for (const { setter, value } of fields) {
        (writer[setter] as (value: number | bigint) => void).call(writer, value);
    }
}

/** The bytes of a struct of `fields`, as DataView writes them */
function littleEndian(fields: Field[]): Uint8Array {
    const bytes = new Uint8Array(sizeOf(fields));
    const view = new DataView(bytes.buffer);

    let position = 0;
    for (const { setter, value } of fields) {
        const { size, write } = SCALARS[setter];
        write(view, position, value as never);
        position += size;
    }

    return bytes;
}

function sizeOf(fields: Field[]): number {
    return fields.reduce((size, { setter }) => size + SCALARS[setter].size, 0);
}

/** Each read helper, the size it reads and DataView's little-endian read */
const READS: ReadonlyArray<
    [
        read: (bytes: Uint8Array, offset: number) => number | bigint,
        size: number,
        reference: (view: DataView, offset: number) => number | bigint
    ]
> = [
    [readUint8, 1, (view, offset) => view.getUint8(offset)],
    [readInt32, 4, (view, offset) => view.getInt32(offset, true)],
    [readUint32, 4, (view, offset) => view.getUint32(offset, true)],
    [readInt64, 8, (view, offset) => view.getBigInt64(offset, true)],
    [readFloat32, 4, (view, offset) => view.getFloat32(offset, true)]
];

it.prop([fc.uint8Array({ maxLength: 12 }), fc.integer({ min: -2, max: 14 }), SLICES])(
    'should read little-endian values within the bytes, and throw DecodeError past them',
    (bytes, offset, slice) => {
        const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
        const sliced = sliceOf(bytes, slice);

        for (const [read, size, reference] of READS) {
            if (offset >= 0 && offset + size <= bytes.length) {
                // toBe() compares with Object.is, so NaN equals NaN and -0 isn't 0
                expect(read(sliced, offset)).toBe(reference(view, offset));
            } else {
                expect(() => read(sliced, offset)).toThrow(DecodeError);
                expect(() => read(sliced, offset)).toThrow(
                    `Read exceeds the supplied buffer: ${size} bytes at ${offset} of ${bytes.length}`
                );
            }
        }
    }
);
