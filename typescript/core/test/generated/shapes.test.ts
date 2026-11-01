import { describe, expect } from 'vitest';
import { fc, it } from '@fast-check/vitest';
import { DecodeError } from '@buffy/core';
import { CORRUPTIONS, corruptionOf, SLICES, sliceOf } from '../conformance/arbitraries.ts';
import { corrupt } from '../conformance/description.ts';
import { buildTable } from '../conformance/tables.ts';
import { expectValuesOrDecodeError } from './expectations.ts';
import {
    DOCUMENT_FIELDS,
    DOCUMENT_LAYOUT,
    describeDocument,
    describeItemList,
    describePoint,
    ITEM_LIST_FIELDS,
    POINT_FIELDS,
    readDocument,
    readItemList,
    readPoint,
    readSample,
    SAMPLE_FIELDS,
    SHAPES,
    sampleBytes
} from './shapesTables.ts';

// The code generated from schemas/shapes.buffy, in TypeScript and in
// JavaScript, writes and reads tables with @buffy/core

const OTHER_VERSIONS = fc
    .integer({ min: 0, max: 0xffff_ffff })
    .filter(version => version !== DOCUMENT_LAYOUT.version);

describe.each(SHAPES)('%s', (_, shapes) => {
    describe('round trip', () => {
        it.prop([POINT_FIELDS, SLICES])(
            'should read back the fields of a Point, wherever its bytes lie',
            (fields, slice) => {
                const bytes = sliceOf(shapes.Point.serialize(fields), slice);

                expect(readPoint(new shapes.Point(bytes))).toEqual(fields);
            }
        );

        it.prop([DOCUMENT_FIELDS, SLICES])(
            'should read back the fields of a Document and its nested Columns, wherever its bytes lie',
            (fields, slice) => {
                const bytes = sliceOf(shapes.Document.serialize(fields), slice);

                expect(readDocument(new shapes.Document(bytes))).toEqual(fields);
            }
        );

        it.prop([ITEM_LIST_FIELDS, SLICES])(
            'should read back the Items of an ItemList and their Pairs, wherever its bytes lie',
            (fields, slice) => {
                const bytes = sliceOf(shapes.ItemList.serialize(fields), slice);

                expect(readItemList(new shapes.ItemList(bytes))).toEqual(fields);
            }
        );
    });

    describe('tables built by hand', () => {
        it.prop([POINT_FIELDS])(
            'should write the bytes of a Point that TableBuilder writes',
            fields => {
                const byHand = buildTable(describePoint(fields)).bytes();

                expect(shapes.Point.serialize(fields)).toStrictEqual(byHand);
            }
        );

        it.prop([DOCUMENT_FIELDS])(
            'should write the bytes of a Document, sized and versioned, that TableBuilder writes',
            fields => {
                const byHand = buildTable(describeDocument(fields)).bytes();

                expect(shapes.Document.serialize(fields)).toStrictEqual(byHand);
            }
        );

        it.prop([ITEM_LIST_FIELDS])(
            'should write the bytes of an ItemList that TableBuilder writes',
            fields => {
                const byHand = buildTable(describeItemList(fields)).bytes();

                expect(shapes.ItemList.serialize(fields)).toStrictEqual(byHand);
            }
        );
    });

    describe('decode errors', () => {
        it.prop([DOCUMENT_FIELDS, OTHER_VERSIONS])(
            'should reject a Document of another version with DecodeError',
            (fields, version) => {
                const bytes = buildTable(describeDocument(fields, version)).bytes();

                expect(() => new shapes.Document(bytes)).toThrow(DecodeError);
            }
        );

        it.prop([POINT_FIELDS, CORRUPTIONS])(
            'should read a corrupted Point as values or throw DecodeError',
            (fields, corruption) => {
                const bytes = shapes.Point.serialize(fields);
                const corrupted = corrupt(bytes, corruptionOf(corruption, bytes.length));

                expectValuesOrDecodeError(() => readPoint(new shapes.Point(corrupted)));
            }
        );

        it.prop([DOCUMENT_FIELDS, CORRUPTIONS])(
            'should read a corrupted Document as values or throw DecodeError',
            (fields, corruption) => {
                const bytes = shapes.Document.serialize(fields);
                const corrupted = corrupt(bytes, corruptionOf(corruption, bytes.length));

                expectValuesOrDecodeError(() => readDocument(new shapes.Document(corrupted)));
            }
        );

        it.prop([ITEM_LIST_FIELDS, CORRUPTIONS])(
            'should read a corrupted ItemList as values or throw DecodeError',
            (fields, corruption) => {
                const bytes = shapes.ItemList.serialize(fields);
                const corrupted = corrupt(bytes, corruptionOf(corruption, bytes.length));

                expectValuesOrDecodeError(() => readItemList(new shapes.ItemList(corrupted)));
            }
        );

        it.prop([fc.uint8Array({ maxLength: 64 })])(
            'should read arbitrary bytes as any message or throw DecodeError',
            bytes => {
                expectValuesOrDecodeError(() => readPoint(new shapes.Point(bytes)));
                expectValuesOrDecodeError(() => readDocument(new shapes.Document(bytes)));
                expectValuesOrDecodeError(() => readItemList(new shapes.ItemList(bytes)));
            }
        );
    });

    describe('struct', () => {
        it.prop([SAMPLE_FIELDS, SLICES])(
            'should write a Sample in its 12 bytes, little-endian at fixed offsets, and read it back wherever they lie',
            (fields, slice) => {
                const bytes = shapes.Sample.serialize(fields);

                expect(shapes.Sample.SIZE).toBe(12);
                expect(bytes).toStrictEqual(sampleBytes(fields));
                expect(readSample(new shapes.Sample(sliceOf(bytes, slice)))).toEqual(fields);
            }
        );

        it.prop([SAMPLE_FIELDS, fc.uint8Array({ minLength: 1, maxLength: 8 })])(
            'should read a Sample from the start of longer bytes',
            (fields, after) => {
                const bytes = new Uint8Array([...shapes.Sample.serialize(fields), ...after]);

                expect(readSample(new shapes.Sample(bytes))).toEqual(fields);
            }
        );

        it.prop([fc.uint8Array({ maxLength: 11 })])(
            'should throw DecodeError reading a Sample from fewer than 12 bytes',
            bytes => {
                expect(() => new shapes.Sample(bytes)).toThrow(DecodeError);
            }
        );

        it.prop([SAMPLE_FIELDS, SAMPLE_FIELDS])(
            'should return bytes of its own from each serialize()',
            (first, second) => {
                const bytes = shapes.Sample.serialize(first);
                shapes.Sample.serialize(second);

                expect(bytes).toStrictEqual(sampleBytes(first));
            }
        );

        it.prop([SAMPLE_FIELDS, SAMPLE_FIELDS])(
            'should write the next Sample after a serialize() that threw',
            (first, second) => {
                // Throws once `key` and `a` are written
                const unreadable = {
                    ...first,
                    get b(): number {
                        throw new Error('An unreadable field');
                    }
                };

                expect(() => shapes.Sample.serialize(unreadable)).toThrow('An unreadable field');
                expect(shapes.Sample.serialize(second)).toStrictEqual(sampleBytes(second));
            }
        );
    });
});
