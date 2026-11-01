import { describe, expect } from 'vitest';
import { it } from '@fast-check/vitest';
import { SIZEOF_32, TableLayout } from '@buffy/core';
import { LAYOUTS } from './conformance/arbitraries.ts';
import { headerSizeOf } from './conformance/format.ts';

describe('TableLayout', () => {
    it('should default to no header slots and 32-bit offsets', () => {
        for (const layout of [TableLayout.DEFAULT, TableLayout.create()]) {
            expect(layout.sized()).toBe(false);
            expect(layout.versioned()).toBe(false);
            expect(layout.offsetSize()).toBe(SIZEOF_32);
            expect(layout.headerSize()).toBe(0);
        }
    });

    it.prop([LAYOUTS])(
        'should have a header slot of the offset size for the length and for the version',
        ({ sized, versioned, offsetSize }) => {
            const layout = TableLayout.create({ sized, versioned, offsetSize });

            expect(layout.sized()).toBe(sized);
            expect(layout.versioned()).toBe(versioned);
            expect(layout.offsetSize()).toBe(offsetSize);
            expect(layout.headerSize()).toBe(headerSizeOf({ sized, versioned, offsetSize }));
        }
    );
});
