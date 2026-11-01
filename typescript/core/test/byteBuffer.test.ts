import { describe, expect } from 'vitest';
import { fc, it } from '@fast-check/vitest';
import {
    ByteBuffer,
    SIZEOF_8,
    SIZEOF_16,
    SIZEOF_32,
    SIZEOF_64,
    TableBuilder,
    TableLayout
} from '@buffy/core';
import { UINT_8_MAX, UINT_16_MAX, UINT_32_MAX } from './limits.ts';
import { IS_LITTLE_ENDIAN } from '#constants';
import { getSerializable, serializable, setSerializable, typedArrays } from './serializable.ts';

it.prop([fc.boolean(), fc.array(serializable)])(
    'should generate a deserializable `ByteBuffer`',
    (sized, entries) => {
        const builder = new TableBuilder(entries.length, TableLayout.create({ sized }));

        for (const entry of entries) {
            setSerializable(builder, entry);
        }

        const buffer = builder.finalize();

        for (const [index, [type, value]] of entries.entries()) {
            expect(getSerializable(buffer, type, index)).toStrictEqual(value);
        }
    }
);

it.prop([fc.boolean(), fc.array(typedArrays)])(
    'should serialize standard arrays',
    (sized, entries) => {
        const builder = new TableBuilder(entries.length, TableLayout.create({ sized }));

        for (const [type, values] of entries) {
            if (values instanceof Uint8Array) {
                setSerializable(builder, [type, values]);
            } else {
                setSerializable(builder, [type, [...values]]);
            }
        }

        const buffer = builder.finalize();

        for (const [index, [type, value]] of entries.entries()) {
            expect(getSerializable(buffer, type, index)).toStrictEqual(value);
        }
    }
);

const byteOffsets = fc.integer({ min: 0, max: 16 });

it.prop([fc.boolean(), fc.array(serializable), byteOffsets])(
    'should deserialize a `ByteBuffer` from a subarray at any byte offset',
    (sized, entries, byteOffset) => {
        const builder = new TableBuilder(entries.length, TableLayout.create({ sized }));

        for (const entry of entries) {
            setSerializable(builder, entry);
        }

        const buffer = new ByteBuffer(
            toSubarray(builder.bytes(), byteOffset),
            TableLayout.create({ sized })
        );

        for (const [index, [type, value]] of entries.entries()) {
            expect(getSerializable(buffer, type, index)).toStrictEqual(value);
        }
    }
);

it.prop([fc.float64Array({ minLength: 1 }), fc.integer({ min: 1, max: 7 })])(
    'should copy typed arrays that are unaligned within the underlying buffer',
    (float64s, byteOffset) => {
        const builder = new TableBuilder(1);
        builder.setFloat64Array(float64s);

        const bytes = toSubarray(builder.bytes(), byteOffset);
        const buffer = new ByteBuffer(bytes);
        const values = buffer.getFloat64Array(buffer.rootOffset(0));

        expect(values).toStrictEqual(float64s);
        expect(values.buffer).not.toBe(bytes.buffer);
    }
);

describe('array byte order', () => {
    it('should share the storage of aligned arrays, except swapped copies on a big-endian host', () => {
        const builder = new TableBuilder(2);
        builder.setUint8Array(new Uint8Array([1, 2, 3]));
        builder.setFloat64Array(new Float64Array([1.5]));

        const bytes = builder.bytes();
        const buffer = new ByteBuffer(bytes);
        const uint8s = buffer.getUint8Array(buffer.rootOffset(0));
        const float64s = buffer.getFloat64Array(buffer.rootOffset(1));

        expect(uint8s.buffer).toBe(bytes.buffer);
        expect(float64s.buffer === bytes.buffer).toBe(IS_LITTLE_ENDIAN);
    });

    // Float32 elements are aligned at byte offsets 0 and 4, and Float64 elements at 4
    it.each([0, 1, 4])(
        'should read -0 and NaN from little-endian float arrays at byte offset %s',
        byteOffset => {
            const values = [-0, Number.NaN, 0, Number.NEGATIVE_INFINITY, -1.5];

            const float32s = littleEndianFloats(values, 'Float32', byteOffset).getFloat32Array(0);
            const float64s = littleEndianFloats(values, 'Float64', byteOffset).getFloat64Array(0);

            for (const [i, value] of values.entries()) {
                expect(float32s[i]).toBe(value);
                expect(float64s[i]).toBe(value);
            }
        }
    );
});

/**
 * Returns a `ByteBuffer` that starts at `byteOffset` in its backing buffer and
 * holds `values` as a length-prefixed array, written with a `DataView`.
 */
function littleEndianFloats(
    values: number[],
    type: 'Float32' | 'Float64',
    byteOffset: number
): ByteBuffer {
    const elementSize = type === 'Float32' ? SIZEOF_32 : SIZEOF_64;
    const backing = new ArrayBuffer(byteOffset + SIZEOF_32 + values.length * elementSize);
    const view = new DataView(backing, byteOffset);

    view.setUint32(0, values.length, true);
    for (const [i, value] of values.entries()) {
        view[`set${type}`](SIZEOF_32 + i * elementSize, value, true);
    }

    return new ByteBuffer(new Uint8Array(backing, byteOffset));
}

it.prop([fc.boolean(), fc.array(serializable, { maxLength: 8 })])(
    'should generate a deserializable `ByteBuffer` with 16-bit sizes',
    (sized, entries) => {
        const builder = new TableBuilder(
            entries.length,
            TableLayout.create({ sized, offsetSize: 2 })
        );

        for (const entry of entries) {
            setSerializable(builder, entry);
        }

        const buffer = builder.finalize();

        for (const [index, [type, value]] of entries.entries()) {
            expect(getSerializable(buffer, type, index)).toStrictEqual(value);
        }
    }
);

it('should write 16-bit sizes and offsets as little-endian', () => {
    const builder = new TableBuilder(1, TableLayout.create({ sized: true, offsetSize: 2 }));
    builder.setString('a'.repeat(300));

    // Buffer size (306), offset of the string (4) and string length (300)
    expect([...builder.bytes().subarray(0, 6)]).toStrictEqual([0x32, 0x01, 0x04, 0x00, 0x2c, 0x01]);
});

describe('versioned tables', () => {
    it.prop([fc.boolean(), fc.integer({ min: 0, max: UINT_32_MAX }), fc.array(serializable)])(
        'should read back the version and fields',
        (sized, version, entries) => {
            const builder = new TableBuilder(
                entries.length,
                TableLayout.create({ sized, versioned: true }),
                version
            );

            for (const entry of entries) {
                setSerializable(builder, entry);
            }

            const buffer = new ByteBuffer(
                builder.bytes(),
                TableLayout.create({ sized, versioned: true })
            );

            expect(buffer.getVersion()).toBe(version);
            for (const [index, [type, value]] of entries.entries()) {
                expect(getSerializable(buffer, type, index)).toStrictEqual(value);
            }
        }
    );

    it.each([
        // Length (4), version (7), offset of the field (3) and the field (42)
        [SIZEOF_8, true, [4, 7, 3, 42]],
        [SIZEOF_16, true, [7, 0, 7, 0, 6, 0, 42]],
        [SIZEOF_32, true, [13, 0, 0, 0, 7, 0, 0, 0, 12, 0, 0, 0, 42]],
        [SIZEOF_32, false, [7, 0, 0, 0, 8, 0, 0, 0, 42]]
    ] as const)(
        'should write the version in a slot of %s bytes after the length if sized (%s)',
        (size, sized, expected) => {
            const builder = new TableBuilder(
                1,
                TableLayout.create({ sized, versioned: true, offsetSize: size }),
                7
            );
            builder.setUint8(42);

            expect([...builder.bytes()]).toStrictEqual(expected);
        }
    );

    it.each([
        [SIZEOF_8, UINT_8_MAX + 1],
        [SIZEOF_16, UINT_16_MAX + 1],
        [SIZEOF_32, UINT_32_MAX + 1],
        [SIZEOF_32, -1],
        [SIZEOF_32, 1.5]
    ] as const)(
        'should reject a version that a slot of %s bytes cannot hold (%s)',
        (size, version) => {
            const layout = TableLayout.create({ versioned: true, offsetSize: size });

            expect(() => new TableBuilder(1, layout, version)).toThrow(RangeError);
        }
    );

    it('should reject reading the version of an unversioned table', () => {
        const layout = TableLayout.create({ sized: true });
        const buffer = new ByteBuffer(new TableBuilder(0, layout).bytes(), layout);

        expect(() => buffer.getVersion()).toThrow(TypeError);
    });

    it('should require a version exactly when the layout has a slot for it', () => {
        const versioned = TableLayout.create({ versioned: true });
        const unversioned = TableLayout.create({ sized: true });

        // @ts-expect-error: A versioned layout needs a version
        expect(() => new TableBuilder(1, versioned)).toThrow(TypeError);
        // @ts-expect-error: The layout has no version slot
        expect(() => new TableBuilder(1, unversioned, 1)).toThrow(TypeError);
    });
});

/** Returns a copy of `bytes` that is a view into a larger buffer, starting at `byteOffset`. */
function toSubarray(bytes: Uint8Array, byteOffset: number): Uint8Array {
    const buffer = new Uint8Array(byteOffset + bytes.length + 8).fill(0xff);
    buffer.set(bytes, byteOffset);

    return buffer.subarray(byteOffset, byteOffset + bytes.length);
}

it.each([0, 1, 3, 8])('should respect a byte slice starting at offset %s', offset => {
    const backing = new Uint8Array(offset + 20);
    const bytes = backing.subarray(offset, offset + 8);
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    view.setUint32(0, 1, true);
    view.setFloat32(4, 12.5, true);
    const buffer = new ByteBuffer(bytes);
    expect([...buffer.getFloat32Array(0)]).toStrictEqual([12.5]);
});

// A slice of 8 bytes, followed by bytes that a read past its end would return.
function sliceOf(...uint32s: number[]): ByteBuffer {
    const backing = new Uint8Array(32).fill(65);
    const bytes = backing.subarray(4, 12);
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    for (const [i, uint32] of uint32s.entries()) {
        view.setUint32(i * 4, uint32, true);
    }

    return new ByteBuffer(bytes);
}

it('should reject an array outside the supplied slice', () => {
    const buffer = sliceOf(5);

    expect(() => buffer.getUint8Array(0)).toThrow('Read exceeds the supplied buffer');
});

it('should reject a string outside the supplied slice', () => {
    const buffer = sliceOf(5);

    expect(() => buffer.getString(0)).toThrow('Read exceeds the supplied buffer');
});

it('should reject a vector item outside the supplied slice', () => {
    const buffer = sliceOf(1, 5);

    expect(() => buffer.getVector(0)).toThrow('Read exceeds the supplied buffer');
});

it('should reject impossible vector counts before allocating their offsets', () => {
    const buffer = new ByteBuffer(new Uint8Array([255, 255, 255, 255]));

    // Allocating the offsets first would fail with a different error
    expect(() => buffer.getVectorOffsets(0)).toThrow('Read exceeds the supplied buffer');
});

it('should reject an Int8 array written past the supplied slice', () => {
    const backing = new Uint8Array(16);
    const buffer = new ByteBuffer(backing.subarray(0, 6));

    expect(() => buffer.setInt8Array([1, 2, 3, 4, 5, 6, 7, 8])).toThrow(RangeError);
    expect([...backing.subarray(6)]).toStrictEqual(Array(10).fill(0));
});

it('should decode strings relative to the supplied slice', () => {
    const bytes = new Uint8Array([99, 99, 2, 0, 0, 0, 111, 107, 99]).subarray(2, 8);
    expect(new ByteBuffer(bytes).getString(0)).toBe('ok');
});

interface Conversion {
    write: (builder: TableBuilder) => void;
    read: (buffer: ByteBuffer, position: number) => ArrayLike<number>;
    expected: ArrayLike<number>;
}

it.each<[string, Conversion]>([
    [
        'a Float64Array to Float32',
        {
            write: builder => builder.setFloat32Array(new Float64Array([1.25, -2.5, Math.PI])),
            read: (buffer, position) => buffer.getFloat32Array(position),
            expected: new Float32Array([1.25, -2.5, Math.PI])
        }
    ],
    [
        'a Float32Array to Float64',
        {
            write: builder => builder.setFloat64Array(new Float32Array([3.5, -4.25])),
            read: (buffer, position) => buffer.getFloat64Array(position),
            expected: new Float64Array([3.5, -4.25])
        }
    ],
    [
        'a Float32Array to Int32',
        {
            write: builder => builder.setInt32Array(new Float32Array([1.75, -2.75])),
            read: (buffer, position) => buffer.getInt32Array(position),
            expected: new Int32Array([1, -2])
        }
    ],
    [
        'an Int32Array to Float32',
        {
            write: builder => builder.setFloat32Array(new Int32Array([3, -4])),
            read: (buffer, position) => buffer.getFloat32Array(position),
            expected: new Float32Array([3, -4])
        }
    ]
])('should convert %s when serializing', (_, { write, read, expected }) => {
    const builder = new TableBuilder(1);
    write(builder);

    const buffer = builder.finalize();

    expect(read(buffer, buffer.rootOffset(0))).toStrictEqual(expected);
});
