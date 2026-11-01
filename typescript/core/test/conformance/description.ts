// The description of the conformance suite in `cases.json`, which every
// implementation loads to read and rebuild the tables. README.md explains
// the format.

export type ScalarType =
    | 'int8'
    | 'uint8'
    | 'int16'
    | 'uint16'
    | 'int32'
    | 'uint32'
    | 'int64'
    | 'uint64'
    | 'float32'
    | 'float64';

export type ArrayType = `${ScalarType}[]`;

/**
 * A value as JSON holds it: a number, a 64-bit integer as a decimal string,
 * since JSON numbers lose precision past 2^53 in JavaScript, or one of the
 * floats that JSON has no number for: "NaN", "Infinity", "-Infinity" or "-0".
 */
export type JsonNumber = number | string;

export type OffsetSize = 1 | 2 | 4;

export type Alignment = 1 | 2 | 4 | 8;

export interface LayoutDescription {
    sized: boolean;
    versioned: boolean;
    offsetSize: OffsetSize;
}

export interface TableDescription {
    layout: LayoutDescription;
    /** The version in the header, present when the layout is versioned */
    version?: number;
    fields: FieldDescription[];
}

/** A table nested from its builder or from the bytes that its builder writes */
export interface NestedTable {
    from: 'builder' | 'bytes';
    table: TableDescription;
}

export interface TableField extends NestedTable {
    type: 'table';
    /** The alignment passed to `setTable()`, absent for its default */
    alignment?: Alignment;
}

export type FieldDescription =
    | { type: ScalarType; value: JsonNumber }
    | { type: ArrayType; value: JsonNumber[] }
    | { type: 'string'; value: string }
    | TableField
    | { type: 'vector'; items: NestedTable[] };

export interface ValidCase {
    name: string;
    description: string;
    /** The table's bytes, relative to `cases.json` */
    file: string;
    table: TableDescription;
}

/** How an invalid case's bytes differ from its valid case's */
export type Corruption =
    | { kind: 'truncate'; length: number }
    | { kind: 'overwrite'; at: number; bytes: number[] };

/**
 * Bytes that read the fields of a valid case before `field` as it describes
 * them, and then throw a decode error reading `field`
 */
export interface InvalidCase {
    name: string;
    description: string;
    file: string;
    /** The name of the valid case whose table description reads the bytes */
    case: string;
    corruption: Corruption;
    field: number;
}

export interface Suite {
    valid: ValidCase[];
    invalid: InvalidCase[];
}

/**
 * Returns a number as JSON holds it, with the floats that JSON has no number
 * for as their names
 */
export function toJsonNumber(value: number): JsonNumber {
    if (Number.isNaN(value)) {
        return 'NaN';
    }

    if (value === Number.POSITIVE_INFINITY) {
        return 'Infinity';
    }

    if (value === Number.NEGATIVE_INFINITY) {
        return '-Infinity';
    }

    return Object.is(value, -0) ? '-0' : value;
}

/**
 * Returns a number that JSON holds as a number or the name of a float.
 *
 * @throws {TypeError} If `value` is any other string, such as a 64-bit
 * integer's.
 */
export function fromJsonNumber(value: JsonNumber): number {
    switch (value) {
        case 'NaN':
            return Number.NaN;

        case 'Infinity':
            return Number.POSITIVE_INFINITY;

        case '-Infinity':
            return Number.NEGATIVE_INFINITY;

        case '-0':
            return -0;
    }

    if (typeof value !== 'number') {
        throw new TypeError(`Not a number: ${value}`);
    }

    return value;
}
