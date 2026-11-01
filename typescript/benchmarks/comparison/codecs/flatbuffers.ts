import { Builder, ByteBuffer } from 'flatbuffers';
import {
    Columns,
    Document,
    Item,
    List,
    Pair,
    Record,
    Sample
} from '../generated/flatbuffers.ts';
import type { DocumentOutput, ItemInput } from '../workloads.ts';
import type { Codec } from './codec.ts';

/**
 * flatc's generated code, used as its documentation shows: a builder of the
 * default size, vectors created element by element, and `xArray()` views.
 */
export const flatbuffers: Codec = {
    name: 'flatbuffers',
    record: {
        serialize(input) {
            const builder = new Builder();
            const name = builder.createString(input.name);

            builder.finish(
                Record.createRecord(builder, input.x, input.y, input.id, name)
            );

            return builder.asUint8Array();
        },
        deserialize(bytes) {
            const record = Record.getRootAsRecord(new ByteBuffer(bytes));

            return {
                x: record.x(),
                y: record.y(),
                id: record.id(),
                name: record.name()!
            };
        }
    },
    document: {
        serialize(input) {
            const builder = new Builder();
            const keys = Columns.createKeysVector(builder, input.keys);
            const a = Columns.createAVector(builder, input.a);
            const b = Columns.createBVector(builder, input.b);
            const columns = Columns.createColumns(
                builder,
                input.base,
                keys,
                a,
                b
            );
            const name = builder.createString(input.name);

            builder.finish(
                Document.createDocument(
                    builder,
                    input.x,
                    input.y,
                    name,
                    columns
                )
            );

            return builder.asUint8Array();
        },
        deserialize(bytes): DocumentOutput {
            const document = Document.getRootAsDocument(new ByteBuffer(bytes));
            const columns = document.columns()!;

            return {
                x: document.x(),
                y: document.y(),
                name: document.name()!,
                base: columns.base(),
                keys: columns.keysArray()!,
                a: columns.aArray()!,
                b: columns.bArray()!
            };
        },
        sumA(bytes) {
            const columns = Document.getRootAsDocument(
                new ByteBuffer(bytes)
            ).columns()!;

            let sum = 0;
            for (let i = 0, length = columns.aLength(); i < length; i++) {
                sum += columns.a(i)!;
            }

            return sum;
        }
    },
    list: {
        serialize({ items }) {
            const builder = new Builder();
            const offsets = items.map(item => serializeItem(builder, item));

            builder.finish(
                List.createList(
                    builder,
                    List.createItemsVector(builder, offsets)
                )
            );

            return builder.asUint8Array();
        },
        deserialize(bytes) {
            const list = List.getRootAsList(new ByteBuffer(bytes));

            const items = new Array<ItemInput>(list.itemsLength());
            for (let i = 0; i < items.length; i++) {
                const item = list.items(i)!;
                const pair = item.pair()!;

                items[i] = {
                    value: item.value(),
                    first: pair.first(),
                    second: pair.second()
                };
            }

            return { items };
        }
    },
    sample: {
        serialize({ key, a, b }) {
            const builder = new Builder();
            builder.finish(Sample.createSample(builder, key, a, b));

            return builder.asUint8Array();
        },
        deserialize(bytes) {
            const sample = Sample.getRootAsSample(new ByteBuffer(bytes));

            return { key: sample.key(), a: sample.a(), b: sample.b() };
        }
    }
};

function serializeItem(
    builder: Builder,
    { value, first, second }: ItemInput
): number {
    const pair = Pair.createPair(builder, first, second);
    return Item.createItem(builder, value, pair);
}
