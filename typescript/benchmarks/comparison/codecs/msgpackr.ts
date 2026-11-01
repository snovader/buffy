// The pack and unpack entry points are plain JS on every runtime, where the
// package root tries to load a native string extractor on Node
import { Packr } from 'msgpackr/pack';
import type {
    DocumentInput,
    DocumentOutput,
    ListInput,
    RecordInput,
    SampleInput
} from '../workloads.ts';
import type { Codec } from './codec.ts';

// `moreTypes` packs typed arrays as extensions whose bytes are copied, rather
// than as arrays of numbers. Unpacking copies them again to align them.
const packr = new Packr({ moreTypes: true });

/**
 * msgpackr is schemaless: an object packs with its keys, and a `number[]`
 * packs as an array of doubles, so `number[]` columns are converted to typed
 * arrays first, as a caller who wants float32 on the wire would.
 */
export const msgpackr: Codec = {
    name: 'msgpackr',
    record: {
        serialize(input) {
            return packr.pack(input);
        },
        deserialize(bytes) {
            return packr.unpack(bytes) as RecordInput;
        }
    },
    document: {
        serialize(input) {
            return packr.pack({
                ...input,
                keys: toTyped(input.keys, Uint32Array),
                a: toTyped(input.a, Float32Array),
                b: toTyped(input.b, Float32Array)
            });
        },
        deserialize(bytes) {
            return packr.unpack(bytes) as DocumentOutput;
        },
        sumA(bytes) {
            const { a } = packr.unpack(bytes) as DocumentInput;

            let sum = 0;
            for (let i = 0; i < a.length; i++) {
                sum += a[i];
            }

            return sum;
        }
    },
    list: {
        serialize(input) {
            return packr.pack(input);
        },
        deserialize(bytes) {
            return packr.unpack(bytes) as ListInput;
        }
    },
    sample: {
        serialize(input) {
            return packr.pack(input);
        },
        deserialize(bytes) {
            return packr.unpack(bytes) as SampleInput;
        }
    }
};

function toTyped<T extends Float32Array | Uint32Array>(
    values: ArrayLike<number>,
    TypedArray: { new (values: ArrayLike<number>): T }
): T {
    return values instanceof TypedArray ? values : new TypedArray(values);
}
