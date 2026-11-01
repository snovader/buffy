// Written by hand in the shape that flatc 25.2.10 generates with `--java` from
// src/main/flatbuffers/workloads.fbs, with only the accessors and builders that the benchmarks use

package io.github.snovader.tafla.benchmarks.generated.flatbuffers;

import com.google.flatbuffers.FlatBufferBuilder;
import com.google.flatbuffers.Table;
import java.nio.ByteBuffer;

@SuppressWarnings("unused")
public final class Item extends Table {
  public void __init(int _i, ByteBuffer _bb) { __reset(_i, _bb); }
  public Item __assign(int _i, ByteBuffer _bb) { __init(_i, _bb); return this; }

  public float value() { int o = __offset(4); return o != 0 ? bb.getFloat(o + bb_pos) : 0.0f; }
  public Pair pair() { return pair(new Pair()); }
  public Pair pair(Pair obj) { int o = __offset(6); return o != 0 ? obj.__assign(__indirect(o + bb_pos), bb) : null; }

  public static int createItem(FlatBufferBuilder builder,
      float value,
      int pairOffset) {
    builder.startTable(2);
    Item.addPair(builder, pairOffset);
    Item.addValue(builder, value);
    return Item.endItem(builder);
  }

  public static void addValue(FlatBufferBuilder builder, float value) { builder.addFloat(0, value, 0.0f); }
  public static void addPair(FlatBufferBuilder builder, int pairOffset) { builder.addOffset(1, pairOffset, 0); }
  public static int endItem(FlatBufferBuilder builder) {
    int o = builder.endTable();
    return o;
  }
}
