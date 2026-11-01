import { ByteBuffer, TableBuilder, TableLayout } from '@buffy/core';
import {
    type FieldDescription,
    fromJsonNumber,
    type JsonNumber,
    type LayoutDescription,
    type NestedTable,
    type TableDescription,
    toJsonNumber
} from './description.ts';

export function layoutOf({
    sized,
    versioned,
    offsetSize
}: LayoutDescription): TableLayout {
    return TableLayout.create({ sized, versioned, offsetSize });
}

/** Builds a table as its description sets it out */
export function buildTable(table: TableDescription): TableBuilder {
    const builder = createBuilder(table);
    for (const field of table.fields) {
        setField(builder, field);
    }

    return builder;
}

function createBuilder({
    layout,
    version,
    fields
}: TableDescription): TableBuilder {
    const { sized, offsetSize } = layout;

    if (layout.versioned) {
        return new TableBuilder(
            fields.length,
            TableLayout.create({ sized, versioned: true, offsetSize }),
            version as number
        );
    }

    return new TableBuilder(
        fields.length,
        TableLayout.create({ sized, offsetSize })
    );
}

function setField(builder: TableBuilder, field: FieldDescription): void {
    switch (field.type) {
        case 'int8':
            builder.setInt8(fromJsonNumber(field.value));
            break;

        case 'uint8':
            builder.setUint8(fromJsonNumber(field.value));
            break;

        case 'int16':
            builder.setInt16(fromJsonNumber(field.value));
            break;

        case 'uint16':
            builder.setUint16(fromJsonNumber(field.value));
            break;

        case 'int32':
            builder.setInt32(fromJsonNumber(field.value));
            break;

        case 'uint32':
            builder.setUint32(fromJsonNumber(field.value));
            break;

        case 'int64':
            builder.setInt64(BigInt(field.value));
            break;

        case 'uint64':
            builder.setUint64(BigInt(field.value));
            break;

        case 'float32':
            builder.setFloat32(fromJsonNumber(field.value));
            break;

        case 'float64':
            builder.setFloat64(fromJsonNumber(field.value));
            break;

        case 'int8[]':
            builder.setInt8Array(field.value.map(fromJsonNumber));
            break;

        case 'uint8[]':
            builder.setUint8Array(field.value.map(fromJsonNumber));
            break;

        case 'int16[]':
            builder.setInt16Array(field.value.map(fromJsonNumber));
            break;

        case 'uint16[]':
            builder.setUint16Array(field.value.map(fromJsonNumber));
            break;

        case 'int32[]':
            builder.setInt32Array(field.value.map(fromJsonNumber));
            break;

        case 'uint32[]':
            builder.setUint32Array(field.value.map(fromJsonNumber));
            break;

        case 'int64[]':
            builder.setInt64Array(field.value.map(toBigInt));
            break;

        case 'uint64[]':
            builder.setUint64Array(field.value.map(toBigInt));
            break;

        case 'float32[]':
            builder.setFloat32Array(field.value.map(fromJsonNumber));
            break;

        case 'float64[]':
            builder.setFloat64Array(field.value.map(fromJsonNumber));
            break;

        case 'string':
            builder.setString(field.value);
            break;

        case 'table':
            builder.setTable(buildNested(field), field.alignment);
            break;

        case 'vector':
            builder.setVector(field.items.map(buildNested));
            break;
    }
}

function buildNested({ from, table }: NestedTable): TableBuilder | Uint8Array {
    const builder = buildTable(table);
    return from === 'builder' ? builder : builder.bytes();
}

function toBigInt(value: JsonNumber): bigint {
    return BigInt(value);
}

/**
 * Reads the bytes of a table as descriptions of its fields set them out, and
 * returns those descriptions with the values that were read in place of
 * theirs, so that they equal the descriptions when the bytes hold the values.
 *
 * A nested table, from `setTable()` or in a vector, is read in place when it
 * has no header and its offsets are as wide as its parent's. Any other is read
 * as a table of its own: a vector item from its bytes, and a field from its
 * position to the end of its parent.
 */
export class TableReader {
    readonly #buffer: ByteBuffer;
    readonly #layout: LayoutDescription;

    constructor(bytes: Uint8Array, layout: LayoutDescription) {
        this.#buffer = new ByteBuffer(bytes, layoutOf(layout));
        this.#layout = layout;
    }

    /**
     * Reads the header and every field of the table, which must have the
     * reader's layout.
     *
     * @throws {Error} If the size slot doesn't hold the table's length.
     */
    readTable(table: TableDescription): TableDescription {
        const buffer = this.#buffer;
        const { layout } = table;

        if (layout.sized && buffer.getSize() !== buffer.bytes().length) {
            throw new Error(
                `The size slot holds ${buffer.getSize()}, but the table has ${buffer.bytes().length} bytes`
            );
        }

        const version = layout.versioned ? buffer.getVersion() : undefined;
        const fields = table.fields.map((field, index) =>
            this.readRootField(index, field)
        );

        return version === undefined
            ? { layout, fields }
            : { layout, version, fields };
    }

    readRootField(index: number, field: FieldDescription): FieldDescription {
        return this.#readField(this.#buffer.rootOffset(index), field);
    }

    #readField(position: number, field: FieldDescription): FieldDescription {
        const buffer = this.#buffer;

        switch (field.type) {
            case 'int8':
                return { ...field, value: buffer.getInt8(position) };

            case 'uint8':
                return { ...field, value: buffer.getUint8(position) };

            case 'int16':
                return { ...field, value: buffer.getInt16(position) };

            case 'uint16':
                return { ...field, value: buffer.getUint16(position) };

            case 'int32':
                return { ...field, value: buffer.getInt32(position) };

            case 'uint32':
                return { ...field, value: buffer.getUint32(position) };

            case 'int64':
                return { ...field, value: String(buffer.getInt64(position)) };

            case 'uint64':
                return { ...field, value: String(buffer.getUint64(position)) };

            case 'float32':
                return {
                    ...field,
                    value: toJsonNumber(buffer.getFloat32(position))
                };

            case 'float64':
                return {
                    ...field,
                    value: toJsonNumber(buffer.getFloat64(position))
                };

            case 'int8[]':
                return {
                    ...field,
                    value: Array.from(buffer.getInt8Array(position))
                };

            case 'uint8[]':
                return {
                    ...field,
                    value: Array.from(buffer.getUint8Array(position))
                };

            case 'int16[]':
                return {
                    ...field,
                    value: Array.from(buffer.getInt16Array(position))
                };

            case 'uint16[]':
                return {
                    ...field,
                    value: Array.from(buffer.getUint16Array(position))
                };

            case 'int32[]':
                return {
                    ...field,
                    value: Array.from(buffer.getInt32Array(position))
                };

            case 'uint32[]':
                return {
                    ...field,
                    value: Array.from(buffer.getUint32Array(position))
                };

            case 'int64[]':
                return {
                    ...field,
                    value: Array.from(buffer.getInt64Array(position), String)
                };

            case 'uint64[]':
                return {
                    ...field,
                    value: Array.from(buffer.getUint64Array(position), String)
                };

            case 'float32[]':
                return {
                    ...field,
                    value: Array.from(
                        buffer.getFloat32Array(position),
                        toJsonNumber
                    )
                };

            case 'float64[]':
                return {
                    ...field,
                    value: Array.from(
                        buffer.getFloat64Array(position),
                        toJsonNumber
                    )
                };

            case 'string':
                return { ...field, value: buffer.getString(position) };

            case 'table':
                return {
                    ...field,
                    table: this.#readNested(position, field.table)
                };

            case 'vector':
                return {
                    ...field,
                    items: this.#readVector(position, field.items)
                };
        }
    }

    /** @throws {Error} If the vector holds another number of items. */
    #readVector(position: number, items: NestedTable[]): NestedTable[] {
        const offsets = this.#buffer.getVectorOffsets(position);
        if (offsets.length !== items.length) {
            throw new Error(
                `The vector holds ${offsets.length} items rather than ${items.length}`
            );
        }

        const vector = this.#buffer.getVector(position);

        return items.map((item, index) => ({
            ...item,
            table: this.#readsInPlace(item.table)
                ? this.#readInPlace(offsets[index], item.table)
                : readTable(vector[index], item.table)
        }));
    }

    /** Reads a table nested with `setTable()` */
    #readNested(position: number, table: TableDescription): TableDescription {
        if (this.#readsInPlace(table)) {
            return this.#readInPlace(position, table);
        }

        // Its length isn't written, so it's read up to the end of its parent
        return readTable(this.#buffer.bytes().subarray(position), table);
    }

    #readsInPlace({ layout }: TableDescription): boolean {
        return (
            !layout.sized &&
            !layout.versioned &&
            layout.offsetSize === this.#layout.offsetSize
        );
    }

    #readInPlace(position: number, table: TableDescription): TableDescription {
        const buffer = this.#buffer;

        return {
            layout: table.layout,
            fields: table.fields.map((field, index) =>
                this.#readField(buffer.fieldOffset(position, index), field)
            )
        };
    }
}

/** Reads a table from its own bytes, as {@link TableReader.readTable} does */
export function readTable(
    bytes: Uint8Array,
    table: TableDescription
): TableDescription {
    return new TableReader(bytes, table.layout).readTable(table);
}
