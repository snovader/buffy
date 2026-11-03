import { describe, expect } from 'vitest';
import { fc, it } from '@fast-check/vitest';
import { DecodeError } from 'tafla';
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

// The code generated from schemas/shapes.tafla, in TypeScript and in
// JavaScript, writes and reads tables with tafla

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

        it.prop([SAMPLE_FIELDS, fc.nat({ max: 16 }), fc.nat({ max: 8 }), fc.nat({ max: 0xff })])(
            'should write a Sample at any offset in longer bytes, and leave the others as they were',
            (fields, offset, after, fill) => {
                const bytes = new Uint8Array(offset + 12 + after).fill(fill);
                shapes.Sample.writeAt(bytes, offset, fields);

                const expected = new Uint8Array(bytes.length).fill(fill);
                expected.set(sampleBytes(fields), offset);
                expect(bytes).toStrictEqual(expected);
            }
        );

        it.prop([SAMPLE_FIELDS, fc.nat({ max: 11 }), fc.nat({ max: 16 })])(
            'should throw RangeError writing a Sample past the end of the bytes',
            (fields, short, offset) => {
                const bytes = new Uint8Array(offset + short);

                expect(() => shapes.Sample.writeAt(bytes, offset, fields)).toThrow(RangeError);
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

        it.prop([SAMPLE_FIELDS, SAMPLE_FIELDS])(
            'should write a Sample whose field getter serializes another Sample',
            (outer, inner) => {
                let innerBytes: Uint8Array | undefined;
                // Reads `b` once `key` and `a` are written
                const reentrant = {
                    ...outer,
                    get b(): number {
                        innerBytes = shapes.Sample.serialize(inner);
                        return outer.b;
                    }
                };

                expect(shapes.Sample.serialize(reentrant)).toStrictEqual(sampleBytes(outer));
                expect(innerBytes).toStrictEqual(sampleBytes(inner));
            }
        );

        it.prop([SAMPLE_FIELDS, SAMPLE_FIELDS])(
            "should write a Sample whose field's valueOf() serializes another Sample",
            (outer, inner) => {
                let innerBytes: Uint8Array | undefined;
                // The setter converts `a` once `key` is written
                const a = {
                    valueOf(): number {
                        innerBytes = shapes.Sample.serialize(inner);
                        return outer.a;
                    }
                };
                const reentrant = { ...outer, a: a as unknown as number };

                expect(shapes.Sample.serialize(reentrant)).toStrictEqual(sampleBytes(outer));
                expect(innerBytes).toStrictEqual(sampleBytes(inner));
            }
        );

        it.prop([SAMPLE_FIELDS, SAMPLE_FIELDS])(
            'should write a Sample whose field getter catches a serialize() of another that threw',
            (outer, inner) => {
                const unreadable = {
                    ...inner,
                    get b(): number {
                        throw new Error('An unreadable field');
                    }
                };
                const reentrant = {
                    ...outer,
                    get b(): number {
                        expect(() => shapes.Sample.serialize(unreadable)).toThrow(
                            'An unreadable field'
                        );
                        return outer.b;
                    }
                };

                expect(shapes.Sample.serialize(reentrant)).toStrictEqual(sampleBytes(outer));
                expect(shapes.Sample.serialize(inner)).toStrictEqual(sampleBytes(inner));
            }
        );
    });
});
