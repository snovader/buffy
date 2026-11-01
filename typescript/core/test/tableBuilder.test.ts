import { describe, expect } from 'vitest';
import { fc, it } from '@fast-check/vitest';
import { ByteBuffer, SIZEOF_8, SIZEOF_16, TableBuilder, TableLayout } from '@buffy/core';
import { UINT_8_MAX, UINT_16_MAX, UINT_32_MAX } from './limits.ts';
import {
    getSerializable,
    getSerializableAt,
    serializable,
    setSerializable
} from './serializable.ts';

it('should reject a field beyond the number the table holds', () => {
    const builder = new TableBuilder(1);
    builder.setUint8(1);

    expect(() => builder.setUint8(2)).toThrow(RangeError);
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

describe('tables longer than the offsets address', () => {
    const limits = [
        [SIZEOF_8, UINT_8_MAX],
        [SIZEOF_16, UINT_16_MAX]
    ] as const;

    // A field of `length` elements, and the bytes each element adds
    const fields: Array<
        [string, (offsetSize: number) => number, (length: number) => [string, unknown]]
    > = [
        ['a byte array', () => 1, length => ['uint8array', new Uint8Array(length)]],
        ['a typed array', () => 8, length => ['float64array', new Float64Array(length)]],
        ['a string', () => 1, length => ['string', 'a'.repeat(length)]],
        [
            "a vector's count",
            offsetSize => offsetSize,
            length => ['vector', Array.from({ length }, () => new Uint8Array(0))]
        ],
        ['a vector item', () => 1, length => ['vector', [new Uint8Array(length)]]]
    ];

    const cases = limits.flatMap(([offsetSize, max]) =>
        [false, true].flatMap(sized =>
            fields.map(
                ([name, unit, field]) => [name, offsetSize, sized, max, unit, field] as const
            )
        )
    );

    it.each(cases)(
        'should fit %s in a table of %s-byte offsets, sized (%s), up to %s bytes',
        (_, offsetSize, sized, max, unit, field) => {
            const layout = TableLayout.create({ sized, offsetSize });

            const empty = new TableBuilder(1, layout);
            setSerializable(empty, field(0));
            const length = Math.floor((max - empty.bytes().length) / unit(offsetSize));

            const longest = new TableBuilder(1, layout);
            const [type, value] = field(length);
            setSerializable(longest, [type, value]);

            const buffer = longest.finalize();
            expect(buffer.bytes().length).toBeLessThanOrEqual(max);
            expect(buffer.bytes().length).toBeGreaterThan(max - unit(offsetSize));
            expect(getSerializable(buffer, type, 0)).toStrictEqual(value);

            // The setter throws and leaves the builder as it was
            const tooLong = new TableBuilder(1, layout);
            expect(() => setSerializable(tooLong, field(length + 1))).toThrow(RangeError);
            setSerializable(tooLong, field(0));
            expect(tooLong.bytes()).toStrictEqual(empty.bytes());
        }
    );

    it.each(limits)(
        'should reject the field that makes a table of %s-byte offsets longer than %s bytes',
        (offsetSize, max) => {
            const layout = TableLayout.create({ offsetSize });

            // Two offsets, the array's length prefix and bytes, and then one byte
            const fits = new TableBuilder(2, layout);
            fits.setUint8Array(new Uint8Array(max - 3 * offsetSize - 1));
            fits.setUint8(7);

            const buffer = fits.finalize();
            expect(buffer.bytes()).toHaveLength(max);
            expect(buffer.getUint8(buffer.rootOffset(1))).toBe(7);

            const tooLong = new TableBuilder(2, layout);
            tooLong.setUint8Array(new Uint8Array(max - 3 * offsetSize));
            expect(() => tooLong.setUint8(7)).toThrow(RangeError);
        }
    );

    it.each(limits)(
        'should reject a header longer than %s-byte offsets address (%s bytes)',
        (offsetSize, max) => {
            for (const sized of [false, true]) {
                const layout = TableLayout.create({ sized, offsetSize });
                const fields = Math.floor(max / offsetSize) - (sized ? 1 : 0);

                expect(() => new TableBuilder(fields, layout)).not.toThrow();
                expect(() => new TableBuilder(fields + 1, layout)).toThrow(RangeError);
            }
        }
    );

    it('should reject a table of 32-bit offsets past 4 GiB before allocating it', () => {
        // An array-like stands in for a 4 GiB array, whose elements aren't read
        const builder = new TableBuilder(1);
        expect(() => builder.setInt8Array({ length: UINT_32_MAX + 1 })).toThrow(RangeError);

        builder.setInt8Array([1]);
        const buffer = builder.finalize();
        expect(buffer.getInt8Array(buffer.rootOffset(0))).toStrictEqual(new Int8Array([1]));
    });
});

it('should write values as they are when finalizing', () => {
    const values = new Float32Array([1, 2]);
    const builder = new TableBuilder(1);
    builder.setFloat32Array(values);
    values[0] = 3;

    const buffer = builder.finalize();
    expect(buffer.getFloat32Array(buffer.rootOffset(0))).toStrictEqual(new Float32Array([3, 2]));
});

describe('tables nested from their builders', () => {
    const entries = fc.array(serializable, { maxLength: 6 });

    it.prop([entries, entries])(
        "should write the same bytes as nesting the table's bytes at its arrays' alignment",
        (parentEntries, nestedEntries) => {
            const withBuilder = builderOf(parentEntries, 1);
            withBuilder.setTable(builderOf(nestedEntries));

            const withBytes = builderOf(parentEntries, 1);
            withBytes.setTable(builderOf(nestedEntries).bytes(), alignmentOf(nestedEntries));

            expect(withBuilder.bytes()).toStrictEqual(withBytes.bytes());
        }
    );

    it.prop([entries, entries, entries])(
        'should read back tables nested two deep, with their arrays as views',
        (outerEntries, middleEntries, innerEntries) => {
            const middle = builderOf(middleEntries, 1);
            middle.setTable(builderOf(innerEntries));

            const layout = TableLayout.create({ sized: true });
            const outer = builderOf(outerEntries, 1, layout);
            outer.setTable(middle);

            const buffer = new ByteBuffer(outer.bytes(), layout);
            const middlePosition = buffer.rootOffset(outerEntries.length);
            const innerPosition = buffer.fieldOffset(middlePosition, middleEntries.length);

            for (const [index, [type, value]] of innerEntries.entries()) {
                const read = getSerializableAt(
                    buffer,
                    type,
                    buffer.fieldOffset(innerPosition, index)
                );
                expect(read).toStrictEqual(value);

                if (ArrayBuffer.isView(read)) {
                    expect(read.buffer).toBe(buffer.bytes().buffer);
                }
            }
        }
    );

    it.prop([fc.array(entries, { maxLength: 5 })])(
        'should write a vector of builders as a vector of their bytes',
        items => {
            const withBuilders = new TableBuilder(1);
            withBuilders.setVector(items.map(item => builderOf(item)));

            const mixed = new TableBuilder(1);
            mixed.setVector(
                items.map((item, i) => (i % 2 === 0 ? builderOf(item) : builderOf(item).bytes()))
            );

            const withBytes = new TableBuilder(1);
            withBytes.setVector(items.map(item => builderOf(item).bytes()));

            expect(withBuilders.bytes()).toStrictEqual(withBytes.bytes());
            expect(mixed.bytes()).toStrictEqual(withBytes.bytes());
        }
    );

    it('should pad a builder without arrays by nothing', () => {
        const nested = new TableBuilder(1);
        nested.setFloat64(1.5);

        const parent = new TableBuilder(2);
        parent.setUint8(1);
        parent.setTable(nested);

        expect(parent.finalize().rootOffset(1)).toBe(2 * 4 + 1);
    });

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

    it('should reject a builder that makes its parent too long, and leave it as it was', () => {
        const nested = new TableBuilder(1);
        nested.setUint8Array(new Uint8Array(UINT_8_MAX - 8));

        const parent = new TableBuilder(1, TableLayout.create({ offsetSize: SIZEOF_8 }));
        expect(() => parent.setTable(nested)).toThrow(RangeError);
        expect(() => parent.setVector([nested])).toThrow(RangeError);

        parent.setVector([]);
        expect([...parent.bytes()]).toStrictEqual([1, 0]);
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

    it.each([
        [SIZEOF_8, TableLayout.DEFAULT],
        [4, TableLayout.create({ offsetSize: SIZEOF_16 })]
    ] as const)(
        'should copy a builder of %s-byte offsets in a vector as its own bytes',
        (offsetSize, parentLayout) => {
            const layout = TableLayout.create({ offsetSize });
            const nested = new TableBuilder(2, layout);
            nested.setString('another width');
            nested.setUint16Array([1, 2]);

            const parent = new TableBuilder(1, parentLayout);
            parent.setVector([nested]);

            const buffer = new ByteBuffer(parent.bytes(), parentLayout);
            const [item] = buffer.getVector(buffer.rootOffset(0));
            expect(item).toStrictEqual(nested.bytes());

            const read = new ByteBuffer(item, layout);
            expect(read.getString(read.rootOffset(0))).toBe('another width');
        }
    );

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

const ELEMENT_SIZES: Record<string, 1 | 2 | 4 | 8> = {
    int16array: 2,
    uint16array: 2,
    int32array: 4,
    uint32array: 4,
    float32array: 4,
    int64array: 8,
    uint64array: 8,
    float64array: 8
};

/** A builder with `entries` set and `more` fields left to set */
function builderOf(
    entries: Array<[string, unknown]>,
    more = 0,
    layout = TableLayout.DEFAULT
): TableBuilder {
    const builder = new TableBuilder(entries.length + more, layout);
    for (const entry of entries) {
        setSerializable(builder, entry);
    }

    return builder;
}

/** The largest element size of the entries' arrays, or 1 without any */
function alignmentOf(entries: Array<[string, unknown]>): 1 | 2 | 4 | 8 {
    let alignment: 1 | 2 | 4 | 8 = 1;
    for (const [type] of entries) {
        alignment = Math.max(alignment, ELEMENT_SIZES[type] ?? 1) as 1 | 2 | 4 | 8;
    }

    return alignment;
}
