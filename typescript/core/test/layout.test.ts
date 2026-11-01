import { describe, expect } from 'vitest';
import { fc, it } from '@fast-check/vitest';
import { ByteBuffer, SIZEOF_8, SIZEOF_16, SIZEOF_32, TableBuilder, TableLayout } from '@buffy/core';
import { getSerializableAt, serializable, setSerializable } from './serializable.ts';

describe('TableLayout', () => {
    it('should default to no header slots and 32-bit offsets', () => {
        for (const layout of [TableLayout.DEFAULT, TableLayout.create()]) {
            expect(layout.sized()).toBe(false);
            expect(layout.versioned()).toBe(false);
            expect(layout.offsetSize()).toBe(SIZEOF_32);
            expect(layout.headerSize()).toBe(0);
        }
    });

    it.each([
        [false, false, SIZEOF_8, 0],
        [true, false, SIZEOF_8, 1],
        [false, true, SIZEOF_16, 2],
        [true, true, SIZEOF_16, 4],
        [true, true, SIZEOF_32, 8]
    ] as const)(
        'should have header slots for sized (%s) and versioned (%s) of %s bytes',
        (sized, versioned, offsetSize, headerSize) => {
            const layout = TableLayout.create({ sized, versioned, offsetSize });

            expect(layout.headerSize()).toBe(headerSize);
        }
    );
});

describe('ByteBuffer.fieldOffset()', () => {
    it.prop([fc.array(serializable, { maxLength: 6 }), fc.array(serializable, { maxLength: 6 })])(
        'should read the fields of a nested table',
        (parentEntries, nestedEntries) => {
            const nested = new TableBuilder(nestedEntries.length);
            for (const entry of nestedEntries) {
                setSerializable(nested, entry);
            }

            const layout = TableLayout.create({ sized: true });
            const parent = new TableBuilder(parentEntries.length + 1, layout);
            for (const entry of parentEntries) {
                setSerializable(parent, entry);
            }
            parent.setTable(nested.bytes());

            const buffer = new ByteBuffer(parent.bytes(), layout);
            const tablePosition = buffer.rootOffset(parentEntries.length);

            for (const [index, [type, value]] of nestedEntries.entries()) {
                const position = buffer.fieldOffset(tablePosition, index);
                expect(getSerializableAt(buffer, type, position)).toStrictEqual(value);
            }
        }
    );
});

describe('TableBuilder.setTable()', () => {
    it.prop([fc.string(), fc.constantFrom(SIZEOF_8, SIZEOF_16, SIZEOF_32, 8 as const)])(
        'should start the nested table at a multiple of the alignment',
        (string, alignment) => {
            const builder = new TableBuilder(2);
            builder.setString(string);
            builder.setTable(new Uint8Array([1, 2, 3]), alignment);

            const buffer = builder.finalize();

            expect(buffer.rootOffset(1) % alignment).toBe(0);
            expect([...buffer.bytes().subarray(buffer.rootOffset(1))]).toStrictEqual([1, 2, 3]);
        }
    );

    it.prop([fc.string(), fc.float64Array({ minLength: 1 })])(
        'should keep 64-bit arrays of a table aligned to 8 as views',
        (string, values) => {
            const nested = new TableBuilder(1);
            nested.setFloat64Array(values);

            const builder = new TableBuilder(2);
            builder.setString(string);
            builder.setTable(nested.bytes(), 8);

            const buffer = builder.finalize();
            const read = buffer.getFloat64Array(buffer.fieldOffset(buffer.rootOffset(1), 0));

            expect(read).toStrictEqual(values);
            expect(read.buffer).toBe(buffer.bytes().buffer);
        }
    );
});
