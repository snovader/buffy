import { describe, expect } from 'vitest';
import { fc, it } from '@fast-check/vitest';
import { SIZEOF_8, SIZEOF_16, TableBuilder, TableLayout } from 'tafla';
import { MAX_VALUES, tables } from './conformance/arbitraries.ts';
import type { FieldDescription, TableDescription } from './conformance/description.ts';
import { alignmentOf, buildTable } from './conformance/tables.ts';

it('should reject a field beyond the number the table holds', () => {
    const builder = new TableBuilder(1);
    builder.setUint8(1);

    expect(() => builder.setUint8(2)).toThrow(new RangeError("Can't add field 2 to a table of 1"));
});

it.prop([fc.nat({ max: 4 })])('should reject finalizing before every field is set', set => {
    const builder = new TableBuilder(set + 1);
    for (let i = 0; i < set; i++) {
        builder.setFloat64(i);
    }

    expect(() => builder.finalize()).toThrow(
        `Only ${set} of the table's ${set + 1} fields were set`
    );
});

it('should reject setting a field after finalizing', () => {
    const builder = new TableBuilder(0);
    builder.finalize();

    expect(() => builder.setInt8(1)).toThrow(TypeError);
});

it('should finalize once and return the same buffer', () => {
    const builder = new TableBuilder(1);
    builder.setString('once');

    expect(builder.finalize()).toBe(builder.finalize());
    expect(builder.bytes()).toBe(builder.finalize().bytes());
});

it('should write values as they are when finalizing', () => {
    const values = new Float32Array([1, 2]);
    const builder = new TableBuilder(1);
    builder.setFloat32Array(values);
    values[0] = 3;

    const buffer = builder.finalize();
    expect(buffer.getFloat32Array(buffer.rootOffset(0))).toStrictEqual(new Float32Array([3, 2]));
});

describe('tables longer than the offsets address', () => {
    it.each([SIZEOF_8, SIZEOF_16] as const)(
        'should reject a header longer than %s-byte offsets address',
        offsetSize => {
            const max = MAX_VALUES[offsetSize];

            for (const sized of [false, true]) {
                const layout = TableLayout.create({ sized, offsetSize });
                const fields = Math.floor(max / offsetSize) - (sized ? 1 : 0);

                expect(() => new TableBuilder(fields, layout)).not.toThrow();
                expect(() => new TableBuilder(fields + 1, layout)).toThrow(
                    `A header of ${fields + 1} fields doesn't fit in ${offsetSize}-byte offsets`
                );
            }
        }
    );

    it('should reject a table of 32-bit offsets past 4 GiB before allocating it', () => {
        // An array-like stands in for a 4 GiB array, whose elements aren't read.
        // The table would hold its offset, its length and 2^32 elements.
        const builder = new TableBuilder(1);
        expect(() => builder.setInt8Array({ length: MAX_VALUES[4] + 1 })).toThrow(
            new RangeError('Field 1 makes the table 4294967304 bytes, too long for 4-byte offsets')
        );

        builder.setInt8Array([1]);
        const buffer = builder.finalize();
        expect(buffer.getInt8Array(buffer.rootOffset(0))).toStrictEqual(new Int8Array([1]));
    });
});

describe('tables nested from their builders', () => {
    // The nested table follows a uint8[] of any length up to 7, which
    // starts it at any offset from an alignment of 8
    it.prop([tables({ headerless: true }), fc.nat({ max: 7 })])(
        'should write the same bytes as nesting the bytes that the builder writes, at the same alignment',
        (table, length) => {
            const parent: TableDescription = {
                layout: { sized: false, versioned: false, offsetSize: 4 },
                fields: [
                    { type: 'uint8[]', value: new Array<number>(length).fill(0) },
                    { type: 'table', from: 'builder', table }
                ]
            };

            expect(buildTable(nestedTheOtherWay(parent)).bytes()).toStrictEqual(
                buildTable(parent).bytes()
            );
        }
    );

    it('should reject a builder with fields left to set, and leave the table as it was', () => {
        const nested = new TableBuilder(2);
        nested.setUint8(1);

        const parent = new TableBuilder(2);
        expect(() => parent.setTable(nested)).toThrow("Only 1 of the table's 2 fields were set");
        expect(() => parent.setVector([new Uint8Array(1), nested])).toThrow(RangeError);

        nested.setUint8(2);
        parent.setTable(nested);
        parent.setVector([nested]);

        const buffer = parent.finalize();
        const table = buffer.rootOffset(0);
        expect(buffer.getUint8(buffer.fieldOffset(table, 1))).toBe(2);
        expect(buffer.getVector(buffer.rootOffset(1))).toStrictEqual([nested.bytes()]);
    });

    it('should write a builder nested several times, or finalized itself, the same each time', () => {
        const nested = new TableBuilder(2);
        nested.setFloat64Array([1, 2]);
        nested.setString('nested');

        // Written straight into the parent, and then copied once finalized
        const first = new TableBuilder(2);
        first.setTable(nested);
        first.setVector([nested, nested]);
        const written = first.bytes();

        const own = nested.bytes();
        const second = new TableBuilder(2);
        second.setTable(nested);
        second.setVector([nested, nested]);

        expect(second.bytes()).toStrictEqual(written);

        const buffer = second.finalize();
        expect(buffer.getVector(buffer.rootOffset(1))).toStrictEqual([own, own]);
    });

    it("should write a nested builder's values as they are when its parent is finalized", () => {
        const values = new Float32Array([1, 2]);
        const nested = new TableBuilder(1);
        nested.setFloat32Array(values);

        const parent = new TableBuilder(1);
        parent.setTable(nested);
        values[0] = 3;

        const buffer = parent.finalize();
        const read = buffer.getFloat32Array(buffer.fieldOffset(buffer.rootOffset(0), 0));
        expect(read).toStrictEqual(new Float32Array([3, 2]));
    });
});

/**
 * Returns the table with every nested table passed the other way: a builder
 * as its bytes and bytes as their builder, at the alignment that it had
 */
function nestedTheOtherWay(table: TableDescription): TableDescription {
    return { ...table, fields: table.fields.map(fieldNestedTheOtherWay) };
}

function fieldNestedTheOtherWay(field: FieldDescription): FieldDescription {
    switch (field.type) {
        case 'table':
            return {
                type: 'table',
                from: field.from === 'builder' ? 'bytes' : 'builder',
                alignment: alignmentOf(field),
                table: nestedTheOtherWay(field.table)
            };

        case 'vector':
            return {
                type: 'vector',
                items: field.items.map(({ from, table }) => ({
                    from: from === 'builder' ? 'bytes' : 'builder',
                    table: nestedTheOtherWay(table)
                }))
            };

        default:
            return field;
    }
}
