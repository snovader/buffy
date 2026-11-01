import { describe, expect } from 'vitest';
import { fc, it } from '@fast-check/vitest';
import { ByteBuffer, DecodeError } from '@buffy/core';
import {
    BYTE_OFFSETS,
    CORRUPTIONS,
    corruptionOf,
    FIELDS,
    fitsItsLayout,
    LAYOUTS,
    MAX_VALUES,
    SCALAR_TYPES,
    SLICES,
    sliceOf,
    tables
} from './conformance/arbitraries.ts';
import {
    corrupt,
    elementTypeOf,
    type FieldDescription,
    isArrayField,
    type LayoutDescription,
    type OffsetSize,
    type ScalarType,
    type TableDescription
} from './conformance/description.ts';
import { headerSizeOf, readScalar, readUnsigned, tablesIn } from './conformance/format.ts';
import {
    alignmentOf,
    buildTable,
    createBuilder,
    DescriptionMismatch,
    ELEMENT_SIZES,
    layoutOf,
    readTable,
    setField,
    TableReader
} from './conformance/tables.ts';

// The properties that every implementation of buffy shares, of tables built
// from arbitrary descriptions in the conformance suite's model. `tablesIn()`
// reads the bytes that the format rules check with DataView, independently
// of ByteBuffer.

it.prop([tables(), SLICES])(
    'should read back the values of a table built from any description, wherever its bytes lie',
    (table, slice) => {
        const bytes = sliceOf(buildTable(table).bytes(), slice);

        // Descriptions hold -0, NaN and the infinities by name and 64-bit
        // integers as strings, so that they compare exactly
        expect(readTable(bytes, table)).toStrictEqual(table);
    }
);

describe('format rules', () => {
    it.prop([tables()])("should hold a sized table's byte length in its size slot", table => {
        for (const {
            table: { layout },
            bytes
        } of tablesIn(buildTable(table).bytes(), table)) {
            const { sized, offsetSize } = layout;

            if (sized) {
                expect(readUnsigned(bytes, 0, offsetSize)).toBe(bytes.length);
            }
        }
    });

    it.prop([tables()])("should hold a versioned table's version in its version slot", table => {
        for (const { table: inner, bytes } of tablesIn(buildTable(table).bytes(), table)) {
            const { sized, versioned, offsetSize } = inner.layout;

            if (versioned) {
                const slot = sized ? offsetSize : 0;
                expect(readUnsigned(bytes, slot, offsetSize)).toBe(inner.version);
            }
        }
    });

    it.prop([tables()])(
        'should write offsets within the table, after its header and in the order of the fields',
        table => {
            for (const { table: inner, bytes, offsets } of tablesIn(
                buildTable(table).bytes(),
                table
            )) {
                const { layout, fields } = inner;
                let previous = headerSizeOf(layout) + fields.length * layout.offsetSize;

                for (const offset of offsets) {
                    expect(offset).toBeGreaterThanOrEqual(previous);
                    expect(offset).toBeLessThanOrEqual(bytes.length);
                    previous = offset;
                }
            }
        }
    );

    it.prop([tables()])(
        'should start every typed array at a multiple of its element size from the start of its table',
        table => {
            for (const { table: inner, offsets } of tablesIn(buildTable(table).bytes(), table)) {
                const { offsetSize } = inner.layout;

                for (const [index, field] of inner.fields.entries()) {
                    if (isArrayField(field)) {
                        const elementSize = ELEMENT_SIZES[elementTypeOf(field.type)];
                        expect((offsets[index] + offsetSize) % elementSize).toBe(0);
                    }
                }
            }
        }
    );

    it.prop([tables()])(
        "should start a table nested with setTable() at a multiple of its alignment from its parent's start",
        table => {
            for (const { table: parent, offsets } of tablesIn(buildTable(table).bytes(), table)) {
                for (const [index, field] of parent.fields.entries()) {
                    if (field.type === 'table') {
                        expect(offsets[index] % alignmentOf(field)).toBe(0);
                    }
                }
            }
        }
    );

    it.prop([tables()])(
        'should write multi-byte values, lengths and counts little-endian',
        table => {
            for (const { table: inner, bytes, offsets } of tablesIn(
                buildTable(table).bytes(),
                table
            )) {
                for (const [index, field] of inner.fields.entries()) {
                    expectLittleEndian(bytes, offsets[index], inner.layout.offsetSize, field);
                }
            }
        }
    );
});

const ENCODER = new TextEncoder();

/**
 * Expects a field's bytes, read with DataView, to hold its description's
 * values little-endian. A nested table's fields are checked as their own.
 */
function expectLittleEndian(
    bytes: Uint8Array,
    position: number,
    offsetSize: OffsetSize,
    field: FieldDescription
): void {
    if (field.type === 'table') {
        return;
    }

    if (field.type === 'vector') {
        expect(readUnsigned(bytes, position, offsetSize)).toBe(field.items.length);
        return;
    }

    if (field.type === 'string') {
        const utf8 = ENCODER.encode(field.value);
        const start = position + offsetSize;

        expect(readUnsigned(bytes, position, offsetSize)).toBe(utf8.length);
        expect(bytes.subarray(start, start + utf8.length)).toStrictEqual(utf8);
        return;
    }

    if (isArrayField(field)) {
        const type = elementTypeOf(field.type);
        const start = position + offsetSize;
        const values = field.value.map((_, index) =>
            readScalar(bytes, type, start + index * ELEMENT_SIZES[type])
        );

        expect(readUnsigned(bytes, position, offsetSize)).toBe(field.value.length);
        expect(values).toStrictEqual(field.value);
        return;
    }

    expect(readScalar(bytes, field.type, position)).toBe(field.value);
}

/** Every getter of a value at a position */
const GETTERS = [
    'getInt8',
    'getUint8',
    'getInt16',
    'getUint16',
    'getInt32',
    'getUint32',
    'getInt64',
    'getUint64',
    'getFloat32',
    'getFloat64',
    'getInt8Array',
    'getUint8Array',
    'getInt16Array',
    'getUint16Array',
    'getInt32Array',
    'getUint32Array',
    'getInt64Array',
    'getUint64Array',
    'getFloat32Array',
    'getFloat64Array',
    'getString',
    'getVector',
    'getVectorOffsets'
] as const;

type Getter = (typeof GETTERS)[number];

/** The index of a root field, and then of a field of each nested table */
const PATHS = fc.array(fc.nat({ max: 20 }), { minLength: 1, maxLength: 3 });

// Each property reads the same bytes from two buffers that hold different
// bytes around them, which a read that strayed outside them would see
describe('untrusted bytes', () => {
    // Each run reads a table cut at each of its lengths, often a hundred or
    // more, so fewer runs suffice
    it.prop([tables(), BYTE_OFFSETS], { numRuns: 20 })(
        'should read the fields of a table cut at any length as values or throw DecodeError, within its bytes',
        (table, byteOffset) => {
            const bytes = buildTable(table).bytes();

            for (let length = 0; length < bytes.length; length++) {
                expectReadsWithin(bytes.subarray(0, length), table, byteOffset);
            }
        }
    );

    it.prop([tables(), fc.array(CORRUPTIONS, { minLength: 1, maxLength: 3 }), BYTE_OFFSETS])(
        'should read the fields of a table with overwritten or cut bytes as values or throw DecodeError, within its bytes',
        (table, corruptions, byteOffset) => {
            let bytes = buildTable(table).bytes();
            for (const corruption of corruptions) {
                bytes = corrupt(bytes, corruptionOf(corruption, bytes.length));
            }

            expectReadsWithin(bytes, table, byteOffset);
        }
    );

    it.prop([
        fc.uint8Array({ maxLength: 64 }),
        LAYOUTS,
        fc.constantFrom(...GETTERS),
        PATHS,
        BYTE_OFFSETS
    ])(
        'should read any bytes with any getter at any field as a value or throw DecodeError, within the bytes',
        (bytes, layout, getter, path, byteOffset) => {
            expect(
                readPath(sliceOf(bytes, { byteOffset, fill: 0x00 }), layout, getter, path)
            ).toStrictEqual(
                readPath(sliceOf(bytes, { byteOffset, fill: 0xff }), layout, getter, path)
            );
        }
    );
});

/**
 * Expects the fields of a table, read as its description sets them out from
 * slices of two buffers that hold different bytes around them, to be the same
 * values or DecodeErrors
 */
function expectReadsWithin(bytes: Uint8Array, table: TableDescription, byteOffset: number): void {
    expect(readFields(sliceOf(bytes, { byteOffset, fill: 0x00 }), table)).toStrictEqual(
        readFields(sliceOf(bytes, { byteOffset, fill: 0xff }), table)
    );
}

/**
 * Reads the header and every root field of a table as its description sets
 * them out, and returns the values or DecodeError of each
 */
function readFields(bytes: Uint8Array, table: TableDescription): unknown[] {
    const { layout, fields } = table;
    const buffer = new ByteBuffer(bytes, layoutOf(layout));
    const reader = new TableReader(bytes, layout);

    return [
        valueOrDecodeError(() => buffer.getSize()),
        valueOrDecodeError(() => (layout.versioned ? buffer.getVersion() : null)),
        ...fields.map((field, index) =>
            valueOrDecodeError(() => readRootField(reader, index, field))
        )
    ];
}

/**
 * Reads a root field as its description sets it out. Corrupted bytes may
 * hold values that don't match the description, which this returns as the
 * mismatch.
 */
function readRootField(reader: TableReader, index: number, field: FieldDescription): unknown {
    try {
        return reader.readRootField(index, field);
    } catch (error) {
        if (error instanceof DescriptionMismatch) {
            return error;
        }

        throw error;
    }
}

/**
 * Reads the header of bytes in a layout, and then a value with the getter at
 * the end of a path of field offsets, and returns the values or DecodeError
 * of each
 */
function readPath(
    bytes: Uint8Array,
    layout: LayoutDescription,
    getter: Getter,
    [root, ...path]: number[]
): unknown[] {
    const buffer = new ByteBuffer(bytes, layoutOf(layout));

    return [
        valueOrDecodeError(() => buffer.getSize()),
        valueOrDecodeError(() => (layout.versioned ? buffer.getVersion() : null)),
        valueOrDecodeError(() => {
            let position = buffer.rootOffset(root);
            for (const index of path) {
                position = buffer.fieldOffset(position, index);
            }

            return buffer[getter](position);
        })
    ];
}

/** Returns what a read returns, or the DecodeError that it throws */
function valueOrDecodeError(read: () => unknown): unknown {
    try {
        return read();
    } catch (error) {
        // Neither a RangeError, which DecodeError extends, nor any other error
        expect(error).toBeInstanceOf(DecodeError);
        return error;
    }
}

describe('builder limits', () => {
    const NARROW_OFFSET_SIZES = [1, 2] as const;

    const tablesWithOversizedFields = fc.oneof(
        ...NARROW_OFFSET_SIZES.map(offsetSize =>
            fc.record({
                table: tables({ offsetSizes: [offsetSize], minFields: 1 }),
                index: fc.nat(),
                field: oversizedFields(offsetSize)
            })
        )
    );

    it.prop([tablesWithOversizedFields])(
        'should reject a field too long for the offset size from its setter, and leave the builder as it was',
        ({ table, index, field }) => {
            const { offsetSize } = table.layout;
            const at = index % table.fields.length;
            const builder = createBuilder(table);

            for (const before of table.fields.slice(0, at)) {
                setField(builder, before);
            }

            const error = thrownBy(() => setField(builder, field));
            const message = new RegExp(
                `^Field ${at + 1} makes the table (\\d+) bytes, too long for ${offsetSize}-byte offsets$`
            );
            expect(error).toBeInstanceOf(RangeError);
            const [, length] = (error as Error).message.match(message) ?? [];
            expect(Number(length)).toBeGreaterThan(MAX_VALUES[offsetSize]);

            for (const after of table.fields.slice(at)) {
                setField(builder, after);
            }
            expect(builder.bytes()).toStrictEqual(buildTable(table).bytes());
        }
    );

    // Fields after which a table's length doesn't depend on where they start
    const unpaddedFields = FIELDS.filter(
        field =>
            field.type !== 'table' &&
            !(isArrayField(field) && ELEMENT_SIZES[elementTypeOf(field.type)] > 1)
    );

    const tablesWithLastFields = fc.oneof(
        ...NARROW_OFFSET_SIZES.map(offsetSize =>
            fc.record({ table: tables({ offsetSizes: [offsetSize] }), last: unpaddedFields })
        )
    );

    it.prop([tablesWithLastFields])(
        'should build a table as long as its offset size addresses, and reject a field one byte longer',
        ({ table, last }) => {
            const { offsetSize } = table.layout;
            const max = MAX_VALUES[offsetSize];
            const withBytes = (length: number): TableDescription => ({
                ...table,
                fields: [...table.fields, bytesField(length), last]
            });

            const shortest = withBytes(0);
            fc.pre(fitsItsLayout(shortest));
            const room = max - buildTable(shortest).bytes().length;

            const longest = withBytes(room);
            const bytes = buildTable(longest).bytes();
            expect(bytes).toHaveLength(max);

            // Comparing up to 64 KiB of the uint8[] value by value is slow,
            // so it's compared by its length
            const read = readTable(bytes, longest);
            expect(read).toStrictEqual({
                ...longest,
                fields: [...table.fields, expect.anything(), last]
            });
            expect(read.fields.at(-2)).toHaveProperty('value.length', room);

            const builder = createBuilder(longest);
            for (const field of [...table.fields, bytesField(room + 1)]) {
                setField(builder, field);
            }

            const error = thrownBy(() => setField(builder, last));
            expect(error).toBeInstanceOf(RangeError);
            expect(error).toHaveProperty(
                'message',
                `Field ${table.fields.length + 2} makes the table ${max + 1} bytes, too long for ${offsetSize}-byte offsets`
            );
        }
    );
});

/** A uint8[] of `length` bytes */
function bytesField(length: number): FieldDescription {
    return { type: 'uint8[]', value: new Array<number>(length).fill(0) };
}

/**
 * Fields that are by themselves longer than a table of `offsetSize`-byte
 * offsets addresses: arrays, strings, nested tables and vectors with a long
 * item or many items
 */
function oversizedFields(offsetSize: OffsetSize): fc.Arbitrary<FieldDescription> {
    const max = MAX_VALUES[offsetSize];
    const layout = { sized: false, versioned: false, offsetSize: 4 } as const;
    const long: TableDescription = { layout, fields: [arrayField('uint8', max)] };
    const empty: TableDescription = { layout, fields: [] };
    const froms = fc.constantFrom('builder' as const, 'bytes' as const);

    const arrays = SCALAR_TYPES.map(type => arrayField(type, max));
    const string: FieldDescription = { type: 'string', value: 'a'.repeat(max) };
    const manyItems: FieldDescription = {
        type: 'vector',
        items: Array.from({ length: Math.ceil(max / offsetSize) }, () => ({
            from: 'bytes' as const,
            table: empty
        }))
    };

    return fc.oneof(
        fc.constantFrom(...arrays, string, manyItems),
        froms.map((from): FieldDescription => ({ type: 'table', from, table: long })),
        froms.map((from): FieldDescription => ({ type: 'vector', items: [{ from, table: long }] }))
    );
}

/** An array of at least `byteLength` bytes of elements */
function arrayField(type: ScalarType, byteLength: number): FieldDescription {
    const length = Math.ceil(byteLength / ELEMENT_SIZES[type]);
    const value = type === 'int64' || type === 'uint64' ? '1' : 1;

    return { type: `${type}[]`, value: new Array(length).fill(value) };
}

function thrownBy(action: () => void): unknown {
    try {
        action();
    } catch (error) {
        return error;
    }

    return undefined;
}
