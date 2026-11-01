import {
    IS_LITTLE_ENDIAN,
    SIZEOF_8,
    SIZEOF_16,
    SIZEOF_32,
    SIZEOF_64
} from './constants.ts';
import { toUint8Array } from './converters.ts';
import { decode } from './encoding.ts';
import { byteOrderSwap, byteOrderSwapped } from './endianness.ts';
import { DecodeError, outOfRange } from './errors.ts';
import { TableLayout } from './layout.ts';
import type { AddressableSpace, ElementSize, TypedArray } from './types.ts';

/**
 * Reads and writes within the supplied byte slice, in the table layout it was
 * created with, storing multi-byte values little-endian. Array reads may share
 * the slice's storage when aligned; unaligned reads return a copy. On a
 * big-endian host, arrays of multi-byte elements are always copies, converted
 * to the host's byte order.
 *
 * Every read checks that it stays within the slice, even when the backing
 * allocation has more bytes, and throws a {@link DecodeError} otherwise.
 * Writes past the slice throw a `RangeError` and write nothing. Lengths,
 * offsets and versions are written like DataView's setters, which wrap a value
 * too large for the layout's `offsetSize`; `TableBuilder` rejects those before
 * it writes a table.
 */
export class ByteBuffer {
    readonly #bytes: Uint8Array;
    readonly #view: DataView;

    readonly #sized: boolean;
    readonly #versioned: boolean;
    /** The width of offsets, lengths and header slots */
    readonly #size: AddressableSpace;
    /** The byte length of the header slots before the root's field offsets */
    readonly #headerSize: number;

    #position = 0;

    constructor(bytes: Uint8Array, layout: TableLayout = TableLayout.DEFAULT) {
        this.#bytes = bytes;
        this.#view = new DataView(
            bytes.buffer,
            bytes.byteOffset,
            bytes.byteLength
        );

        this.#sized = layout.sized();
        this.#versioned = layout.versioned();
        this.#size = layout.offsetSize();
        this.#headerSize = layout.headerSize();
    }

    reset(): void {
        this.#position = 0;
    }

    bytes(): Uint8Array {
        return this.#bytes;
    }

    getSize(): number {
        return this.#sized ? this.#getSize(0) : this.#bytes.length;
    }

    setSize(size: number): void {
        this.#setSize(size);
    }

    /**
     * Returns the version that the table's header carries after its length.
     * It's as wide as the header's other slots.
     *
     * @throws {TypeError} If the buffer's layout isn't versioned.
     */
    getVersion(): number {
        if (!this.#versioned) {
            throw new TypeError('The table has no version');
        }

        return this.#getSize(this.#sized ? this.#size : 0);
    }

    setVersion(version: number): void {
        this.#setSize(version);
    }

    /** Returns the position of field `index` of the root table */
    rootOffset(index: number): number {
        return this.#getSize(this.#headerSize + index * this.#size);
    }

    /**
     * Returns the position of field `index` of a nested table, which has no
     * header and stores its field offsets from its own start.
     *
     * @param tablePosition - The nested table's position, which is the
     * offset of the field that holds it.
     */
    fieldOffset(tablePosition: number, index: number): number {
        return (
            tablePosition + this.#getSize(tablePosition + index * this.#size)
        );
    }

    setOffset(offset: number): void {
        this.#setSize(offset);
    }

    // The scalar getters let the DataView check the range, which is free,
    // and rethrow its error. A check of their own made reading 27,028 floats
    // one at a time 2.7 times slower in d8 and Node.

    getInt8(position: number): number {
        try {
            return this.#view.getInt8(position);
        } catch {
            throw outOfRange(position, SIZEOF_8, this.#view.byteLength);
        }
    }

    getUint8(position: number): number {
        try {
            return this.#view.getUint8(position);
        } catch {
            throw outOfRange(position, SIZEOF_8, this.#view.byteLength);
        }
    }

    getInt16(position: number): number {
        try {
            return this.#view.getInt16(position, true);
        } catch {
            throw outOfRange(position, SIZEOF_16, this.#view.byteLength);
        }
    }

    getUint16(position: number): number {
        try {
            return this.#view.getUint16(position, true);
        } catch {
            throw outOfRange(position, SIZEOF_16, this.#view.byteLength);
        }
    }

    getInt32(position: number): number {
        try {
            return this.#view.getInt32(position, true);
        } catch {
            throw outOfRange(position, SIZEOF_32, this.#view.byteLength);
        }
    }

    getUint32(position: number): number {
        try {
            return this.#view.getUint32(position, true);
        } catch {
            throw outOfRange(position, SIZEOF_32, this.#view.byteLength);
        }
    }

    getInt64(position: number): bigint {
        try {
            return this.#view.getBigInt64(position, true);
        } catch {
            throw outOfRange(position, SIZEOF_64, this.#view.byteLength);
        }
    }

    getUint64(position: number): bigint {
        try {
            return this.#view.getBigUint64(position, true);
        } catch {
            throw outOfRange(position, SIZEOF_64, this.#view.byteLength);
        }
    }

    getFloat32(position: number): number {
        try {
            return this.#view.getFloat32(position, true);
        } catch {
            throw outOfRange(position, SIZEOF_32, this.#view.byteLength);
        }
    }

    getFloat64(position: number): number {
        try {
            return this.#view.getFloat64(position, true);
        } catch {
            throw outOfRange(position, SIZEOF_64, this.#view.byteLength);
        }
    }

    // The array getters are written out per element type, like the setters.
    // The elements are stored little-endian, so a big-endian host gets a copy
    // in its own byte order; single bytes have no order to swap.

    /**
     * Reads the array at `position`, as a view of the buffer.
     */
    getInt8Array(position: number): Int8Array {
        const length = this.#getSize(position);
        const byteOffset = this.#arrayOffset(position, length);

        return new Int8Array(this.#view.buffer, byteOffset, length);
    }

    /**
     * Reads the array at `position`, as a view of the buffer.
     */
    getUint8Array(position: number): Uint8Array {
        const length = this.#getSize(position);
        const byteOffset = this.#arrayOffset(position, length);

        return new Uint8Array(this.#view.buffer, byteOffset, length);
    }

    /**
     * Reads the array at `position`: a view of the buffer when its elements
     * are aligned, as TableBuilder aligns them, and otherwise a copy, as on
     * a big-endian host.
     */
    getInt16Array(position: number): Int16Array {
        const length = this.#getSize(position);
        const byteLength = length * SIZEOF_16;
        const byteOffset = this.#arrayOffset(position, byteLength);

        if (!IS_LITTLE_ENDIAN) {
            return byteOrderSwapped(
                Int16Array,
                this.#view.buffer,
                byteOffset,
                length
            );
        }

        return byteOffset % SIZEOF_16 === 0
            ? new Int16Array(this.#view.buffer, byteOffset, length)
            : new Int16Array(this.#copy(byteOffset, byteLength));
    }

    /**
     * Reads the array at `position`: a view of the buffer when its elements
     * are aligned, as TableBuilder aligns them, and otherwise a copy, as on
     * a big-endian host.
     */
    getUint16Array(position: number): Uint16Array {
        const length = this.#getSize(position);
        const byteLength = length * SIZEOF_16;
        const byteOffset = this.#arrayOffset(position, byteLength);

        if (!IS_LITTLE_ENDIAN) {
            return byteOrderSwapped(
                Uint16Array,
                this.#view.buffer,
                byteOffset,
                length
            );
        }

        return byteOffset % SIZEOF_16 === 0
            ? new Uint16Array(this.#view.buffer, byteOffset, length)
            : new Uint16Array(this.#copy(byteOffset, byteLength));
    }

    /**
     * Reads the array at `position`: a view of the buffer when its elements
     * are aligned, as TableBuilder aligns them, and otherwise a copy, as on
     * a big-endian host.
     */
    getInt32Array(position: number): Int32Array {
        const length = this.#getSize(position);
        const byteLength = length * SIZEOF_32;
        const byteOffset = this.#arrayOffset(position, byteLength);

        if (!IS_LITTLE_ENDIAN) {
            return byteOrderSwapped(
                Int32Array,
                this.#view.buffer,
                byteOffset,
                length
            );
        }

        return byteOffset % SIZEOF_32 === 0
            ? new Int32Array(this.#view.buffer, byteOffset, length)
            : new Int32Array(this.#copy(byteOffset, byteLength));
    }

    /**
     * Reads the array at `position`: a view of the buffer when its elements
     * are aligned, as TableBuilder aligns them, and otherwise a copy, as on
     * a big-endian host.
     */
    getUint32Array(position: number): Uint32Array {
        const length = this.#getSize(position);
        const byteLength = length * SIZEOF_32;
        const byteOffset = this.#arrayOffset(position, byteLength);

        if (!IS_LITTLE_ENDIAN) {
            return byteOrderSwapped(
                Uint32Array,
                this.#view.buffer,
                byteOffset,
                length
            );
        }

        return byteOffset % SIZEOF_32 === 0
            ? new Uint32Array(this.#view.buffer, byteOffset, length)
            : new Uint32Array(this.#copy(byteOffset, byteLength));
    }

    /**
     * Reads the array at `position`: a view of the buffer when its elements
     * are aligned, as TableBuilder aligns them, and otherwise a copy, as on
     * a big-endian host.
     */
    getInt64Array(position: number): BigInt64Array {
        const length = this.#getSize(position);
        const byteLength = length * SIZEOF_64;
        const byteOffset = this.#arrayOffset(position, byteLength);

        if (!IS_LITTLE_ENDIAN) {
            return byteOrderSwapped(
                BigInt64Array,
                this.#view.buffer,
                byteOffset,
                length
            );
        }

        return byteOffset % SIZEOF_64 === 0
            ? new BigInt64Array(this.#view.buffer, byteOffset, length)
            : new BigInt64Array(this.#copy(byteOffset, byteLength));
    }

    /**
     * Reads the array at `position`: a view of the buffer when its elements
     * are aligned, as TableBuilder aligns them, and otherwise a copy, as on
     * a big-endian host.
     */
    getUint64Array(position: number): BigUint64Array {
        const length = this.#getSize(position);
        const byteLength = length * SIZEOF_64;
        const byteOffset = this.#arrayOffset(position, byteLength);

        if (!IS_LITTLE_ENDIAN) {
            return byteOrderSwapped(
                BigUint64Array,
                this.#view.buffer,
                byteOffset,
                length
            );
        }

        return byteOffset % SIZEOF_64 === 0
            ? new BigUint64Array(this.#view.buffer, byteOffset, length)
            : new BigUint64Array(this.#copy(byteOffset, byteLength));
    }

    /**
     * Reads the array at `position`: a view of the buffer when its elements
     * are aligned, as TableBuilder aligns them, and otherwise a copy, as on
     * a big-endian host.
     */
    getFloat32Array(position: number): Float32Array {
        const length = this.#getSize(position);
        const byteLength = length * SIZEOF_32;
        const byteOffset = this.#arrayOffset(position, byteLength);

        if (!IS_LITTLE_ENDIAN) {
            return byteOrderSwapped(
                Float32Array,
                this.#view.buffer,
                byteOffset,
                length
            );
        }

        return byteOffset % SIZEOF_32 === 0
            ? new Float32Array(this.#view.buffer, byteOffset, length)
            : new Float32Array(this.#copy(byteOffset, byteLength));
    }

    /**
     * Reads the array at `position`: a view of the buffer when its elements
     * are aligned, as TableBuilder aligns them, and otherwise a copy, as on
     * a big-endian host.
     */
    getFloat64Array(position: number): Float64Array {
        const length = this.#getSize(position);
        const byteLength = length * SIZEOF_64;
        const byteOffset = this.#arrayOffset(position, byteLength);

        if (!IS_LITTLE_ENDIAN) {
            return byteOrderSwapped(
                Float64Array,
                this.#view.buffer,
                byteOffset,
                length
            );
        }

        return byteOffset % SIZEOF_64 === 0
            ? new Float64Array(this.#view.buffer, byteOffset, length)
            : new Float64Array(this.#copy(byteOffset, byteLength));
    }

    /**
     * Checks that `byteLength` bytes of elements follow the length prefix at
     * `position`.
     *
     * @returns The byte offset of the elements in the backing buffer.
     */
    #arrayOffset(position: number, byteLength: number): number {
        this.#assertRange(position + this.#size, byteLength);
        return this.#view.byteOffset + position + this.#size;
    }

    /** A typed array can't start at an unaligned offset, so copy its bytes */
    #copy(byteOffset: number, byteLength: number): ArrayBufferLike {
        return this.#view.buffer.slice(byteOffset, byteOffset + byteLength);
    }

    setInt8(value: number): void {
        this.#view.setInt8(this.#position, value);
        this.#position += SIZEOF_8;
    }

    setUint8(value: number): void {
        this.#view.setUint8(this.#position, value);
        this.#position += SIZEOF_8;
    }

    setInt16(value: number): void {
        this.#view.setInt16(this.#position, value, true);
        this.#position += SIZEOF_16;
    }

    setUint16(value: number): void {
        this.#view.setUint16(this.#position, value, true);
        this.#position += SIZEOF_16;
    }

    setInt32(value: number): void {
        this.#view.setInt32(this.#position, value, true);
        this.#position += SIZEOF_32;
    }

    setUint32(value: number): void {
        this.#view.setUint32(this.#position, value, true);
        this.#position += SIZEOF_32;
    }

    setInt64(value: bigint): void {
        this.#view.setBigInt64(this.#position, value, true);
        this.#position += SIZEOF_64;
    }

    setUint64(value: bigint): void {
        this.#view.setBigUint64(this.#position, value, true);
        this.#position += SIZEOF_64;
    }

    setFloat32(value: number): void {
        this.#view.setFloat32(this.#position, value, true);
        this.#position += SIZEOF_32;
    }

    setFloat64(value: number): void {
        this.#view.setFloat64(this.#position, value, true);
        this.#position += SIZEOF_64;
    }

    // The array setters are written out per element type, so that each
    // conversion and typed array construction sees one type and stays
    // monomorphic, whatever other types the program writes.

    /**
     * Writes the length and then the elements. An `Int8Array` is copied as
     * bytes; any other array-like is converted as `new Int8Array(values)`
     * would.
     */
    setInt8Array(values: ArrayLike<number> | Int8Array): void {
        const length = values.length;
        const byteOffset = this.#startArray(length, SIZEOF_8);

        new Int8Array(this.#bytes.buffer, byteOffset, length).set(values);
        this.#position += length;
    }

    /**
     * Writes the length and then the elements. A `Uint8Array` is copied as
     * bytes; any other array-like is converted as `new Uint8Array(values)`
     * would.
     */
    setUint8Array(values: ArrayLike<number> | Uint8Array): void {
        const length = values.length;
        const byteOffset = this.#startArray(length, SIZEOF_8);

        new Uint8Array(this.#bytes.buffer, byteOffset, length).set(values);
        this.#position += length;
    }

    /**
     * Writes the length and then the elements. An `Int16Array` is copied as
     * bytes; any other array-like is converted as `new Int16Array(values)`
     * would.
     */
    setInt16Array(values: ArrayLike<number> | Int16Array): void {
        const length = values.length;
        const byteOffset = this.#startArray(length, SIZEOF_16);

        if (IS_LITTLE_ENDIAN && byteOffset % SIZEOF_16 === 0) {
            new Int16Array(this.#bytes.buffer, byteOffset, length).set(values);
        } else {
            this.#copyElements(
                values instanceof Int16Array ? values : new Int16Array(values)
            );
        }

        this.#position += length * SIZEOF_16;
    }

    /**
     * Writes the length and then the elements. A `Uint16Array` is copied as
     * bytes; any other array-like is converted as `new Uint16Array(values)`
     * would.
     */
    setUint16Array(values: ArrayLike<number> | Uint16Array): void {
        const length = values.length;
        const byteOffset = this.#startArray(length, SIZEOF_16);

        if (IS_LITTLE_ENDIAN && byteOffset % SIZEOF_16 === 0) {
            new Uint16Array(this.#bytes.buffer, byteOffset, length).set(values);
        } else {
            this.#copyElements(
                values instanceof Uint16Array ? values : new Uint16Array(values)
            );
        }

        this.#position += length * SIZEOF_16;
    }

    /**
     * Writes the length and then the elements. An `Int32Array` is copied as
     * bytes; any other array-like is converted as `new Int32Array(values)`
     * would.
     */
    setInt32Array(values: ArrayLike<number> | Int32Array): void {
        const length = values.length;
        const byteOffset = this.#startArray(length, SIZEOF_32);

        if (IS_LITTLE_ENDIAN && byteOffset % SIZEOF_32 === 0) {
            new Int32Array(this.#bytes.buffer, byteOffset, length).set(values);
        } else {
            this.#copyElements(
                values instanceof Int32Array ? values : new Int32Array(values)
            );
        }

        this.#position += length * SIZEOF_32;
    }

    /**
     * Writes the length and then the elements. A `Uint32Array` is copied as
     * bytes; any other array-like is converted as `new Uint32Array(values)`
     * would.
     */
    setUint32Array(values: ArrayLike<number> | Uint32Array): void {
        const length = values.length;
        const byteOffset = this.#startArray(length, SIZEOF_32);

        if (IS_LITTLE_ENDIAN && byteOffset % SIZEOF_32 === 0) {
            new Uint32Array(this.#bytes.buffer, byteOffset, length).set(values);
        } else {
            this.#copyElements(
                values instanceof Uint32Array ? values : new Uint32Array(values)
            );
        }

        this.#position += length * SIZEOF_32;
    }

    /**
     * Writes the length and then the elements. A `BigInt64Array` is copied as
     * bytes; any other array-like is converted as `new BigInt64Array(values)`
     * would.
     */
    setInt64Array(values: ArrayLike<bigint> | BigInt64Array): void {
        const length = values.length;
        const byteOffset = this.#startArray(length, SIZEOF_64);

        if (IS_LITTLE_ENDIAN && byteOffset % SIZEOF_64 === 0) {
            new BigInt64Array(this.#bytes.buffer, byteOffset, length).set(
                values
            );
        } else {
            this.#copyElements(
                values instanceof BigInt64Array
                    ? values
                    : new BigInt64Array(values)
            );
        }

        this.#position += length * SIZEOF_64;
    }

    /**
     * Writes the length and then the elements. A `BigUint64Array` is copied as
     * bytes; any other array-like is converted as `new BigUint64Array(values)`
     * would.
     */
    setUint64Array(values: ArrayLike<bigint> | BigUint64Array): void {
        const length = values.length;
        const byteOffset = this.#startArray(length, SIZEOF_64);

        if (IS_LITTLE_ENDIAN && byteOffset % SIZEOF_64 === 0) {
            new BigUint64Array(this.#bytes.buffer, byteOffset, length).set(
                values
            );
        } else {
            this.#copyElements(
                values instanceof BigUint64Array
                    ? values
                    : new BigUint64Array(values)
            );
        }

        this.#position += length * SIZEOF_64;
    }

    /**
     * Writes the length and then the elements. A `Float32Array` is copied as
     * bytes; any other array-like is converted as `new Float32Array(values)`
     * would.
     */
    setFloat32Array(values: ArrayLike<number> | Float32Array): void {
        const length = values.length;
        const byteOffset = this.#startArray(length, SIZEOF_32);

        if (IS_LITTLE_ENDIAN && byteOffset % SIZEOF_32 === 0) {
            new Float32Array(this.#bytes.buffer, byteOffset, length).set(
                values
            );
        } else {
            this.#copyElements(
                values instanceof Float32Array
                    ? values
                    : new Float32Array(values)
            );
        }

        this.#position += length * SIZEOF_32;
    }

    /**
     * Writes the length and then the elements. A `Float64Array` is copied as
     * bytes; any other array-like is converted as `new Float64Array(values)`
     * would.
     */
    setFloat64Array(values: ArrayLike<number> | Float64Array): void {
        const length = values.length;
        const byteOffset = this.#startArray(length, SIZEOF_64);

        if (IS_LITTLE_ENDIAN && byteOffset % SIZEOF_64 === 0) {
            new Float64Array(this.#bytes.buffer, byteOffset, length).set(
                values
            );
        } else {
            this.#copyElements(
                values instanceof Float64Array
                    ? values
                    : new Float64Array(values)
            );
        }

        this.#position += length * SIZEOF_64;
    }

    /** Writes `bytes` as they are, without a length prefix */
    setBytes(bytes: Uint8Array): void {
        this.#bytes.set(bytes, this.#position);
        this.#position += bytes.length;
    }

    /** Writes `byteLength` zeros, such as to align the field after them */
    setPadding(byteLength: number): void {
        this.#assertWritable(byteLength);

        this.#bytes.fill(0, this.#position, this.#position + byteLength);
        this.#position += byteLength;
    }

    /**
     * Checks that `length` elements of `elementSize` bytes fit after their
     * length prefix, and writes the prefix.
     *
     * Every array setter writes through a typed array view of the elements
     * with `set()`, which converts any array-like in a built-in. A JS loop
     * instead would share its type feedback between callers: once a loop had
     * seen typed arrays and JS arrays, writing 27,028 floats from a `number[]`
     * took 130–180 µs, against 14–20 µs for the built-in.
     *
     * @returns The byte offset of the elements in the backing buffer.
     */
    #startArray(length: number, elementSize: ElementSize): number {
        this.#assertWritable(this.#size + length * elementSize);
        this.#setSize(length);

        return this.#bytes.byteOffset + this.#position;
    }

    /**
     * Copies the elements' bytes in little-endian order, for a destination
     * that a view can't start at or a big-endian host.
     */
    #copyElements(values: TypedArray): void {
        const destination = this.#bytes.subarray(
            this.#position,
            this.#position + values.byteLength
        );

        if (IS_LITTLE_ENDIAN) {
            destination.set(toUint8Array(values));
        } else {
            byteOrderSwap(
                toUint8Array(values),
                destination,
                values.BYTES_PER_ELEMENT
            );
        }
    }

    /** @throws {DecodeError} If the string isn't valid UTF-8. */
    getString(position: number): string {
        const length = this.#getSize(position);
        const bytes = new Uint8Array(
            this.#view.buffer,
            this.#arrayOffset(position, length),
            length
        );

        try {
            return decode(bytes);
        } catch (error) {
            throw new DecodeError(
                `The string at ${position} isn't valid UTF-8`,
                { cause: error }
            );
        }
    }

    setString(bytes: Uint8Array): void {
        this.#setSize(bytes.length);
        this.setBytes(bytes);
    }

    getVectorOffsets(position: number): Uint32Array {
        const count = this.#getSize(position);
        // Every vector item needs at least its size prefix. Validate before
        // allocating the offset array from an untrusted count.
        this.#assertRange(position + this.#size, count * this.#size);
        const offsets = new Uint32Array(count);

        for (let i = 0, offset = position + this.#size; i < count; i++) {
            const length = this.#getSize(offset);
            this.#assertRange(offset + this.#size, length);
            offsets[i] = offset + this.#size;
            offset += this.#size + length;
        }

        return offsets;
    }

    getVector(position: number): Uint8Array[] {
        const offsets = this.getVectorOffsets(position);
        const vector = new Array<Uint8Array>(offsets.length);

        // An indexed loop, since `Array.from` goes through the iterator
        // protocol for typed arrays
        for (let i = 0; i < offsets.length; i++) {
            vector[i] = this.getUint8Array(offsets[i] - this.#size);
        }

        return vector;
    }

    setVector(vector: Uint8Array[]): void {
        this.#setSize(vector.length);

        for (const bytes of vector) {
            this.#setSize(bytes.length);
            this.setBytes(bytes);
        }
    }

    #assertRange(position: number, byteLength: number): void {
        if (
            !Number.isSafeInteger(position) ||
            position < 0 ||
            !Number.isSafeInteger(byteLength) ||
            byteLength < 0 ||
            position > this.#view.byteLength - byteLength
        ) {
            throw outOfRange(position, byteLength, this.#view.byteLength);
        }
    }

    #assertWritable(byteLength: number): void {
        if (this.#position > this.#bytes.length - byteLength) {
            throw new RangeError('Write exceeds the supplied buffer');
        }
    }

    #getSize(position: number): number {
        switch (this.#size) {
            case SIZEOF_8:
                return this.getUint8(position);

            case SIZEOF_16:
                return this.getUint16(position);

            case SIZEOF_32:
                return this.getUint32(position);
        }
    }

    #setSize(value: number): void {
        switch (this.#size) {
            case SIZEOF_8:
                this.#view.setUint8(this.#position, value);
                break;

            case SIZEOF_16:
                this.#view.setUint16(this.#position, value, true);
                break;

            case SIZEOF_32:
                this.#view.setUint32(this.#position, value, true);
                break;
        }

        this.#position += this.#size;
    }
}
