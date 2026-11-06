import { describe, expect, it } from 'vitest';
import { ByteBuffer, DecodeError, TableBuilder } from 'tafla';

describe('DecodeError', () => {
    it('should be a RangeError named DecodeError', () => {
        const error = new DecodeError('message');

        expect(error).toBeInstanceOf(RangeError);
        expect(error.name).toBe('DecodeError');
    });

    it.each<[string, (buffer: ByteBuffer) => unknown, string]>([
        ['getFloat64()', buffer => buffer.getFloat64(4), '8 bytes at 4 of 8'],
        ['getInt64()', buffer => buffer.getInt64(-1), '8 bytes at -1 of 8'],
        ['getUint32()', buffer => buffer.getUint32(5), '4 bytes at 5 of 8'],
        ['getInt8()', buffer => buffer.getInt8(8), '1 bytes at 8 of 8'],
        // The slice holds a vector's count of 1 and its item's end at 13,
        // which is also read as the length of an array or string
        ['getUint8Array()', buffer => buffer.getUint8Array(4), '13 bytes at 8 of 8'],
        ['getString()', buffer => buffer.getString(4), '13 bytes at 8 of 8'],
        ['getStructArray()', buffer => buffer.getStructArray(4, 2), '26 bytes at 8 of 8'],
        ['getVector()', buffer => buffer.getVector(0), '5 bytes at 8 of 8'],
        ['getVectorOffset()', buffer => buffer.getVectorOffset(0, 0), '5 bytes at 8 of 8']
    ])(
        'should give the position and length of a read outside the slice from %s, and the bytes available',
        (_, read, range) => {
            // Bytes that a read past the slice's end would find
            const backing = new Uint8Array(32).fill(0x41);
            const bytes = backing.subarray(0, 8);
            bytes.set([1, 0, 0, 0, 13, 0, 0, 0]);
            const buffer = new ByteBuffer(bytes);

            expect(() => read(buffer)).toThrow(DecodeError);
            expect(() => read(buffer)).toThrow(`Read exceeds the supplied buffer: ${range}`);
        }
    );

    it('should throw DecodeError for a vector item that ends before it starts', () => {
        // A count of 2 puts the first item after the ends, at 12, but it ends at 11
        const bytes = new Uint8Array([2, 0, 0, 0, 11, 0, 0, 0, 12, 0, 0, 0]);

        expect(() => new ByteBuffer(bytes).getVector(0)).toThrow(
            new DecodeError('The vector item at 12 ends before it starts, at 11')
        );
        expect(() => new ByteBuffer(bytes).getVectorOffset(0, 0)).toThrow(DecodeError);
    });

    it.each([
        ['a lone continuation byte', [0x80]],
        ['a truncated sequence', [0xe2, 0x82]],
        ['an overlong encoding', [0xc0, 0xaf]],
        ['an encoded surrogate', [0xed, 0xa0, 0x80]]
    ])('should throw DecodeError for a string with %s', (_, utf8) => {
        const bytes = new Uint8Array([utf8.length, 0, 0, 0, ...utf8]);

        expect(() => new ByteBuffer(bytes).getString(0)).toThrow(
            new DecodeError("The string at 0 isn't valid UTF-8")
        );
    });

    it('should keep the original error as the cause of an invalid string', () => {
        const buffer = new ByteBuffer(new Uint8Array([1, 0, 0, 0, 0xff]));

        expect(() => buffer.getString(0)).toThrow(
            expect.objectContaining({ cause: expect.any(TypeError) })
        );
    });
});

/**
 * A table with a field of each kind that has a getter, in a resizable buffer
 * with room around it, and the getters that read its fields
 */
function resizableTable(): [ArrayBuffer, ByteBuffer, Record<string, () => unknown>] {
    const builder = new TableBuilder(6);
    builder.setUint32(7);
    builder.setFloat64Array([1.5, 2.5]);
    builder.setString('🦀');
    builder.setStruct(Uint8Array.of(1, 2, 3, 4));
    builder.setStructArray(Uint8Array.of(1, 2, 3, 4), 2, 2);
    builder.setVector([Uint8Array.of(5), Uint8Array.of(6, 7)]);
    const table = builder.bytes();

    const buffer = new ArrayBuffer(8 + table.length + 8, { maxByteLength: 1024 });
    const bytes = new Uint8Array(buffer, 8, table.length);
    bytes.set(table);

    const reader = new ByteBuffer(bytes);
    const [scalar, array, string, struct, structArray, vector] = [0, 1, 2, 3, 4, 5].map(index =>
        reader.rootOffset(index)
    );
    const getters: Record<string, () => unknown> = {
        'rootOffset()': () => reader.rootOffset(0),
        'getUint32()': () => reader.getUint32(scalar),
        'getFloat64()': () => reader.getFloat64(array),
        'getFloat64Array()': () => reader.getFloat64Array(array),
        'getUint8Array()': () => reader.getUint8Array(string),
        'getString()': () => reader.getString(string),
        'getStruct()': () => reader.getStruct(struct, 4),
        'getStructArray()': () => reader.getStructArray(structArray, 2),
        'getVectorLength()': () => reader.getVectorLength(vector),
        'getVectorOffset()': () => reader.getVectorOffset(vector, 1),
        'getVectorOffsets()': () => reader.getVectorOffsets(vector),
        'getVector()': () => reader.getVector(vector)
    };

    return [buffer, reader, getters];
}

describe('bytes lost after the reader was made', () => {
    const GETTERS = Object.keys(resizableTable()[2]);

    it.each(GETTERS)('should read the table with %s before its buffer changes', getter => {
        const [, , getters] = resizableTable();

        expect(getters[getter]).not.toThrow();
    });

    it.each(GETTERS)(
        'should throw DecodeError from %s once the buffer has shrunk below the table',
        getter => {
            const [buffer, , getters] = resizableTable();
            buffer.resize(12);

            expect(getters[getter]).toThrow(DecodeError);
        }
    );

    it.each(GETTERS)('should throw DecodeError from %s once the buffer is detached', getter => {
        const [buffer, , getters] = resizableTable();
        buffer.transfer();

        expect(getters[getter]).toThrow(DecodeError);
    });
});
