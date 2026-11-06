import { SIZEOF_8, SIZEOF_16, SIZEOF_32 } from './constants.ts';
import type { OffsetSize } from './types.ts';

export interface TableLayoutOptions<Versioned extends boolean = boolean> {
    /** Whether the header starts with the table's total byte length */
    sized?: boolean;
    /**
     * Whether the header has a slot for a version, after the length when
     * `sized`. Readers can check it before reading any field.
     */
    versioned?: Versioned;
    /**
     * The offset size, the width in bytes of the header slots, field offsets,
     * counts, lengths and item ends, which limits the table's length: 1, of
     * `uint8` offsets, for up to 255 bytes, 2, of `uint16`, for 65,535 and 4,
     * of `uint32`, the default, for 4 GiB.
     */
    offsetSize?: OffsetSize;
}

/**
 * The header and offset size of a table. A format creates its layout once
 * and passes it to both the `TableBuilder` that writes the table and the
 * `ByteBuffer` that reads it, so that the two can't disagree. Nested tables
 * use {@link TableLayout.DEFAULT}.
 *
 * `Versioned` lets TypeScript require a version where a versioned table is
 * built.
 */
export class TableLayout<Versioned extends boolean = boolean> {
    /** No header slots, and offsets and lengths of the offset type `uint32` */
    static readonly DEFAULT: TableLayout<false> = new TableLayout(
        false,
        false,
        SIZEOF_32
    );

    readonly #sized: boolean;
    readonly #versioned: Versioned;
    readonly #offsetSize: OffsetSize;

    private constructor(
        sized: boolean,
        versioned: Versioned,
        offsetSize: OffsetSize
    ) {
        this.#sized = sized;
        this.#versioned = versioned;
        this.#offsetSize = offsetSize;
    }

    static create<Versioned extends boolean = false>({
        sized = false,
        versioned = false as Versioned,
        offsetSize = SIZEOF_32
    }: TableLayoutOptions<Versioned> = {}): TableLayout<Versioned> {
        return new TableLayout(sized, versioned, offsetSize);
    }

    sized(): boolean {
        return this.#sized;
    }

    versioned(): Versioned {
        return this.#versioned;
    }

    offsetSize(): OffsetSize {
        return this.#offsetSize;
    }

    /** The byte length of the header slots before the field offsets */
    headerSize(): number {
        return (
            ((this.#sized ? 1 : 0) + (this.#versioned ? 1 : 0)) *
            this.#offsetSize
        );
    }
}

/**
 * The byte length of the header of a table of `fields` fields in `layout`,
 * whose writer puts `version` in the header
 *
 * @throws {TypeError} If `version` is given for a layout without a version
 * slot, or missing for one with it.
 * @throws {RangeError} If `fields` isn't a non-negative integer, `version`
 * isn't an integer that fits in the layout's offset size, or the header
 * alone is longer than the layout addresses.
 */
export function headerLength(
    fields: number,
    layout: TableLayout,
    version: number | undefined
): number {
    assertCount(fields);
    const size = layout.offsetSize();

    if (layout.versioned() !== (version !== undefined)) {
        throw new TypeError(
            layout.versioned()
                ? 'A versioned layout needs a version'
                : 'The layout has no version slot'
        );
    }

    if (version !== undefined && !fitsIn(version, size)) {
        throw new RangeError(
            `Version ${version} doesn't fit in a ${size}-byte slot`
        );
    }

    const length = layout.headerSize() + fields * size;
    if (!(length <= maxLength(size))) {
        throw new RangeError(
            `A header of ${fields} fields doesn't fit in ${size}-byte offsets`
        );
    }

    return length;
}

/**
 * The longest table that offsets of `size` bytes address, which is also the
 * largest number that they hold unsigned, without a power
 */
export function maxLength(size: OffsetSize): number {
    switch (size) {
        case SIZEOF_8:
            return 0xff;

        case SIZEOF_16:
            return 0xffff;

        case SIZEOF_32:
            return 0xffff_ffff;
    }
}

/**
 * Checks that `count`, of a table's fields, an array's elements or a vector's
 * items, is a non-negative integer, which a table can be written with
 *
 * @throws {RangeError} If it isn't.
 */
export function assertCount(count: number): void {
    if (!(Number.isSafeInteger(count) && count >= 0)) {
        throw new RangeError(
            `A count must be a non-negative integer, not ${count}`
        );
    }
}

/** Whether `value` is a whole number that `size` bytes hold unsigned */
function fitsIn(value: number, size: OffsetSize): boolean {
    return Number.isInteger(value) && value >= 0 && value <= maxLength(size);
}
