import { describe, expect } from 'vitest';
import { fc, it } from '@fast-check/vitest';
import {
    ByteBuffer,
    DecodeError,
    readFloat32,
    readInt32,
    readInt64,
    readUint8,
    readUint32,
    TableBuilder,
    TableLayout
} from '@buffy/core';
import { getSerializable, serializable, setSerializable } from './serializable.ts';

const types = [
    'int8',
    'uint8',
    'int16',
    'uint16',
    'int32',
    'uint32',
    'int64',
    'uint64',
    'float32',
    'float64',
    'int8array',
    'uint8array',
    'int16array',
    'uint16array',
    'int32array',
    'uint32array',
    'int64array',
    'uint64array',
    'float32array',
    'float64array',
    'string',
    'vector'
];

/** Reads a field, returning the error it throws instead */
function read(buffer: ByteBuffer, type: string, index: number): unknown {
    try {
        return getSerializable(buffer, type, index);
    } catch (error) {
        return error;
    }
}

describe('DecodeError', () => {
    it('should be a RangeError named DecodeError', () => {
        const error = new DecodeError('message');

        expect(error).toBeInstanceOf(RangeError);
        expect(error.name).toBe('DecodeError');
    });

    it.prop([fc.array(serializable, { minLength: 1, maxLength: 8 }), fc.boolean()])(
        'should read each field of a truncated table or throw DecodeError',
        (entries, sized) => {
            const builder = new TableBuilder(entries.length, TableLayout.create({ sized }));
            for (const entry of entries) {
                setSerializable(builder, entry);
            }

            const bytes = builder.bytes();
            for (let length = 0; length < bytes.length; length++) {
                // A copy, so that no read can reach the bytes after the cut
                const buffer = new ByteBuffer(
                    bytes.slice(0, length),
                    TableLayout.create({ sized })
                );

                for (const [index, [type, value]] of entries.entries()) {
                    const result = read(buffer, type, index);

                    if (result instanceof Error) {
                        expect(result).toBeInstanceOf(DecodeError);
                    } else {
                        expect(result).toStrictEqual(value);
                    }
                }
            }
        }
    );

    it.prop([
        fc.uint8Array({ maxLength: 64 }),
        fc.constantFrom(...types),
        fc.nat({ max: 16 }),
        fc.boolean()
    ])('should throw only DecodeError for any bytes', (bytes, type, index, sized) => {
        const result = read(new ByteBuffer(bytes, TableLayout.create({ sized })), type, index);

        if (result instanceof Error) {
            expect(result).toBeInstanceOf(DecodeError);
        }
    });

    it.each([
        ['getFloat64', (buffer: ByteBuffer) => buffer.getFloat64(4)],
        ['getInt64', (buffer: ByteBuffer) => buffer.getInt64(-1)],
        ['getUint32', (buffer: ByteBuffer) => buffer.getUint32(5)],
        ['getInt8', (buffer: ByteBuffer) => buffer.getInt8(8)]
    ])('should throw DecodeError from %s outside the slice', (_, get) => {
        const backing = new Uint8Array(32);
        const buffer = new ByteBuffer(backing.subarray(0, 8));

        expect(() => get(buffer)).toThrow(DecodeError);
        expect(() => get(buffer)).toThrow('Read exceeds the supplied buffer');
    });

    it.each([
        ['a lone continuation byte', [0x80]],
        ['a truncated sequence', [0xe2, 0x82]],
        ['an overlong encoding', [0xc0, 0xaf]],
        ['an encoded surrogate', [0xed, 0xa0, 0x80]]
    ])('should throw DecodeError for a string with %s', (_, utf8) => {
        const bytes = new Uint8Array([utf8.length, 0, 0, 0, ...utf8]);

        expect(() => new ByteBuffer(bytes).getString(0)).toThrow(DecodeError);
    });

    it('should keep the original error as the cause of an invalid string', () => {
        const buffer = new ByteBuffer(new Uint8Array([1, 0, 0, 0, 0xff]));

        expect(() => buffer.getString(0)).toThrow(
            expect.objectContaining({ cause: expect.any(TypeError) })
        );
    });

    it.prop([fc.string({ unit: 'grapheme' })])(
        'should read back any well-formed string, with a leading U+FEFF',
        string => {
            for (const value of [string, `﻿${string}`]) {
                const builder = new TableBuilder(1);
                builder.setString(value);

                const buffer = builder.finalize();
                expect(buffer.getString(buffer.rootOffset(0))).toBe(value);
            }
        }
    );

    it.each([
        ['readUint8', (bytes: Uint8Array) => readUint8(bytes, 4)],
        ['readInt32', (bytes: Uint8Array) => readInt32(bytes, 1)],
        ['readUint32', (bytes: Uint8Array) => readUint32(bytes, -1)],
        ['readFloat32', (bytes: Uint8Array) => readFloat32(bytes, 2)],
        ['readInt64', (bytes: Uint8Array) => readInt64(bytes, 0)]
    ])('should throw DecodeError from %s past the end', (_, read) => {
        expect(() => read(new Uint8Array(4))).toThrow(DecodeError);
    });
});
