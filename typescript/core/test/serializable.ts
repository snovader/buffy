import { fc } from '@fast-check/vitest';
import type { ByteBuffer, TableBuilder } from '@buffy/core';
import {
    INT_8_MAX,
    INT_16_MAX,
    INT_32_MAX,
    INT_64_MAX,
    UINT_8_MAX,
    UINT_16_MAX,
    UINT_32_MAX,
    UINT_64_MAX
} from './limits.ts';

/** An array field of each element type, as [type, values] */
export const typedArrays = fc.oneof(
    fc.tuple(fc.constant('int8array'), fc.int8Array()),
    fc.tuple(fc.constant('uint8array'), fc.uint8Array()),
    fc.tuple(fc.constant('int16array'), fc.int16Array()),
    fc.tuple(fc.constant('uint16array'), fc.uint16Array()),
    fc.tuple(fc.constant('int32array'), fc.int32Array()),
    fc.tuple(fc.constant('uint32array'), fc.uint32Array()),
    fc.tuple(fc.constant('int64array'), fc.bigInt64Array()),
    fc.tuple(fc.constant('uint64array'), fc.bigUint64Array()),
    fc.tuple(fc.constant('float32array'), fc.float32Array()),
    fc.tuple(fc.constant('float64array'), fc.float64Array())
);

/** A field of each type a table holds, as [type, value] */
export const serializable = fc.oneof(
    fc.tuple(
        fc.constant('int8'),
        fc.integer({ min: -INT_8_MAX, max: INT_8_MAX - 1 })
    ),
    fc.tuple(fc.constant('uint8'), fc.integer({ min: 0, max: UINT_8_MAX - 1 })),
    fc.tuple(
        fc.constant('int16'),
        fc.integer({ min: -INT_16_MAX, max: INT_16_MAX - 1 })
    ),
    fc.tuple(
        fc.constant('uint16'),
        fc.integer({ min: 0, max: UINT_16_MAX - 1 })
    ),
    fc.tuple(
        fc.constant('int32'),
        fc.integer({ min: -INT_32_MAX, max: INT_32_MAX - 1 })
    ),
    fc.tuple(
        fc.constant('uint32'),
        fc.integer({ min: 0, max: UINT_32_MAX - 1 })
    ),
    fc.tuple(
        fc.constant('int64'),
        fc.bigInt({ min: -INT_64_MAX, max: INT_64_MAX - 1n })
    ),
    fc.tuple(
        fc.constant('uint64'),
        fc.bigInt({ min: 0n, max: UINT_64_MAX - 1n })
    ),
    fc.tuple(fc.constant('float32'), fc.float()),
    fc.tuple(fc.constant('float64'), fc.double()),
    typedArrays,
    fc.tuple(fc.constant('string'), fc.string()),
    fc.tuple(fc.constant('vector'), fc.array(fc.uint8Array()))
);

export function getSerializable(
    byteBuffer: ByteBuffer,
    type: string,
    index: number
): unknown {
    return getSerializableAt(byteBuffer, type, byteBuffer.rootOffset(index));
}

/** Reads a field of `type` at `offset`, such as a field of a nested table */
export function getSerializableAt(
    byteBuffer: ByteBuffer,
    type: string,
    offset: number
): unknown {
    switch (type) {
        case 'int8':
            return byteBuffer.getInt8(offset);

        case 'uint8':
            return byteBuffer.getUint8(offset);

        case 'int16':
            return byteBuffer.getInt16(offset);

        case 'uint16':
            return byteBuffer.getUint16(offset);

        case 'int32':
            return byteBuffer.getInt32(offset);

        case 'uint32':
            return byteBuffer.getUint32(offset);

        case 'int64':
            return byteBuffer.getInt64(offset);

        case 'uint64':
            return byteBuffer.getUint64(offset);

        case 'float32':
            return byteBuffer.getFloat32(offset);

        case 'float64':
            return byteBuffer.getFloat64(offset);

        case 'int8array':
            return byteBuffer.getInt8Array(offset);

        case 'uint8array':
            return byteBuffer.getUint8Array(offset);

        case 'int16array':
            return byteBuffer.getInt16Array(offset);

        case 'uint16array':
            return byteBuffer.getUint16Array(offset);

        case 'int32array':
            return byteBuffer.getInt32Array(offset);

        case 'uint32array':
            return byteBuffer.getUint32Array(offset);

        case 'int64array':
            return byteBuffer.getInt64Array(offset);

        case 'uint64array':
            return byteBuffer.getUint64Array(offset);

        case 'float32array':
            return byteBuffer.getFloat32Array(offset);

        case 'float64array':
            return byteBuffer.getFloat64Array(offset);

        case 'string':
            return byteBuffer.getString(offset);

        case 'vector':
            return byteBuffer.getVector(offset);

        default:
            throw new Error(`Unknown type: ${type}`);
    }
}

export function setSerializable(
    builder: TableBuilder,
    [type, value]: [string, unknown]
): void {
    switch (type) {
        case 'int8':
            builder.setInt8(value as number);
            break;

        case 'uint8':
            builder.setUint8(value as number);
            break;

        case 'int16':
            builder.setInt16(value as number);
            break;

        case 'uint16':
            builder.setUint16(value as number);
            break;

        case 'int32':
            builder.setInt32(value as number);
            break;

        case 'uint32':
            builder.setUint32(value as number);
            break;

        case 'int64':
            builder.setInt64(value as bigint);
            break;

        case 'uint64':
            builder.setUint64(value as bigint);
            break;

        case 'float32':
            builder.setFloat32(value as number);
            break;

        case 'float64':
            builder.setFloat64(value as number);
            break;

        case 'int8array':
            builder.setInt8Array(value as Int8Array);
            break;

        case 'uint8array':
            builder.setUint8Array(value as Uint8Array);
            break;

        case 'int16array':
            builder.setInt16Array(value as Int16Array);
            break;

        case 'uint16array':
            builder.setUint16Array(value as Uint16Array);
            break;

        case 'int32array':
            builder.setInt32Array(value as Int32Array);
            break;

        case 'uint32array':
            builder.setUint32Array(value as Uint32Array);
            break;

        case 'int64array':
            builder.setInt64Array(value as BigInt64Array);
            break;

        case 'uint64array':
            builder.setUint64Array(value as BigUint64Array);
            break;

        case 'float32array':
            builder.setFloat32Array(value as Float32Array);
            break;

        case 'float64array':
            builder.setFloat64Array(value as Float64Array);
            break;

        case 'string':
            builder.setString(value as string);
            break;

        case 'vector':
            builder.setVector(value as Uint8Array[]);
            break;

        default:
            throw new Error(`Unknown type: ${type}`);
    }
}
