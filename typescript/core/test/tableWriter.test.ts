import { describe, expect } from 'vitest';
import { fc, it } from '@fast-check/vitest';
import { SIZEOF_8, SIZEOF_16, TableBuilder, TableLayout, TableSizer } from 'tafla';
import { FIELDS, fitsItsLayout, MAX_VALUES, tables } from './conformance/arbitraries.ts';
import {
    elementTypeOf,
    type FieldDescription,
    isArrayField,
    type OffsetSize,
    type TableDescription
} from './conformance/description.ts';
import {
    buildTable,
    ELEMENT_SIZES,
    nestedInPlace,
    readTable,
    structAlignment,
    writeTable
} from './conformance/tables.ts';

/** Tables whose nested tables a TableWriter writes in place */
function inPlaceTables(offsetSizes?: readonly OffsetSize[]): fc.Arbitrary<TableDescription> {
    return tables({ offsetSizes }).map(nestedInPlace).filter(fitsItsLayout);
}

/** The bytes that `write` returns, or the class of the error that it throws */
function outcomeOf(write: () => Uint8Array): Uint8Array | string {
    try {
        return write();
    } catch (error) {
        return error instanceof Error ? error.constructor.name : String(error);
    }
}

it.prop([inPlaceTables()])(
    'should write the bytes that TableBuilder builds from the same fields',
    table => {
        expect(writeTable(table)).toStrictEqual(buildTable(table).bytes());
    }
);

it('should write a struct array, a nested table and a vector as TableBuilder pads them', () => {
    // A uint8 leaves a uint32[] 3 bytes short of its alignment, and a uint16
    // a nested table of float64s 1 byte short of its own
    const table: TableDescription = {
        layout: { sized: true, versioned: true, offsetSize: 2 },
        version: 7,
        fields: [
            { type: 'uint8', value: 1 },
            { type: 'uint32[]', value: [2, 3] },
            { type: 'struct[]', fields: ['uint16', 'float64'], items: [[4, 5]] },
            {
                type: 'table',
                from: 'builder',
                table: {
                    layout: { sized: false, versioned: false, offsetSize: 2 },
                    fields: [{ type: 'float64[]', value: [6] }]
                }
            },
            {
                type: 'vector',
                items: [
                    {
                        from: 'builder',
                        table: {
                            layout: { sized: false, versioned: false, offsetSize: 2 },
                            fields: [{ type: 'string', value: '🦀' }]
                        }
                    }
                ]
            }
        ]
    };

    const bytes = writeTable(table);
    expect(bytes).toStrictEqual(buildTable(table).bytes());
    expect(readTable(bytes, table)).toStrictEqual(table);
});

describe('strings', () => {
    // Any UTF-16 code units, lone surrogates too, around the length up to
    // which strings are written a byte at a time when they're ASCII
    const codeUnits = fc.integer({ min: 0, max: 0xffff }).map(unit => String.fromCharCode(unit));
    const strings = fc.oneof(
        fc.string({ unit: 'binary-ascii', maxLength: 80 }),
        fc.string({ unit: codeUnits, maxLength: 80 }),
        fc.string({ unit: 'binary', maxLength: 40 })
    );

    it.prop([strings, fc.constantFrom(...([1, 2, 4] as const))])(
        'should write any string as TableBuilder does, with lone surrogates as U+FFFD',
        (value, offsetSize) => {
            const table: TableDescription = {
                layout: { sized: false, versioned: false, offsetSize },
                fields: [{ type: 'string', value }]
            };
            fc.pre(fitsItsLayout(table));

            expect(writeTable(table)).toStrictEqual(buildTable(table).bytes());
        }
    );
});

describe('tables longer than the offsets address', () => {
    const NARROW_OFFSET_SIZES = [1, 2] as const;

    // Fields after which a table's length doesn't depend on where they start
    const unpaddedFields = FIELDS.filter(
        field =>
            field.type !== 'table' &&
            !(isArrayField(field) && ELEMENT_SIZES[elementTypeOf(field.type)] > 1) &&
            !(field.type === 'struct[]' && structAlignment(field.fields) > 1)
    ).map(field => nestedInPlace({ layout: LAYOUT_8, fields: [field] }).fields[0]);

    const tablesWithLastFields = fc.oneof(
        ...NARROW_OFFSET_SIZES.map(offsetSize =>
            fc.record({ table: inPlaceTables([offsetSize]), last: unpaddedFields })
        )
    );

    // Each run writes and builds tables of up to 64 KiB
    it.prop([tablesWithLastFields], { numRuns: 30 })(
        'should write a table as long as its offset size addresses, and throw RangeError for one a byte longer',
        ({ table, last }) => {
            const max = MAX_VALUES[table.layout.offsetSize];
            const withBytes = (length: number): TableDescription =>
                nestedInPlace({ ...table, fields: [...table.fields, bytesField(length), last] });

            const shortest = withBytes(0);
            fc.pre(fitsItsLayout(shortest));
            const room = max - buildTable(shortest).bytes().length;

            const longest = withBytes(room);
            const bytes = writeTable(longest);
            expect(bytes).toHaveLength(max);
            // Compared in a loop, since the matcher compares 64 KiB slowly
            expect(sameBytes(bytes, buildTable(longest).bytes())).toBe(true);

            const tooLong = withBytes(room + 1);
            expect(() => writeTable(tooLong)).toThrow(
                new RangeError(
                    `The table's fields make it ${max + 1} bytes, too long for ${table.layout.offsetSize}-byte offsets`
                )
            );
        }
    );

    it.prop([inPlaceTables(NARROW_OFFSET_SIZES), fc.nat({ max: 300 })])(
        'should throw RangeError for any table that TableBuilder rejects as too long',
        (table, length) => {
            const longer = { ...table, fields: [...table.fields, bytesField(length)] };

            expect(outcomeOf(() => writeTable(longer))).toStrictEqual(
                outcomeOf(() => buildTable(longer).bytes())
            );
        }
    );

    it('should reject a table of 32-bit offsets past 4 GiB before allocating it', () => {
        // The table would hold its offset, its length and 2^32 elements
        const sizer = new TableSizer(1);
        sizer.addInt8Array(MAX_VALUES[4] + 1);

        expect(() => sizer.writer()).toThrow(
            new RangeError(
                "The table's fields make it 4294967304 bytes, too long for 4-byte offsets"
            )
        );
    });

    it.each([SIZEOF_8, SIZEOF_16] as const)(
        'should reject a header longer than %s-byte offsets address, as TableBuilder does',
        offsetSize => {
            const max = MAX_VALUES[offsetSize];

            for (const sized of [false, true]) {
                const layout = TableLayout.create({ sized, offsetSize });
                const fields = Math.floor(max / offsetSize) - (sized ? 1 : 0);

                expect(() => new TableSizer(fields, layout)).not.toThrow();
                expect(() => new TableSizer(fields + 1, layout)).toThrow(
                    `A header of ${fields + 1} fields doesn't fit in ${offsetSize}-byte offsets`
                );
            }
        }
    );
});

describe('versions', () => {
    const versioned = TableLayout.create({ versioned: true, offsetSize: 1 });

    it.each([
        ['a version that the layout has no slot for', TableLayout.DEFAULT, 1, TypeError],
        ['no version for a versioned layout', versioned, undefined, TypeError],
        ['a version too large for its slot', versioned, 256, RangeError],
        ['a version that is not an integer', versioned, 1.5, RangeError]
    ])('should reject %s, as TableBuilder does', (_, layout, version, error) => {
        expect(() => new TableBuilder(0, layout as never, version as never)).toThrow(error);
        expect(() => new TableSizer(0, layout as never, version as never)).toThrow(error);
    });
});

describe('counts and sizes', () => {
    // Negative, fractional and non-finite numbers, and integers past those
    // that a double holds exactly
    const invalidCounts = fc.double().filter(count => !(Number.isSafeInteger(count) && count >= 0));

    it.prop([invalidCounts])(
        'should reject a number of fields that is not a non-negative integer, as TableBuilder does',
        fields => {
            const error = new RangeError(`A count must be a non-negative integer, not ${fields}`);

            expect(() => new TableBuilder(fields)).toThrow(error);
            expect(() => new TableSizer(fields)).toThrow(error);
        }
    );

    it.prop([invalidCounts])(
        'should reject a count that is not a non-negative integer, and leave the sizer and writer as they were',
        count => {
            const sizer = new TableSizer(2);
            expect(() => sizer.addUint16Array(count)).toThrow(RangeError);
            expect(() => sizer.addStructArray(count, 2, 2)).toThrow(RangeError);
            expect(() => sizer.addVector(count)).toThrow(RangeError);
            expect(() => sizer.startTable(count)).toThrow(RangeError);
            sizer.addUint16Array(1);
            sizer.addVector(0);

            const writer = sizer.writer();
            expect(() => writer.setVector(1, count)).toThrow(RangeError);
            expect(() => writer.startTable(count)).toThrow(RangeError);
            writer.setUint16Array(0, [7]);
            writer.setVector(1, 0);

            const builder = new TableBuilder(2);
            builder.setUint16Array([7]);
            builder.setVector([]);
            expect(writer.bytes()).toStrictEqual(builder.bytes());
        }
    );

    it.each([0, -2, 1.5, Number.NaN, Number.POSITIVE_INFINITY])(
        'should reject a struct of %s bytes',
        size => {
            const sizer = new TableSizer(1);
            const error = new RangeError(`A struct's size must be a positive integer, not ${size}`);

            expect(() => sizer.addStruct(size)).toThrow(error);
            expect(() => sizer.addStructArray(1, size, 1)).toThrow(error);
        }
    );
});

describe('fields that differ from those sized', () => {
    it('should throw RangeError for a string longer than the one sized', () => {
        const sizer = new TableSizer(1);
        sizer.addString('a');
        const writer = sizer.writer();

        expect(() => writer.setString(0, 'ab')).toThrow(RangeError);
    });

    it('should throw RangeError from bytes() for fields shorter than those sized', () => {
        const sizer = new TableSizer(1);
        sizer.addString('ab');
        const writer = sizer.writer();
        writer.setString(0, 'a');

        expect(() => writer.bytes()).toThrow(
            new RangeError("The fields wrote 9 of the table's 10 bytes")
        );
    });
});

const LAYOUT_8 = { sized: false, versioned: false, offsetSize: 1 } as const;

/** A uint8[] of `length` bytes */
function bytesField(length: number): FieldDescription {
    return { type: 'uint8[]', value: new Array<number>(length).fill(0) };
}

function sameBytes(left: Uint8Array, right: Uint8Array): boolean {
    return left.length === right.length && left.every((byte, index) => byte === right[index]);
}
