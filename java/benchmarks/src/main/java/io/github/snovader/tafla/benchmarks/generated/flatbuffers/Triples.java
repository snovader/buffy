// Written by hand in the shape that flatc 25.2.10 generates with `--java` from
// src/main/flatbuffers/workloads.fbs, with only the accessors and builders that the benchmarks use

package io.github.snovader.tafla.benchmarks.generated.flatbuffers;

import com.google.flatbuffers.FlatBufferBuilder;
import com.google.flatbuffers.Table;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;

@SuppressWarnings("unused")
public final class Triples extends Table {
  public static Triples getRootAsTriples(ByteBuffer _bb) { return getRootAsTriples(_bb, new Triples()); }
  public static Triples getRootAsTriples(ByteBuffer _bb, Triples obj) { _bb.order(ByteOrder.LITTLE_ENDIAN); return (obj.__assign(_bb.getInt(_bb.position()) + _bb.position(), _bb)); }
  public void __init(int _i, ByteBuffer _bb) { __reset(_i, _bb); }
  public Triples __assign(int _i, ByteBuffer _bb) { __init(_i, _bb); return this; }

  public Triple items(int j) { return items(new Triple(), j); }
  public Triple items(Triple obj, int j) { int o = __offset(4); return o != 0 ? obj.__assign(__vector(o) + j * 12, bb) : null; }
  public int itemsLength() { int o = __offset(4); return o != 0 ? __vector_len(o) : 0; }

  public static int createTriples(FlatBufferBuilder builder,
      int itemsOffset) {
    builder.startTable(1);
    Triples.addItems(builder, itemsOffset);
    return Triples.endTriples(builder);
  }

  public static void addItems(FlatBufferBuilder builder, int itemsOffset) { builder.addOffset(0, itemsOffset, 0); }
  public static void startItemsVector(FlatBufferBuilder builder, int numElems) { builder.startVector(12, numElems, 4); }
  public static int endTriples(FlatBufferBuilder builder) {
    int o = builder.endTable();
    return o;
  }
}
