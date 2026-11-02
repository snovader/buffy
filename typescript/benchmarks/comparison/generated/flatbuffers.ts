/*
 * Written by hand in the shape that flatc 25.9.23 generates with `--ts`, with
 * only the accessors and builders the benchmarks use, from this schema:
 *
 *     table Record { x: double; y: double; id: long; name: string; }
 *
 *     table Columns { base: long; keys: [uint]; a: [float]; b: [float]; }
 *     table Document { x: double; y: double; name: string; columns: Columns; }
 *
 *     table Pair { first: float; second: float; }
 *     table Item { value: float; pair: Pair; }
 *     table List { items: [Item]; }
 *
 *     struct Triple { value: float; first: float; second: float; }
 *     table Triples { items: [Triple]; }
 *
 *     table Sample { key: uint; a: float; b: float; }
 */

import type { Builder, ByteBuffer, Offset } from 'flatbuffers';

export class Record {
    bb: ByteBuffer | null = null;
    bb_pos = 0;

    __init(i: number, bb: ByteBuffer): Record {
        this.bb_pos = i;
        this.bb = bb;
        return this;
    }

    static getRootAsRecord(bb: ByteBuffer, obj?: Record): Record {
        return (obj || new Record()).__init(
            bb.readInt32(bb.position()) + bb.position(),
            bb
        );
    }

    x(): number {
        const offset = this.bb!.__offset(this.bb_pos, 4);
        return offset ? this.bb!.readFloat64(this.bb_pos + offset) : 0.0;
    }

    y(): number {
        const offset = this.bb!.__offset(this.bb_pos, 6);
        return offset ? this.bb!.readFloat64(this.bb_pos + offset) : 0.0;
    }

    id(): bigint {
        const offset = this.bb!.__offset(this.bb_pos, 8);
        return offset ? this.bb!.readInt64(this.bb_pos + offset) : BigInt('0');
    }

    name(): string | null {
        const offset = this.bb!.__offset(this.bb_pos, 10);
        return offset
            ? (this.bb!.__string(this.bb_pos + offset) as string)
            : null;
    }

    static createRecord(
        builder: Builder,
        x: number,
        y: number,
        id: bigint,
        nameOffset: Offset
    ): Offset {
        builder.startObject(4);
        builder.addFieldFloat64(0, x, 0.0);
        builder.addFieldFloat64(1, y, 0.0);
        builder.addFieldInt64(2, id, BigInt('0'));
        builder.addFieldOffset(3, nameOffset, 0);
        return builder.endObject();
    }
}

export class Columns {
    bb: ByteBuffer | null = null;
    bb_pos = 0;

    __init(i: number, bb: ByteBuffer): Columns {
        this.bb_pos = i;
        this.bb = bb;
        return this;
    }

    base(): bigint {
        const offset = this.bb!.__offset(this.bb_pos, 4);
        return offset ? this.bb!.readInt64(this.bb_pos + offset) : BigInt('0');
    }

    keysArray(): Uint32Array | null {
        const offset = this.bb!.__offset(this.bb_pos, 6);
        return offset
            ? new Uint32Array(
                  this.bb!.bytes().buffer,
                  this.bb!.bytes().byteOffset +
                      this.bb!.__vector(this.bb_pos + offset),
                  this.bb!.__vector_len(this.bb_pos + offset)
              )
            : null;
    }

    a(index: number): number | null {
        const offset = this.bb!.__offset(this.bb_pos, 8);
        return offset
            ? this.bb!.readFloat32(
                  this.bb!.__vector(this.bb_pos + offset) + index * 4
              )
            : 0;
    }

    aLength(): number {
        const offset = this.bb!.__offset(this.bb_pos, 8);
        return offset ? this.bb!.__vector_len(this.bb_pos + offset) : 0;
    }

    aArray(): Float32Array | null {
        const offset = this.bb!.__offset(this.bb_pos, 8);
        return offset
            ? new Float32Array(
                  this.bb!.bytes().buffer,
                  this.bb!.bytes().byteOffset +
                      this.bb!.__vector(this.bb_pos + offset),
                  this.bb!.__vector_len(this.bb_pos + offset)
              )
            : null;
    }

    bArray(): Float32Array | null {
        const offset = this.bb!.__offset(this.bb_pos, 10);
        return offset
            ? new Float32Array(
                  this.bb!.bytes().buffer,
                  this.bb!.bytes().byteOffset +
                      this.bb!.__vector(this.bb_pos + offset),
                  this.bb!.__vector_len(this.bb_pos + offset)
              )
            : null;
    }

    static createKeysVector(builder: Builder, data: ArrayLike<number>): Offset {
        builder.startVector(4, data.length, 4);
        for (let i = data.length - 1; i >= 0; i--) {
            builder.addInt32(data[i]!);
        }
        return builder.endVector();
    }

    static createAVector(builder: Builder, data: ArrayLike<number>): Offset {
        builder.startVector(4, data.length, 4);
        for (let i = data.length - 1; i >= 0; i--) {
            builder.addFloat32(data[i]!);
        }
        return builder.endVector();
    }

    static createBVector(builder: Builder, data: ArrayLike<number>): Offset {
        builder.startVector(4, data.length, 4);
        for (let i = data.length - 1; i >= 0; i--) {
            builder.addFloat32(data[i]!);
        }
        return builder.endVector();
    }

    static createColumns(
        builder: Builder,
        base: bigint,
        keysOffset: Offset,
        aOffset: Offset,
        bOffset: Offset
    ): Offset {
        builder.startObject(4);
        builder.addFieldInt64(0, base, BigInt('0'));
        builder.addFieldOffset(1, keysOffset, 0);
        builder.addFieldOffset(2, aOffset, 0);
        builder.addFieldOffset(3, bOffset, 0);
        return builder.endObject();
    }
}

export class Document {
    bb: ByteBuffer | null = null;
    bb_pos = 0;

    __init(i: number, bb: ByteBuffer): Document {
        this.bb_pos = i;
        this.bb = bb;
        return this;
    }

    static getRootAsDocument(bb: ByteBuffer, obj?: Document): Document {
        return (obj || new Document()).__init(
            bb.readInt32(bb.position()) + bb.position(),
            bb
        );
    }

    x(): number {
        const offset = this.bb!.__offset(this.bb_pos, 4);
        return offset ? this.bb!.readFloat64(this.bb_pos + offset) : 0.0;
    }

    y(): number {
        const offset = this.bb!.__offset(this.bb_pos, 6);
        return offset ? this.bb!.readFloat64(this.bb_pos + offset) : 0.0;
    }

    name(): string | null {
        const offset = this.bb!.__offset(this.bb_pos, 8);
        return offset
            ? (this.bb!.__string(this.bb_pos + offset) as string)
            : null;
    }

    columns(obj?: Columns): Columns | null {
        const offset = this.bb!.__offset(this.bb_pos, 10);
        return offset
            ? (obj || new Columns()).__init(
                  this.bb!.__indirect(this.bb_pos + offset),
                  this.bb!
              )
            : null;
    }

    static createDocument(
        builder: Builder,
        x: number,
        y: number,
        nameOffset: Offset,
        columnsOffset: Offset
    ): Offset {
        builder.startObject(4);
        builder.addFieldFloat64(0, x, 0.0);
        builder.addFieldFloat64(1, y, 0.0);
        builder.addFieldOffset(2, nameOffset, 0);
        builder.addFieldOffset(3, columnsOffset, 0);
        return builder.endObject();
    }
}

export class Pair {
    bb: ByteBuffer | null = null;
    bb_pos = 0;

    __init(i: number, bb: ByteBuffer): Pair {
        this.bb_pos = i;
        this.bb = bb;
        return this;
    }

    first(): number {
        const offset = this.bb!.__offset(this.bb_pos, 4);
        return offset ? this.bb!.readFloat32(this.bb_pos + offset) : 0.0;
    }

    second(): number {
        const offset = this.bb!.__offset(this.bb_pos, 6);
        return offset ? this.bb!.readFloat32(this.bb_pos + offset) : 0.0;
    }

    static createPair(builder: Builder, first: number, second: number): Offset {
        builder.startObject(2);
        builder.addFieldFloat32(0, first, 0.0);
        builder.addFieldFloat32(1, second, 0.0);
        return builder.endObject();
    }
}

export class Item {
    bb: ByteBuffer | null = null;
    bb_pos = 0;

    __init(i: number, bb: ByteBuffer): Item {
        this.bb_pos = i;
        this.bb = bb;
        return this;
    }

    value(): number {
        const offset = this.bb!.__offset(this.bb_pos, 4);
        return offset ? this.bb!.readFloat32(this.bb_pos + offset) : 0.0;
    }

    pair(obj?: Pair): Pair | null {
        const offset = this.bb!.__offset(this.bb_pos, 6);
        return offset
            ? (obj || new Pair()).__init(
                  this.bb!.__indirect(this.bb_pos + offset),
                  this.bb!
              )
            : null;
    }

    static createItem(
        builder: Builder,
        value: number,
        pairOffset: Offset
    ): Offset {
        builder.startObject(2);
        builder.addFieldFloat32(0, value, 0.0);
        builder.addFieldOffset(1, pairOffset, 0);
        return builder.endObject();
    }
}

export class List {
    bb: ByteBuffer | null = null;
    bb_pos = 0;

    __init(i: number, bb: ByteBuffer): List {
        this.bb_pos = i;
        this.bb = bb;
        return this;
    }

    static getRootAsList(bb: ByteBuffer, obj?: List): List {
        return (obj || new List()).__init(
            bb.readInt32(bb.position()) + bb.position(),
            bb
        );
    }

    items(index: number, obj?: Item): Item | null {
        const offset = this.bb!.__offset(this.bb_pos, 4);
        return offset
            ? (obj || new Item()).__init(
                  this.bb!.__indirect(
                      this.bb!.__vector(this.bb_pos + offset) + index * 4
                  ),
                  this.bb!
              )
            : null;
    }

    itemsLength(): number {
        const offset = this.bb!.__offset(this.bb_pos, 4);
        return offset ? this.bb!.__vector_len(this.bb_pos + offset) : 0;
    }

    static createItemsVector(builder: Builder, data: Offset[]): Offset {
        builder.startVector(4, data.length, 4);
        for (let i = data.length - 1; i >= 0; i--) {
            builder.addOffset(data[i]!);
        }
        return builder.endVector();
    }

    static createList(builder: Builder, itemsOffset: Offset): Offset {
        builder.startObject(1);
        builder.addFieldOffset(0, itemsOffset, 0);
        return builder.endObject();
    }
}

export class Triple {
    bb: ByteBuffer | null = null;
    bb_pos = 0;

    __init(i: number, bb: ByteBuffer): Triple {
        this.bb_pos = i;
        this.bb = bb;
        return this;
    }

    value(): number {
        return this.bb!.readFloat32(this.bb_pos);
    }

    first(): number {
        return this.bb!.readFloat32(this.bb_pos + 4);
    }

    second(): number {
        return this.bb!.readFloat32(this.bb_pos + 8);
    }

    static sizeOf(): number {
        return 12;
    }

    static createTriple(
        builder: Builder,
        value: number,
        first: number,
        second: number
    ): Offset {
        builder.prep(4, 12);
        builder.writeFloat32(second);
        builder.writeFloat32(first);
        builder.writeFloat32(value);
        return builder.offset();
    }
}

export class Triples {
    bb: ByteBuffer | null = null;
    bb_pos = 0;

    __init(i: number, bb: ByteBuffer): Triples {
        this.bb_pos = i;
        this.bb = bb;
        return this;
    }

    static getRootAsTriples(bb: ByteBuffer, obj?: Triples): Triples {
        return (obj || new Triples()).__init(
            bb.readInt32(bb.position()) + bb.position(),
            bb
        );
    }

    items(index: number, obj?: Triple): Triple | null {
        const offset = this.bb!.__offset(this.bb_pos, 4);
        return offset
            ? (obj || new Triple()).__init(
                  this.bb!.__vector(this.bb_pos + offset) + index * 12,
                  this.bb!
              )
            : null;
    }

    itemsLength(): number {
        const offset = this.bb!.__offset(this.bb_pos, 4);
        return offset ? this.bb!.__vector_len(this.bb_pos + offset) : 0;
    }

    static startItemsVector(builder: Builder, numElems: number): void {
        builder.startVector(12, numElems, 4);
    }

    static createTriples(builder: Builder, itemsOffset: Offset): Offset {
        builder.startObject(1);
        builder.addFieldOffset(0, itemsOffset, 0);
        return builder.endObject();
    }
}

export class Sample {
    bb: ByteBuffer | null = null;
    bb_pos = 0;

    __init(i: number, bb: ByteBuffer): Sample {
        this.bb_pos = i;
        this.bb = bb;
        return this;
    }

    static getRootAsSample(bb: ByteBuffer, obj?: Sample): Sample {
        return (obj || new Sample()).__init(
            bb.readInt32(bb.position()) + bb.position(),
            bb
        );
    }

    key(): number {
        const offset = this.bb!.__offset(this.bb_pos, 4);
        return offset ? this.bb!.readUint32(this.bb_pos + offset) : 0;
    }

    a(): number {
        const offset = this.bb!.__offset(this.bb_pos, 6);
        return offset ? this.bb!.readFloat32(this.bb_pos + offset) : 0.0;
    }

    b(): number {
        const offset = this.bb!.__offset(this.bb_pos, 8);
        return offset ? this.bb!.readFloat32(this.bb_pos + offset) : 0.0;
    }

    static createSample(
        builder: Builder,
        key: number,
        a: number,
        b: number
    ): Offset {
        builder.startObject(3);
        builder.addFieldInt32(0, key, 0);
        builder.addFieldFloat32(1, a, 0.0);
        builder.addFieldFloat32(2, b, 0.0);
        return builder.endObject();
    }
}
