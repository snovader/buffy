import { describe, expect } from 'vitest';
import { fc, it } from '@fast-check/vitest';
import { ByteBuffer, TableBuilder, TableLayout } from 'tafla';
import { growing } from './arrayLikes.ts';
import { LAYOUTS, MAX_VALUES, SLICES, sliceOf } from './conformance/arbitraries.ts';
import type { LayoutDescription } from './conformance/description.ts';
import { layoutOf } from './conformance/tables.ts';

const IS_LITTLE_ENDIAN = new Uint8Array(new Uint16Array([1]).buffer)[0] === 1;

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

it('should reject impossible vector counts before anything allocates for them', () => {
    const buffer = new ByteBuffer(new Uint8Array([255, 255, 255, 255]));

    // Allocating the offsets first would fail with a different error
    expect(() => buffer.getVectorLength(0)).toThrow('Read exceeds the supplied buffer');
    expect(() => buffer.getVectorOffsets(0)).toThrow('Read exceeds the supplied buffer');
    expect(() => buffer.getVector(0)).toThrow('Read exceeds the supplied buffer');
});

it.each([-1, 2, 0.5])('should reject item %s of a vector of 2 with a RangeError', index => {
    const builder = new TableBuilder(1);
    builder.setVector([new Uint8Array([1]), new Uint8Array([2, 3])]);
    const buffer = builder.finalize();

    expect(() => buffer.getVectorOffset(buffer.rootOffset(0), index)).toThrow(
        new RangeError(`Item ${index} is outside the vector of 2 at 4`)
    );
});

it('should find each vector item from its ends, as reading every item finds it', () => {
    const builder = new TableBuilder(1);
    builder.setVector([new Uint8Array([1]), new Uint8Array(0), new Uint8Array([2, 3])]);
    const buffer = builder.finalize();
    const vector = buffer.rootOffset(0);

    // The count and the three ends, at 4, then the items from 20
    expect(Array.from(buffer.getVectorOffsets(vector))).toStrictEqual([20, 21, 21]);
    expect([0, 1, 2].map(index => buffer.getVectorOffset(vector, index))).toStrictEqual([
        20, 21, 21
    ]);
    expect(buffer.getVectorLength(vector)).toBe(3);
    expect(buffer.getVector(vector)).toStrictEqual([
        new Uint8Array([1]),
        new Uint8Array(0),
        new Uint8Array([2, 3])
    ]);
});

describe('versioned tables', () => {
    it.each([
        [1, MAX_VALUES[1] + 1],
        [2, MAX_VALUES[2] + 1],
        [4, MAX_VALUES[4] + 1],
        [4, -1],
        [4, 1.5]
    ] as const)(
        'should reject a version that a slot of %s bytes cannot hold (%s)',
        (size, version) => {
            const layout = TableLayout.create({ versioned: true, offsetSize: size });

            expect(() => new TableBuilder(1, layout, version)).toThrow(
                new RangeError(`Version ${version} doesn't fit in a ${size}-byte slot`)
            );
        }
    );

    it('should reject reading the version of an unversioned table', () => {
        const layout = TableLayout.create({ sized: true });
        const buffer = new ByteBuffer(new TableBuilder(0, layout).bytes(), layout);

        expect(() => buffer.getVersion()).toThrow(new TypeError('The table has no version'));
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

/** A ByteBuffer setter, and the value to write with it */
interface Write {
    setter: Extract<keyof ByteBuffer, `set${string}`>;
    value: unknown;
}

function writes(
    setters: ReadonlyArray<Write['setter']>,
    values: fc.Arbitrary<unknown>
): Array<fc.Arbitrary<Write>> {
    return setters.map(setter => values.map((value): Write => ({ setter, value })));
}

/** Arrays of values, and typed arrays of the setter's element type */
function arrays<T>(
    values: fc.Arbitrary<T>,
    TypedArray: { from: (values: T[]) => unknown }
): fc.Arbitrary<unknown> {
    return fc
        .tuple(fc.array(values, { maxLength: 4 }), fc.boolean())
        .map(([array, typed]) => (typed ? TypedArray.from(array) : array));
}

const SCALAR_SIZES = {
    setInt8: 1,
    setUint8: 1,
    setInt16: 2,
    setUint16: 2,
    setInt32: 4,
    setUint32: 4,
    setFloat32: 4,
    setInt64: 8,
    setUint64: 8,
    setFloat64: 8
} as const;

const NUMBER_ARRAYS = [
    ['setInt8Array', Int8Array],
    ['setUint8Array', Uint8Array],
    ['setInt16Array', Int16Array],
    ['setUint16Array', Uint16Array],
    ['setInt32Array', Int32Array],
    ['setUint32Array', Uint32Array],
    ['setFloat32Array', Float32Array],
    ['setFloat64Array', Float64Array]
] as const;

const BIGINT_ARRAYS = [
    ['setInt64Array', BigInt64Array],
    ['setUint64Array', BigUint64Array]
] as const;

/** Writes of every setter but setString() and setVector() */
const WRITES = fc.oneof(
    ...writes(['setSize', 'setVersion', 'setOffset'], fc.nat({ max: 0xffff_ffff })),
    ...writes(
        ['setInt8', 'setUint8', 'setInt16', 'setUint16', 'setInt32', 'setUint32'],
        fc.integer({ min: -0x8000_0000, max: 0xffff_ffff })
    ),
    ...writes(['setFloat32', 'setFloat64'], fc.double()),
    ...writes(['setInt64', 'setUint64'], fc.bigInt({ min: -(2n ** 63n), max: 2n ** 64n - 1n })),
    ...NUMBER_ARRAYS.flatMap(([setter, TypedArray]) =>
        writes([setter], arrays(fc.double(), TypedArray))
    ),
    ...BIGINT_ARRAYS.flatMap(([setter, TypedArray]) =>
        writes([setter], arrays(fc.bigInt({ min: 0n, max: 2n ** 63n - 1n }), TypedArray))
    ),
    ...writes(['setBytes'], fc.uint8Array({ maxLength: 8 })),
    ...writes(['setPadding'], fc.nat({ max: 8 }))
);

/** Writes of a length or count and then the bytes after it */
const LENGTH_PREFIXED_WRITES = fc.oneof(
    ...writes(['setString'], fc.uint8Array({ maxLength: 8 })),
    ...writes(['setVector'], fc.array(fc.uint8Array({ maxLength: 4 }), { maxLength: 3 }))
);

describe('writes past the end of the bytes', () => {
    const lengths = fc.nat({ max: 24 });

    // The examples once wrote their length or count before throwing
    it.prop(
        [lengths, LAYOUTS, fc.array(fc.oneof(WRITES, LENGTH_PREFIXED_WRITES), { maxLength: 8 })],
        {
            examples: [
                [
                    1,
                    { sized: false, versioned: false, offsetSize: 1 },
                    [{ setter: 'setString', value: new Uint8Array(1) }]
                ],
                [
                    1,
                    { sized: false, versioned: false, offsetSize: 1 },
                    [{ setter: 'setVector', value: [new Uint8Array(0)] }]
                ]
            ]
        }
    )('should throw a RangeError and write nothing', expectWritesWithin);
});

/** An array setter, and more values than the array it counts */
const GROWN_ARRAYS = fc.oneof(
    ...NUMBER_ARRAYS.map(([setter]) =>
        fc.record({
            setter: fc.constant(setter),
            values: fc.array(fc.double(), { minLength: 1, maxLength: 8 })
        })
    ),
    ...BIGINT_ARRAYS.map(([setter]) =>
        fc.record({
            setter: fc.constant(setter),
            values: fc.array(fc.bigInt({ min: 0n, max: 2n ** 63n - 1n }), {
                minLength: 1,
                maxLength: 8
            })
        })
    )
);

describe('arrays that grow after they are counted', () => {
    // Room after the counted array often holds the grown one, so that the
    // count, rather than the bytes, must bound the write. The slice's offset
    // in its buffer covers aligned and unaligned arrays.
    it.prop([GROWN_ARRAYS, fc.nat(), fc.nat({ max: 64 }), SLICES])(
        'should throw a RangeError and write nothing in or past the bytes',
        ({ setter, values }, cut, room, slice) => {
            const counted = values.slice(0, cut % values.length);
            const length = 4 + counted.length * ELEMENT_SIZES[setter] + room;
            const bytes = sliceOf(new Uint8Array(length), slice);
            const buffer = new ByteBuffer(bytes);
            const before = new Uint8Array(bytes.buffer.slice(0));

            expect(() =>
                setArray(buffer, setter, growing<number | bigint>(values, counted.length))
            ).toThrow(RangeError);
            expect(new Uint8Array(bytes.buffer)).toStrictEqual(before);

            // The buffer is where it was, and writes the array as it was counted
            const expected = new Uint8Array(length);
            setArray(new ByteBuffer(expected), setter, counted);
            setArray(buffer, setter, counted);
            expect(bytes).toStrictEqual(expected);
        }
    );
});

function setArray(buffer: ByteBuffer, setter: Write['setter'], values: ArrayLike<unknown>): void {
    (buffer[setter] as (values: ArrayLike<unknown>) => void).call(buffer, values);
}

/**
 * Applies writes to a ByteBuffer of `length` bytes, at the start of a larger
 * buffer, and expects those that fit to write within the bytes, one after
 * another, and those that don't to throw a RangeError and write nothing,
 * which leaves the next write where it would have been
 */
function expectWritesWithin(length: number, layout: LayoutDescription, writes: Write[]): void {
    const backing = new Uint8Array(length + 8).fill(0xa5);
    const buffer = new ByteBuffer(backing.subarray(0, length), layoutOf(layout));
    const write = ({ setter, value }: Write): void =>
        (buffer[setter] as (value: unknown) => void).call(buffer, value);

    let position = 0;
    for (const next of writes) {
        const end = position + byteLengthOf(next, layout.offsetSize);

        if (end <= length) {
            write(next);
            position = end;
        } else {
            const before = backing.slice();
            expect(() => write(next)).toThrow(RangeError);
            expect(backing).toStrictEqual(before);
        }
    }

    expect(backing.subarray(length)).toStrictEqual(new Uint8Array(8).fill(0xa5));
}

/** The element size of each array setter */
const ELEMENT_SIZES: Readonly<Record<string, number>> = Object.fromEntries(
    [...NUMBER_ARRAYS, ...BIGINT_ARRAYS].map(([setter, TypedArray]) => [
        setter,
        TypedArray.BYTES_PER_ELEMENT
    ])
);

/** The number of bytes that a write writes, with lengths of `offsetSize` */
function byteLengthOf({ setter, value }: Write, offsetSize: number): number {
    if (setter in SCALAR_SIZES) {
        return SCALAR_SIZES[setter as keyof typeof SCALAR_SIZES];
    }

    if (setter in ELEMENT_SIZES) {
        return offsetSize + (value as ArrayLike<unknown>).length * ELEMENT_SIZES[setter];
    }

    switch (setter) {
        case 'setSize':
        case 'setVersion':
        case 'setOffset':
            return offsetSize;

        case 'setPadding':
            return value as number;

        case 'setBytes':
            return (value as Uint8Array).length;

        case 'setString':
            return offsetSize + (value as Uint8Array).length;

        case 'setVector':
            return (value as Uint8Array[]).reduce(
                (length, item) => length + offsetSize + item.length,
                offsetSize
            );

        default:
            throw new Error(`No byte length for ${setter}()`);
    }
}
