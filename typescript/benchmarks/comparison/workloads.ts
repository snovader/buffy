/**
 * Generic table shapes, sized to bracket how tafla is used: small tables of
 * scalars and a string, documents whose header nests a table of numeric
 * columns, vectors of small nested tables, and fixed-size records.
 */

/** A small table: two float64s, an int64 and a 16-byte string */
export interface RecordInput {
    x: number;
    y: number;
    id: bigint;
    name: string;
}

/**
 * A document: a sized, versioned table of two float64s and a string, with a
 * nested table of an int64 and three columns of the same length.
 */
export interface DocumentInput<
    U extends ArrayLike<number> = ArrayLike<number>,
    F extends ArrayLike<number> = ArrayLike<number>
> {
    x: number;
    y: number;
    name: string;
    base: bigint;
    keys: U;
    a: F;
    b: F;
}

export interface DocumentOutput {
    x: number;
    y: number;
    name: string;
    base: bigint;
    keys: Uint32Array;
    a: Float32Array;
    b: Float32Array;
}

/** A table of a float32 and a nested table of two float32s */
export interface ItemInput {
    value: number;
    first: number;
    second: number;
}

/** A table holding a vector of item tables */
export interface ListInput {
    items: ItemInput[];
}

/** A fixed-size record of a uint32 and two float32s */
export interface SampleInput {
    key: number;
    a: number;
    b: number;
}

/** Rows per document: a small one and a large one */
export const DOCUMENT_ROWS = [100, 100_000] as const;

export const LIST_ITEMS = 100;

export const RECORD: RecordInput = {
    x: 59.3293,
    y: 18.0686,
    id: -631_152_000n,
    name: 'tafla-benchmarks'
};

export const SAMPLE: SampleInput = {
    key: 2_743_826_400,
    a: Math.fround(21.75),
    b: Math.fround(3.25)
};

export function documentInput(
    rows: number
): DocumentInput<Uint32Array, Float32Array> {
    const random = xorshift(rows);

    // Keys below 2^24 take 4 bytes in protobuf's varints, like the floats
    const keys = new Uint32Array(rows);
    const a = new Float32Array(rows);
    const b = new Float32Array(rows);
    for (let i = 0; i < rows; i++) {
        keys[i] = Math.floor(random() * 2 ** 24);
        a[i] = (random() - 0.5) * 100;
        b[i] = (random() - 0.5) * 100;
    }

    return { ...RECORD, base: RECORD.id, keys, a, b };
}

/** The same document with `number[]` columns, as parsed JSON would have */
export function documentNumbers(
    input: DocumentInput
): DocumentInput<number[], number[]> {
    return {
        ...input,
        keys: Array.from(input.keys),
        a: Array.from(input.a),
        b: Array.from(input.b)
    };
}

export function listInput(items: number): ListInput {
    const random = xorshift(items);

    // Float32 values, so that every format reads them back exactly
    return {
        items: Array.from({ length: items }, () => ({
            value: Math.fround(random() * 30),
            first: Math.fround(random() * 4 - 2),
            second: Math.fround(random() * 50)
        }))
    };
}

/** A deterministic generator in [0, 1), so every run sees the same values */
function xorshift(seed: number): () => number {
    let state = 0x2545f491 ^ seed;

    return () => {
        state ^= state << 13;
        state ^= state >>> 17;
        state ^= state << 5;

        return (state >>> 0) / 2 ** 32;
    };
}
