import { SIZEOF_32 } from './constants.ts';
import type { AddressableSpace } from './types.ts';

export interface TableLayoutOptions<Versioned extends boolean = boolean> {
    /** Whether the header starts with the table's total byte length */
    sized?: boolean;
    /**
     * Whether the header has a slot for a version, after the length when
     * `sized`. Readers can check it before reading any field.
     */
    versioned?: Versioned;
    /**
     * The width in bytes of the header slots, field offsets and length
     * prefixes, which limits the table's size: 1 for up to 255 bytes, 2 for
     * 65,535 and 4, the default, for 4 GiB.
     */
    offsetSize?: AddressableSpace;
}

/**
 * The header and offset width of a table. A format creates its layout once
 * and passes it to both the `TableBuilder` that writes the table and the
 * `ByteBuffer` that reads it, so that the two can't disagree. Nested tables
 * use {@link TableLayout.DEFAULT}.
 *
 * `Versioned` lets TypeScript require a version where a versioned table is
 * built.
 */
export class TableLayout<Versioned extends boolean = boolean> {
    /** No header slots, and 32-bit offsets and lengths */
    static readonly DEFAULT: TableLayout<false> = new TableLayout(
        false,
        false,
        SIZEOF_32
    );

    readonly #sized: boolean;
    readonly #versioned: Versioned;
    readonly #offsetSize: AddressableSpace;

    private constructor(
        sized: boolean,
        versioned: Versioned,
        offsetSize: AddressableSpace
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

    offsetSize(): AddressableSpace {
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
