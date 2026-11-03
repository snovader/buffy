import { ByteBuffer } from './byteBuffer.ts';

/**
 * Writes scalars one after another into the supplied bytes, without a header,
 * offsets, length prefixes or padding, so a struct is read back at fixed
 * offsets. `bytes()` rewinds and returns a copy, so one writer can be reused for
 * every struct with the same layout. A struct whose write threw never reaches
 * `bytes()`, so a shared writer calls `reset()` before each struct. A struct
 * written while another is, such as from a getter of one of its values,
 * mustn't share its writer, whose `reset()` would write over the other.
 */
export class StructWriter {
    readonly #buffer: ByteBuffer;

    constructor(bytes: Uint8Array) {
        this.#buffer = new ByteBuffer(bytes);
    }

    /** Rewinds to the start, discarding a struct that wasn't finished */
    reset(): void {
        this.#buffer.reset();
    }

    bytes(): Uint8Array {
        this.#buffer.reset();
        return this.#buffer.bytes().slice();
    }

    setInt8(value: number): void {
        this.#buffer.setInt8(value);
    }

    setUint8(value: number): void {
        this.#buffer.setUint8(value);
    }

    setInt16(value: number): void {
        this.#buffer.setInt16(value);
    }

    setUint16(value: number): void {
        this.#buffer.setUint16(value);
    }

    setInt32(value: number): void {
        this.#buffer.setInt32(value);
    }

    setUint32(value: number): void {
        this.#buffer.setUint32(value);
    }

    setInt64(value: bigint): void {
        this.#buffer.setInt64(value);
    }

    setUint64(value: bigint): void {
        this.#buffer.setUint64(value);
    }

    setFloat32(value: number): void {
        this.#buffer.setFloat32(value);
    }

    setFloat64(value: number): void {
        this.#buffer.setFloat64(value);
    }
}
