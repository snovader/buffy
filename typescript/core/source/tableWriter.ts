import { SIZEOF_8, SIZEOF_16, SIZEOF_32, SIZEOF_64 } from './constants.ts';
import { assertStructSize, structCount } from './byteBuffer.ts';
import {
    setFloat32Elements,
    setFloat64Elements,
    setInt8Elements,
    setInt16Elements,
    setInt32Elements,
    setInt64Elements,
    setUint8Elements,
    setUint16Elements,
    setUint32Elements,
    setUint64Elements
} from './elements.ts';
import { encodeInto, utf8Length } from './encoding.ts';
import { assertCount, headerLength, maxLength, TableLayout } from './layout.ts';
import type { AddressableSpace, ElementSize } from './types.ts';

/**
 * Counts the bytes of a table, so that a {@link TableWriter} can write it
 * into bytes of exactly that length. It's for code that knows a table's
 * fields before it writes them, such as the code that tafla-gen generates,
 * which writes tables without recording their fields as `TableBuilder` does.
 *
 * The fields are added in order, each with its type's method, which takes
 * what decides its length: nothing for a scalar, the count of an array, the
 * string itself. A nested table is added with `addTable()`, and a vector
 * with `addVector()`, and the fields of each such table then follow between
 * `startTable()` and `endTable()`. The sizer pads arrays and nested tables
 * as `TableBuilder` does, so a writer that takes the same fields in the same
 * order writes each where the sizer counted it.
 */
export class TableSizer {
    readonly #layout: TableLayout;
    readonly #version: number | undefined;
    readonly #fields: number;
    /** The width of offsets, lengths and header slots */
    readonly #size: AddressableSpace;
    /** The table's length so far, which is where the next field goes */
    #position: number;
    /** The start of the table being sized, which arrays are aligned from */
    #table = 0;

    /**
     * @param fields - The number of fields that the table holds.
     * @param layout - The table's header and offset width, which its readers
     * must use too.
     * @param version - The version that the writer writes to the header,
     * required when the layout is versioned.
     * @throws {TypeError} If `version` is given for a layout without a version
     * slot, or missing for one with it.
     * @throws {RangeError} If `fields` isn't a non-negative integer, `version`
     * isn't an integer that fits in the layout's offset size, or the header
     * alone is longer than the layout addresses.
     */
    constructor(fields: number, layout?: TableLayout<false>);
    constructor(fields: number, layout: TableLayout<true>, version: number);
    constructor(
        fields: number,
        layout: TableLayout = TableLayout.DEFAULT,
        version?: number
    ) {
        this.#layout = layout;
        this.#version = version;
        this.#fields = fields;
        this.#size = layout.offsetSize();
        this.#position = headerLength(fields, layout, version);
    }

    /**
     * The byte length of the table, with every field added
     *
     * @throws {RangeError} If the table is longer than its layout's offsets
     * address: 255 bytes, 64 KiB or 4 GiB.
     */
    byteLength(): number {
        const length = this.#position;
        if (!(length <= maxLength(this.#size))) {
            throw new RangeError(
                `The table's fields make it ${length} bytes, too long for ${this.#size}-byte offsets`
            );
        }

        return length;
    }

    /**
     * Returns a writer of the table, into new bytes of its length, which
     * takes its fields in the order that they were added
     *
     * @throws {RangeError} If the table is longer than its layout's offsets
     * address.
     */
    writer(): TableWriter {
        const bytes = new Uint8Array(this.byteLength());

        return this.#version === undefined
            ? new TableWriter(
                  bytes,
                  this.#fields,
                  this.#layout as TableLayout<false>
              )
            : new TableWriter(
                  bytes,
                  this.#fields,
                  this.#layout as TableLayout<true>,
                  this.#version
              );
    }

    /**
     * Starts the fields of a nested table or vector item of `fields` fields,
     * after its parent's `addTable()` or `addVector()`.
     *
     * @returns The parent's start, which `endTable()` takes back.
     * @throws {RangeError} If `fields` isn't a non-negative integer.
     */
    startTable(fields: number): number {
        assertCount(fields);

        const parent = this.#table;
        this.#table = this.#position;
        this.#position += fields * this.#size;

        return parent;
    }

    /** Ends the fields of the table that `startTable()` started */
    endTable(parent: number): void {
        this.#table = parent;
    }

    addInt8(): void {
        this.#position += SIZEOF_8;
    }

    addUint8(): void {
        this.#position += SIZEOF_8;
    }

    addInt16(): void {
        this.#position += SIZEOF_16;
    }

    addUint16(): void {
        this.#position += SIZEOF_16;
    }

    addInt32(): void {
        this.#position += SIZEOF_32;
    }

    addUint32(): void {
        this.#position += SIZEOF_32;
    }

    addInt64(): void {
        this.#position += SIZEOF_64;
    }

    addUint64(): void {
        this.#position += SIZEOF_64;
    }

    addFloat32(): void {
        this.#position += SIZEOF_32;
    }

    addFloat64(): void {
        this.#position += SIZEOF_64;
    }

    // The array adders take the count of the elements, and throw a RangeError
    // for one that isn't a non-negative integer

    addInt8Array(count: number): void {
        this.#addArray(count, SIZEOF_8, SIZEOF_8);
    }

    addUint8Array(count: number): void {
        this.#addArray(count, SIZEOF_8, SIZEOF_8);
    }

    addInt16Array(count: number): void {
        this.#addArray(count, SIZEOF_16, SIZEOF_16);
    }

    addUint16Array(count: number): void {
        this.#addArray(count, SIZEOF_16, SIZEOF_16);
    }

    addInt32Array(count: number): void {
        this.#addArray(count, SIZEOF_32, SIZEOF_32);
    }

    addUint32Array(count: number): void {
        this.#addArray(count, SIZEOF_32, SIZEOF_32);
    }

    addInt64Array(count: number): void {
        this.#addArray(count, SIZEOF_64, SIZEOF_64);
    }

    addUint64Array(count: number): void {
        this.#addArray(count, SIZEOF_64, SIZEOF_64);
    }

    addFloat32Array(count: number): void {
        this.#addArray(count, SIZEOF_32, SIZEOF_32);
    }

    addFloat64Array(count: number): void {
        this.#addArray(count, SIZEOF_64, SIZEOF_64);
    }

    /** Adds a string, as UTF-8 with lone surrogates replaced by U+FFFD */
    addString(string: string): void {
        this.#position += this.#size + utf8Length(string);
    }

    /**
     * Adds a struct of `size` bytes
     *
     * @throws {RangeError} If `size` isn't a positive integer.
     */
    addStruct(size: number): void {
        assertStructSize(size);
        this.#position += size;
    }

    /**
     * Adds `count` structs of `size` bytes, padded so that they start at a
     * multiple of `alignment` from the start of the table
     *
     * @throws {RangeError} If `count` isn't a non-negative integer, or `size`
     * a positive one.
     */
    addStructArray(count: number, size: number, alignment: ElementSize): void {
        assertStructSize(size);
        this.#addArray(count, size, alignment);
    }

    /**
     * Adds a nested table, padded to start at a multiple of `alignment`,
     * whose fields then follow between `startTable()` and `endTable()`
     */
    addTable(alignment: ElementSize): void {
        this.#position += padding(this.#position - this.#table, alignment);
    }

    /**
     * Adds a vector of `count` tables, whose fields then follow, each
     * between `startTable()` and `endTable()`
     *
     * @throws {RangeError} If `count` isn't a non-negative integer.
     */
    addVector(count: number): void {
        assertCount(count);
        this.#position += (count + 1) * this.#size;
    }

    /**
     * Adds an array of `count` elements of `size` bytes, padded so that they
     * are aligned after the count
     */
    #addArray(count: number, size: number, alignment: ElementSize): void {
        assertCount(count);

        const start = this.#position + this.#size;
        this.#position =
            start + padding(start - this.#table, alignment) + count * size;
    }
}

/**
 * Writes a table forward into bytes of exactly its length, which a
 * {@link TableSizer} counted, without recording its fields first as
 * `TableBuilder` does. Its writer comes from `TableSizer.writer()`, and takes
 * the fields that the sizer added, in the same order, each with its index in
 * its table.
 *
 * Each field's offset is written as the writer reaches the field. A nested
 * table follows its parent's `setTable()`, between `startTable()` and
 * `endTable()`; the items of a vector follow its `setVector()` the same way,
 * each followed by `setItemEnd()`, which writes where it ends. Arrays and
 * nested tables are padded as `TableBuilder` pads them, so the two write the
 * same bytes for the same fields.
 *
 * Counts that aren't non-negative integers throw a `RangeError`, before the
 * writer moves. The index of each field and the item of each end must be
 * their own: another writes the offset or end into another slot of the
 * bytes.
 *
 * Every write stays within the bytes, even when they're part of a larger
 * buffer. A field that doesn't fit in them, as when it's longer than the one
 * that the sizer counted, throws a `RangeError` and leaves the writer where
 * it was, without writing any of the field: its offset, its count or length,
 * or its values. Only a string, whose UTF-8 length is known once it's
 * written, has then written what fits of it after the position. An array is
 * written as long as its `length` was when the setter read it, and one that
 * has grown by the time its elements are written throws the same way.
 *
 * Typed arrays of a field's element type are copied as bytes; any other
 * array-like, such as a `number[]` or a typed array of another type, is
 * converted as the typed array's constructor would. Values that don't fit
 * their type are written as `DataView`'s setters write them, and a `number`
 * given for an `int64` throws a `TypeError`, as they do.
 */
export class TableWriter {
    // Scalars, offsets and lengths are written as bytes rather than through
    // a DataView. V8 keeps a Uint8Array of up to 64 bytes on its heap until
    // something asks for its ArrayBuffer, as a DataView does: in Node 26 that
    // took 140 ns of the 290 ns that writing a Point of 60 bytes took, and
    // writing it as bytes took 140 ns in all. A vector of 100 tables of
    // scalars took 5-15% longer, which arrays of structs write faster anyway.
    readonly #bytes: Uint8Array;
    /** The width of offsets, lengths and header slots */
    readonly #size: AddressableSpace;
    readonly #headerSize: number;
    /** Where the next field goes */
    #position: number;
    /** The start of the table being written, which its offsets count from */
    #table = 0;
    /** The position of that table's first offset, after the root's header */
    #slots: number;

    /**
     * Writes the header of a table of `fields` fields into `bytes`, which
     * must be as long as the table, as `TableSizer.writer()` makes them. They
     * needn't be zero: the writer writes every byte of the table, padding
     * included.
     *
     * @throws {TypeError} If `version` is given for a layout without a version
     * slot, or missing for one with it.
     * @throws {RangeError} If `fields` isn't a non-negative integer, `version`
     * doesn't fit in the layout's offset size, or `bytes` are longer than the
     * layout addresses.
     */
    constructor(bytes: Uint8Array, fields: number, layout?: TableLayout<false>);
    constructor(
        bytes: Uint8Array,
        fields: number,
        layout: TableLayout<true>,
        version: number
    );
    constructor(
        bytes: Uint8Array,
        fields: number,
        layout: TableLayout = TableLayout.DEFAULT,
        version?: number
    ) {
        const size = layout.offsetSize();
        const header = headerLength(fields, layout, version);
        if (!(bytes.length <= maxLength(size))) {
            throw new RangeError(
                `A table of ${bytes.length} bytes is too long for ${size}-byte offsets`
            );
        }

        this.#bytes = bytes;
        this.#size = size;
        this.#headerSize = layout.headerSize();
        this.#slots = this.#headerSize;
        this.#position = header;

        let slot = 0;
        if (layout.sized()) {
            this.#setLength(slot, bytes.length);
            slot += size;
        }

        if (version !== undefined) {
            this.#setLength(slot, version);
        }
    }

    /**
     * Returns the table's bytes.
     *
     * @throws {RangeError} If the fields didn't fill the bytes, as when they
     * differ from those that the sizer counted.
     */
    bytes(): Uint8Array {
        if (this.#position !== this.#bytes.length) {
            throw new RangeError(
                `The fields wrote ${this.#position} of the table's ${this.#bytes.length} bytes`
            );
        }

        return this.#bytes;
    }

    /**
     * Starts the fields of a nested table or vector item of `fields` fields,
     * after its parent's `setTable()` or `setVector()`.
     *
     * @returns The parent's start, which `endTable()` takes back.
     * @throws {RangeError} If `fields` isn't a non-negative integer.
     */
    startTable(fields: number): number {
        assertCount(fields);

        const parent = this.#table;
        this.#table = this.#position;
        this.#slots = this.#position;
        this.#position += fields * this.#size;

        return parent;
    }

    /** Ends the fields of the table that `startTable()` started */
    endTable(parent: number): void {
        this.#table = parent;
        // Only the root starts at 0, and only its offsets follow a header
        this.#slots = parent === 0 ? this.#headerSize : parent;
    }

    // A scalar is converted and copied into the bytes before its offset is
    // written, so that one that doesn't fit, or a number for an int64,
    // writes nothing

    setInt8(index: number, value: number): void {
        SCRATCH.setInt8(0, value);
        copy1(this.#bytes, this.#position);
        this.#setOffset(index);
        this.#position += SIZEOF_8;
    }

    setUint8(index: number, value: number): void {
        SCRATCH.setUint8(0, value);
        copy1(this.#bytes, this.#position);
        this.#setOffset(index);
        this.#position += SIZEOF_8;
    }

    setInt16(index: number, value: number): void {
        SCRATCH.setInt16(0, value, true);
        copy2(this.#bytes, this.#position);
        this.#setOffset(index);
        this.#position += SIZEOF_16;
    }

    setUint16(index: number, value: number): void {
        SCRATCH.setUint16(0, value, true);
        copy2(this.#bytes, this.#position);
        this.#setOffset(index);
        this.#position += SIZEOF_16;
    }

    setInt32(index: number, value: number): void {
        SCRATCH.setInt32(0, value, true);
        copy4(this.#bytes, this.#position);
        this.#setOffset(index);
        this.#position += SIZEOF_32;
    }

    setUint32(index: number, value: number): void {
        SCRATCH.setUint32(0, value, true);
        copy4(this.#bytes, this.#position);
        this.#setOffset(index);
        this.#position += SIZEOF_32;
    }

    setInt64(index: number, value: bigint): void {
        SCRATCH.setBigInt64(0, value, true);
        copy8(this.#bytes, this.#position);
        this.#setOffset(index);
        this.#position += SIZEOF_64;
    }

    setUint64(index: number, value: bigint): void {
        SCRATCH.setBigUint64(0, value, true);
        copy8(this.#bytes, this.#position);
        this.#setOffset(index);
        this.#position += SIZEOF_64;
    }

    setFloat32(index: number, value: number): void {
        SCRATCH.setFloat32(0, value, true);
        copy4(this.#bytes, this.#position);
        this.#setOffset(index);
        this.#position += SIZEOF_32;
    }

    setFloat64(index: number, value: number): void {
        SCRATCH.setFloat64(0, value, true);
        copy8(this.#bytes, this.#position);
        this.#setOffset(index);
        this.#position += SIZEOF_64;
    }

    setInt8Array(index: number, values: ArrayLike<number> | Int8Array): void {
        const count = values.length;
        const elements = this.#arrayElements(count, SIZEOF_8, SIZEOF_8);

        setInt8Elements(this.#bytes, elements, values, count);
        this.#endArray(index, count, elements, count * SIZEOF_8);
    }

    setUint8Array(index: number, values: ArrayLike<number> | Uint8Array): void {
        const count = values.length;
        const elements = this.#arrayElements(count, SIZEOF_8, SIZEOF_8);

        setUint8Elements(this.#bytes, elements, values, count);
        this.#endArray(index, count, elements, count * SIZEOF_8);
    }

    setInt16Array(index: number, values: ArrayLike<number> | Int16Array): void {
        const count = values.length;
        const elements = this.#arrayElements(count, SIZEOF_16, SIZEOF_16);

        setInt16Elements(this.#bytes, elements, values, count);
        this.#endArray(index, count, elements, count * SIZEOF_16);
    }

    setUint16Array(
        index: number,
        values: ArrayLike<number> | Uint16Array
    ): void {
        const count = values.length;
        const elements = this.#arrayElements(count, SIZEOF_16, SIZEOF_16);

        setUint16Elements(this.#bytes, elements, values, count);
        this.#endArray(index, count, elements, count * SIZEOF_16);
    }

    setInt32Array(index: number, values: ArrayLike<number> | Int32Array): void {
        const count = values.length;
        const elements = this.#arrayElements(count, SIZEOF_32, SIZEOF_32);

        setInt32Elements(this.#bytes, elements, values, count);
        this.#endArray(index, count, elements, count * SIZEOF_32);
    }

    setUint32Array(
        index: number,
        values: ArrayLike<number> | Uint32Array
    ): void {
        const count = values.length;
        const elements = this.#arrayElements(count, SIZEOF_32, SIZEOF_32);

        setUint32Elements(this.#bytes, elements, values, count);
        this.#endArray(index, count, elements, count * SIZEOF_32);
    }

    setInt64Array(
        index: number,
        values: ArrayLike<bigint> | BigInt64Array
    ): void {
        const count = values.length;
        const elements = this.#arrayElements(count, SIZEOF_64, SIZEOF_64);

        setInt64Elements(this.#bytes, elements, values, count);
        this.#endArray(index, count, elements, count * SIZEOF_64);
    }

    setUint64Array(
        index: number,
        values: ArrayLike<bigint> | BigUint64Array
    ): void {
        const count = values.length;
        const elements = this.#arrayElements(count, SIZEOF_64, SIZEOF_64);

        setUint64Elements(this.#bytes, elements, values, count);
        this.#endArray(index, count, elements, count * SIZEOF_64);
    }

    setFloat32Array(
        index: number,
        values: ArrayLike<number> | Float32Array
    ): void {
        const count = values.length;
        const elements = this.#arrayElements(count, SIZEOF_32, SIZEOF_32);

        setFloat32Elements(this.#bytes, elements, values, count);
        this.#endArray(index, count, elements, count * SIZEOF_32);
    }

    setFloat64Array(
        index: number,
        values: ArrayLike<number> | Float64Array
    ): void {
        const count = values.length;
        const elements = this.#arrayElements(count, SIZEOF_64, SIZEOF_64);

        setFloat64Elements(this.#bytes, elements, values, count);
        this.#endArray(index, count, elements, count * SIZEOF_64);
    }

    /** Writes `string` as UTF-8, with lone surrogates replaced by U+FFFD */
    setString(index: number, string: string): void {
        // Its UTF-8 length is known once it's written, so a string that
        // doesn't fit throws before its offset and length are written
        const start = this.#position + this.#size;
        assertFits(this.#bytes, start);
        const length = encodeInto(string, this.#bytes, start);
        this.#setOffset(index);
        this.#setLength(this.#position, length);
        this.#position = start + length;
    }

    /**
     * Writes a struct field from its bytes: its scalars, little-endian, one
     * after another, without padding
     */
    setStruct(index: number, bytes: Uint8Array): void {
        assertFits(this.#bytes, this.#position + bytes.length);
        this.#setOffset(index);

        this.#bytes.set(bytes, this.#position);
        this.#position += bytes.length;
    }

    /**
     * Writes an array of structs of `size` bytes each, from their bytes one
     * after another, padded so that they start at a multiple of `alignment`
     * from the start of the table
     *
     * @throws {RangeError} If `size` isn't a positive integer, or `bytes`
     * aren't a whole number of structs.
     */
    setStructArray(
        index: number,
        bytes: Uint8Array,
        size: number,
        alignment: ElementSize
    ): void {
        const count = structCount(bytes.length, size);
        const elements = this.#arrayElements(count, size, alignment);

        this.#bytes.set(bytes, elements);
        this.#endArray(index, count, elements, bytes.length);
    }

    /**
     * Writes the offset of a nested table, padded to start at a multiple of
     * `alignment`, whose fields then follow between `startTable()` and
     * `endTable()`
     */
    setTable(index: number, alignment: ElementSize): void {
        this.#pad(padding(this.#position - this.#table, alignment));
        this.#setOffset(index);
    }

    /**
     * Writes the offset and count of a vector of `count` tables, whose items
     * then follow, each between `startTable()` and `endTable()` and then
     * `setItemEnd()`.
     *
     * @returns The vector's position, which `setItemEnd()` takes.
     * @throws {RangeError} If `count` isn't a non-negative integer.
     */
    setVector(index: number, count: number): number {
        assertCount(count);
        assertFits(this.#bytes, this.#position + (count + 1) * this.#size);
        this.#setOffset(index);

        const vector = this.#position;
        this.#setLength(vector, count);
        this.#position += (count + 1) * this.#size;

        return vector;
    }

    /** Writes the end of item `item` of the vector, which was just written */
    setItemEnd(vector: number, item: number): void {
        this.#setLength(
            vector + (item + 1) * this.#size,
            this.#position - vector
        );
    }

    /**
     * Where an array's elements go, after the padding that aligns them and
     * its count, once its count and its elements fit. It writes nothing, so
     * that elements that throw leave the writer where it was.
     */
    #arrayElements(
        count: number,
        elementSize: number,
        alignment: ElementSize
    ): number {
        const start = this.#position + this.#size;
        const elements = start + padding(start - this.#table, alignment);
        assertFits(this.#bytes, elements + count * elementSize);

        return elements;
    }

    /**
     * Writes the offset and count of an array whose elements were written
     * at `elements`, and moves past them
     */
    #endArray(
        index: number,
        count: number,
        elements: number,
        byteLength: number
    ): void {
        this.#pad(elements - this.#size - this.#position);
        this.#setOffset(index);
        this.#setLength(this.#position, count);
        this.#position = elements + byteLength;
    }

    /**
     * Writes `length` bytes of zero padding at the position, and moves past
     * them. The bytes needn't be new, so their padding is written rather
     * than left as it was.
     */
    #pad(length: number): void {
        const position = this.#position;
        this.#bytes.fill(0, position, position + length);
        this.#position = position + length;
    }

    #setOffset(index: number): void {
        this.#setLength(
            this.#slots + index * this.#size,
            this.#position - this.#table
        );
    }

    /**
     * Writes an offset, length, count or header slot at `position`,
     * little-endian. A value too large for the width is wrapped, as
     * DataView's setters wrap it; the sizer rejects a table long enough to
     * need one.
     */
    #setLength(position: number, value: number): void {
        const bytes = this.#bytes;
        bytes[position] = value;

        if (this.#size !== SIZEOF_8) {
            bytes[position + 1] = value >>> 8;

            if (this.#size === SIZEOF_32) {
                bytes[position + 2] = value >>> 16;
                bytes[position + 3] = value >>> 24;
            }
        }
    }
}

// A scalar is set little-endian in the scratch, whatever the host's byte
// order, and its bytes copied into the table once they fit. A function per
// width keeps each small enough for V8 to inline into the setters, and
// each checks inline and builds its error out of line to stay so.
const SCRATCH = new DataView(new ArrayBuffer(SIZEOF_64));
const SCRATCH_BYTES = new Uint8Array(SCRATCH.buffer);

function copy1(bytes: Uint8Array, position: number): void {
    if (!(position + SIZEOF_8 <= bytes.length)) {
        throw tooLong(bytes, position + SIZEOF_8);
    }

    bytes[position] = SCRATCH_BYTES[0];
}

function copy2(bytes: Uint8Array, position: number): void {
    if (!(position + SIZEOF_16 <= bytes.length)) {
        throw tooLong(bytes, position + SIZEOF_16);
    }

    bytes[position] = SCRATCH_BYTES[0];
    bytes[position + 1] = SCRATCH_BYTES[1];
}

function copy4(bytes: Uint8Array, position: number): void {
    if (!(position + SIZEOF_32 <= bytes.length)) {
        throw tooLong(bytes, position + SIZEOF_32);
    }

    bytes[position] = SCRATCH_BYTES[0];
    bytes[position + 1] = SCRATCH_BYTES[1];
    bytes[position + 2] = SCRATCH_BYTES[2];
    bytes[position + 3] = SCRATCH_BYTES[3];
}

function copy8(bytes: Uint8Array, position: number): void {
    if (!(position + SIZEOF_64 <= bytes.length)) {
        throw tooLong(bytes, position + SIZEOF_64);
    }

    bytes[position] = SCRATCH_BYTES[0];
    bytes[position + 1] = SCRATCH_BYTES[1];
    bytes[position + 2] = SCRATCH_BYTES[2];
    bytes[position + 3] = SCRATCH_BYTES[3];
    bytes[position + 4] = SCRATCH_BYTES[4];
    bytes[position + 5] = SCRATCH_BYTES[5];
    bytes[position + 6] = SCRATCH_BYTES[6];
    bytes[position + 7] = SCRATCH_BYTES[7];
}

/**
 * Checks that the table's bytes reach `end`, before a field writes up to it.
 * An array's elements are written through a view of the bytes' buffer, which
 * would write past bytes that are only part of it.
 *
 * @throws {RangeError} If they don't.
 */
function assertFits(bytes: Uint8Array, end: number): void {
    if (!(end <= bytes.length)) {
        throw tooLong(bytes, end);
    }
}

function tooLong(bytes: Uint8Array, end: number): RangeError {
    return new RangeError(
        `The fields need ${end} bytes, more than the table's ${bytes.length}`
    );
}

/** The bytes that bring `offset` to a multiple of `alignment` */
function padding(offset: number, alignment: ElementSize): number {
    const remainder = offset % alignment;
    return remainder === 0 ? 0 : alignment - remainder;
}
