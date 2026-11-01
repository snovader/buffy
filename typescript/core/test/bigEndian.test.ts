import { expect, vi } from 'vitest';
import { fc, it } from '@fast-check/vitest';
import { ByteBuffer, TableBuilder } from '@buffy/core';
import { getSerializable, setSerializable, typedArrays } from './serializable.ts';

// Runs buffy as if on a big-endian host. This host is little-endian, so the
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

it.prop([fc.array(typedArrays, { maxLength: 8 })])(
    'should read back every array type on a big-endian host',
    entries => {
        const builder = new TableBuilder(entries.length);
        for (const entry of entries) {
            setSerializable(builder, entry);
        }

        const buffer = builder.finalize();

        for (const [index, [type, values]] of entries.entries()) {
            expect(getSerializable(buffer, type, index)).toStrictEqual(values);
        }
    }
);
