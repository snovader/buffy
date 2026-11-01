import { describe, expect } from 'vitest';
import { fc, it } from '@fast-check/vitest';
import { DecodeError } from 'tafla';
import { CORRUPTIONS, corruptionOf, SLICES, sliceOf } from '../conformance/arbitraries.ts';
import { corrupt } from '../conformance/description.ts';
import { buildTable } from '../conformance/tables.ts';
import {
    ALL_TYPES,
    ARRAYS_FIELDS,
    DOCUMENTED_FIELDS,
    describeArrays,
    describeNested,
    describeReserved,
    describeScalars,
    documentedBytes,
    NESTED_FIELDS,
    packedBytes,
    RESERVED_FIELDS,
    readArrays,
    readDocumented,
    readNested,
    readReserved,
    readScalars,
    SCALARS_FIELDS
} from './allTypesTables.ts';
import { expectValuesOrDecodeError } from './expectations.ts';

// The code generated from schemas/all_types.tafla, in TypeScript and in
// JavaScript, writes and reads tables of every type of field with tafla

describe.each(ALL_TYPES)('%s', (_, allTypes) => {
    describe('round trip', () => {
        it.prop([SCALARS_FIELDS, SLICES])(
            'should read back a field of every scalar type, wherever its bytes lie',
            (fields, slice) => {
                const bytes = sliceOf(allTypes.Scalars.serialize(fields), slice);

                expect(readScalars(new allTypes.Scalars(bytes))).toEqual(fields);
            }
        );

        it.prop([ARRAYS_FIELDS, SLICES])(
            'should read back an array of every scalar type, wherever its bytes lie',
            (fields, slice) => {
                const bytes = sliceOf(allTypes.Arrays.serialize(fields), slice);

                expect(readArrays(new allTypes.Arrays(bytes))).toEqual(fields);
            }
        );

        it.prop([NESTED_FIELDS, SLICES])(
            'should read back tables nested three deep and vectors of trees, wherever their bytes lie',
            (fields, slice) => {
                const bytes = sliceOf(allTypes.Nested.serialize(fields), slice);

                expect(readNested(new allTypes.Nested(bytes))).toEqual(fields);
            }
        );

        it.prop([RESERVED_FIELDS, SLICES])(
            'should read back the fields named after keywords, wherever their bytes lie',
            (fields, slice) => {
                const bytes = sliceOf(allTypes.Reserved.serialize(fields), slice);

                expect(readReserved(new allTypes.Reserved(bytes))).toEqual(fields);
            }
        );
    });

    describe('tables built by hand', () => {
        it.prop([SCALARS_FIELDS])(
            'should write the bytes of Scalars that TableBuilder writes',
            fields => {
                const byHand = buildTable(describeScalars(fields)).bytes();

                expect(allTypes.Scalars.serialize(fields)).toStrictEqual(byHand);
            }
        );

        it.prop([ARRAYS_FIELDS])(
            'should write the bytes of Arrays, aligned to their element sizes, that TableBuilder writes',
            fields => {
                const byHand = buildTable(describeArrays(fields)).bytes();

                expect(allTypes.Arrays.serialize(fields)).toStrictEqual(byHand);
            }
        );

        it.prop([NESTED_FIELDS])(
            'should write the bytes of Nested that TableBuilder writes',
            fields => {
                const byHand = buildTable(describeNested(fields)).bytes();

                expect(allTypes.Nested.serialize(fields)).toStrictEqual(byHand);
            }
        );

        it.prop([RESERVED_FIELDS])(
            'should write the bytes of Reserved that TableBuilder writes',
            fields => {
                const byHand = buildTable(describeReserved(fields)).bytes();

                expect(allTypes.Reserved.serialize(fields)).toStrictEqual(byHand);
            }
        );
    });

    describe('decode errors', () => {
        it.prop([SCALARS_FIELDS, CORRUPTIONS])(
            'should read corrupted Scalars as values or throw DecodeError',
            (fields, corruption) => {
                const bytes = allTypes.Scalars.serialize(fields);
                const corrupted = corrupt(bytes, corruptionOf(corruption, bytes.length));

                expectValuesOrDecodeError(() => readScalars(new allTypes.Scalars(corrupted)));
            }
        );

        it.prop([ARRAYS_FIELDS, CORRUPTIONS])(
            'should read corrupted Arrays as values or throw DecodeError',
            (fields, corruption) => {
                const bytes = allTypes.Arrays.serialize(fields);
                const corrupted = corrupt(bytes, corruptionOf(corruption, bytes.length));

                expectValuesOrDecodeError(() => readArrays(new allTypes.Arrays(corrupted)));
            }
        );

        it.prop([NESTED_FIELDS, CORRUPTIONS])(
            'should read corrupted Nested as values or throw DecodeError',
            (fields, corruption) => {
                const bytes = allTypes.Nested.serialize(fields);
                const corrupted = corrupt(bytes, corruptionOf(corruption, bytes.length));

                expectValuesOrDecodeError(() => readNested(new allTypes.Nested(corrupted)));
            }
        );

        it.prop([RESERVED_FIELDS, CORRUPTIONS])(
            'should read corrupted Reserved as values or throw DecodeError',
            (fields, corruption) => {
                const bytes = allTypes.Reserved.serialize(fields);
                const corrupted = corrupt(bytes, corruptionOf(corruption, bytes.length));

                expectValuesOrDecodeError(() => readReserved(new allTypes.Reserved(corrupted)));
            }
        );

        it.prop([fc.uint8Array({ maxLength: 64 })])(
            'should read arbitrary bytes as any message or throw DecodeError',
            bytes => {
                expectValuesOrDecodeError(() => readScalars(new allTypes.Scalars(bytes)));
                expectValuesOrDecodeError(() => readArrays(new allTypes.Arrays(bytes)));
                expectValuesOrDecodeError(() => readNested(new allTypes.Nested(bytes)));
                expectValuesOrDecodeError(() => readReserved(new allTypes.Reserved(bytes)));
            }
        );
    });

    describe('structs', () => {
        it.prop([SCALARS_FIELDS, SLICES])(
            'should write Packed in its 42 bytes, little-endian at fixed offsets, and read it back wherever they lie',
            (fields, slice) => {
                const bytes = allTypes.Packed.serialize(fields);

                expect(allTypes.Packed.SIZE).toBe(42);
                expect(bytes).toStrictEqual(packedBytes(fields));
                expect(readScalars(new allTypes.Packed(sliceOf(bytes, slice)))).toEqual(fields);
            }
        );

        it.prop([DOCUMENTED_FIELDS, SLICES])(
            'should write Documented in its 7 bytes, little-endian at fixed offsets, and read it back wherever they lie',
            (fields, slice) => {
                const bytes = allTypes.Documented.serialize(fields);

                expect(allTypes.Documented.SIZE).toBe(7);
                expect(bytes).toStrictEqual(documentedBytes(fields));
                expect(readDocumented(new allTypes.Documented(sliceOf(bytes, slice)))).toEqual(
                    fields
                );
            }
        );

        it.prop([SCALARS_FIELDS, fc.uint8Array({ minLength: 1, maxLength: 8 })])(
            'should read Packed from the start of longer bytes',
            (fields, after) => {
                const bytes = new Uint8Array([...allTypes.Packed.serialize(fields), ...after]);

                expect(readScalars(new allTypes.Packed(bytes))).toEqual(fields);
            }
        );

        it.prop([fc.uint8Array({ maxLength: 41 })])(
            'should throw DecodeError reading Packed from fewer than 42 bytes',
            bytes => {
                expect(() => new allTypes.Packed(bytes)).toThrow(DecodeError);
            }
        );

        it.prop([fc.uint8Array({ maxLength: 6 })])(
            'should throw DecodeError reading Documented from fewer than 7 bytes',
            bytes => {
                expect(() => new allTypes.Documented(bytes)).toThrow(DecodeError);
            }
        );

        it.prop([SCALARS_FIELDS, SCALARS_FIELDS])(
            'should return bytes of its own from each serialize()',
            (first, second) => {
                const bytes = allTypes.Packed.serialize(first);
                allTypes.Packed.serialize(second);

                expect(bytes).toStrictEqual(packedBytes(first));
            }
        );
    });
});
