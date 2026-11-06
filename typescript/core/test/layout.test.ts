import { describe, expect } from 'vitest';
import { it } from '@fast-check/vitest';
import { type OffsetType, TableLayout } from 'tafla';
import { LAYOUTS } from './conformance/arbitraries.ts';
import { headerSizeOf, OFFSET_SIZES } from './conformance/format.ts';

describe('TableLayout', () => {
    it('should default to no header slots and uint32 offsets', () => {
        for (const layout of [TableLayout.DEFAULT, TableLayout.create()]) {
            expect(layout.sized()).toBe(false);
            expect(layout.versioned()).toBe(false);
            expect(layout.offsetType()).toBe('uint32');
            expect(layout.offsetSize()).toBe(4);
            expect(layout.headerSize()).toBe(0);
        }
    });

    it.prop([LAYOUTS])(
        'should have a header slot of the offset size for the length and for the version',
        ({ sized, versioned, offsetType }) => {
            const layout = TableLayout.create({ sized, versioned, offsetType });

            expect(layout.sized()).toBe(sized);
            expect(layout.versioned()).toBe(versioned);
            expect(layout.offsetType()).toBe(offsetType);
            expect(layout.offsetSize()).toBe(OFFSET_SIZES[offsetType]);
            expect(layout.headerSize()).toBe(headerSizeOf({ sized, versioned, offsetType }));
        }
    );

    it.each(['int16', 'uint64', 'toString', 2])(
        'should reject %s as an offset type',
        offsetType => {
            expect(() => TableLayout.create({ offsetType: offsetType as OffsetType })).toThrow(
                new RangeError(`${offsetType} isn't an offset type`)
            );
        }
    );
});
