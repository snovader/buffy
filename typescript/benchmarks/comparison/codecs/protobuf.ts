import protobuf from 'protobufjs';
import type { ItemInput } from '../workloads.ts';
import type { Codec } from './codec.ts';

// proto3 packs repeated scalars by default
const { root } = protobuf.parse(`
    syntax = "proto3";

    message Record { double x = 1; double y = 2; int64 id = 3; string name = 4; }

    message Columns {
        int64 base = 1;
        repeated uint32 keys = 2;
        repeated float a = 3;
        repeated float b = 4;
    }

    message Document {
        double x = 1;
        double y = 2;
        string name = 3;
        Columns columns = 4;
    }

    message Pair { float first = 1; float second = 2; }
    message Item { float value = 1; Pair pair = 2; }
    message List { repeated Item items = 1; }

    message Triple { float value = 1; float first = 2; float second = 3; }
    message Triples { repeated Triple items = 1; }

    message Sample { uint32 key = 1; float a = 2; float b = 3; }
`);

const RecordMessage = root.lookupType('Record');
const DocumentMessage = root.lookupType('Document');
const ListMessage = root.lookupType('List');
const TriplesMessage = root.lookupType('Triples');
const SampleMessage = root.lookupType('Sample');

/** The shape protobufjs decodes an int64 into, since `long` is installed */
interface Long {
    toString(): string;
}

interface DecodedRecord {
    x: number;
    y: number;
    id: Long;
    name: string;
}

interface DecodedDocument {
    x: number;
    y: number;
    name: string;
    columns: { base: Long; keys: number[]; a: number[]; b: number[] };
}

interface DecodedItem {
    value: number;
    pair: { first: number; second: number };
}

/**
 * protobufjs with its reflection-generated encoders and decoders, which build
 * their code with `new Function` and so can't run on Workers; `pbjs` emits the
 * same code as a static module for that. Repeated fields decode into
 * `number[]`, which `deserialize()` converts to typed arrays. It encodes a
 * bigint int64 as 0, so int64s are passed as numbers.
 *
 * protobuf has no structs, so an array of them is a repeated message of the
 * three floats, whose fields protobuf writes as fixed32s. Packed repeated
 * floats would be a column for each field, which the documents time already.
 */
export const protobufjs: Codec = {
    name: 'protobufjs',
    record: {
        serialize(input) {
            return RecordMessage.encode({
                ...input,
                id: Number(input.id)
            }).finish();
        },
        deserialize(bytes) {
            const record = RecordMessage.decode(
                bytes
            ) as unknown as DecodedRecord;

            return {
                x: record.x,
                y: record.y,
                id: BigInt(record.id.toString()),
                name: record.name
            };
        }
    },
    document: {
        serialize(input) {
            return DocumentMessage.encode({
                x: input.x,
                y: input.y,
                name: input.name,
                columns: {
                    base: Number(input.base),
                    keys: input.keys,
                    a: input.a,
                    b: input.b
                }
            }).finish();
        },
        deserialize(bytes) {
            const document = DocumentMessage.decode(
                bytes
            ) as unknown as DecodedDocument;
            const { columns } = document;

            return {
                x: document.x,
                y: document.y,
                name: document.name,
                base: BigInt(columns.base.toString()),
                keys: new Uint32Array(columns.keys),
                a: new Float32Array(columns.a),
                b: new Float32Array(columns.b)
            };
        },
        sumA(bytes) {
            const { a } = (
                DocumentMessage.decode(bytes) as unknown as DecodedDocument
            ).columns;

            let sum = 0;
            for (let i = 0; i < a.length; i++) {
                sum += a[i];
            }

            return sum;
        }
    },
    list: {
        serialize({ items }) {
            return ListMessage.encode({
                items: items.map(toItemMessage)
            }).finish();
        },
        deserialize(bytes) {
            const { items } = ListMessage.decode(bytes) as unknown as {
                items: DecodedItem[];
            };

            return {
                items: items.map(({ value, pair }) => ({
                    value,
                    first: pair.first,
                    second: pair.second
                }))
            };
        }
    },
    structs: {
        serialize(input) {
            return TriplesMessage.encode(input).finish();
        },
        deserialize(bytes) {
            const { items } = TriplesMessage.decode(bytes) as unknown as {
                items: ItemInput[];
            };

            return {
                items: items.map(({ value, first, second }) => ({
                    value,
                    first,
                    second
                }))
            };
        }
    },
    sample: {
        serialize(input) {
            return SampleMessage.encode(input).finish();
        },
        deserialize(bytes) {
            const { key, a, b } = SampleMessage.decode(bytes) as unknown as {
                key: number;
                a: number;
                b: number;
            };

            return { key, a, b };
        }
    }
};

function toItemMessage({ value, first, second }: ItemInput): object {
    return { value, pair: { first, second } };
}
