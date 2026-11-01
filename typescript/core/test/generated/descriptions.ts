import {
    type FieldDescription,
    type JsonNumber,
    type LayoutDescription,
    type ScalarType,
    type TableDescription,
    toJsonNumber
} from '../conformance/description.ts';

// The fields of generated messages and tables in the conformance suite's
// model, so that `buildTable()` writes the same table by hand with
// `TableBuilder`, as the suite builds its tables, for tests to compare with
// the bytes that the generated code writes.

/** The layout of tables and of messages without attributes */
export const DEFAULT_LAYOUT: LayoutDescription = {
    sized: false,
    versioned: false,
    offsetSize: 4
};

export function scalar(
    type: ScalarType,
    value: number | bigint
): FieldDescription {
    return { type, value: jsonOf(value) };
}

export function array(
    type: ScalarType,
    values: ArrayLike<number> | ArrayLike<bigint>
): FieldDescription {
    return { type: `${type}[]`, value: Array.from(values, jsonOf) };
}

export function string(value: string): FieldDescription {
    return { type: 'string', value };
}

/** A table nested from its builder at its default alignment, as generated code nests it */
export function table(fields: FieldDescription[]): FieldDescription {
    return {
        type: 'table',
        from: 'builder',
        table: { layout: DEFAULT_LAYOUT, fields }
    };
}

/** A vector of tables from their builders, as generated code writes it */
export function vector(items: readonly FieldDescription[][]): FieldDescription {
    return {
        type: 'vector',
        items: items.map(fields => ({
            from: 'builder',
            table: { layout: DEFAULT_LAYOUT, fields }
        }))
    };
}

/** A message's table, with its version when its layout is versioned */
export function message(
    layout: LayoutDescription,
    fields: FieldDescription[],
    version?: number
): TableDescription {
    return version === undefined
        ? { layout, fields }
        : { layout, version, fields };
}

function jsonOf(value: number | bigint): JsonNumber {
    return typeof value === 'bigint' ? String(value) : toJsonNumber(value);
}
