// Written by hand in the shape that flatc 25.2.10 generates with `--java` from
// src/main/flatbuffers/workloads.fbs, with only the accessors and builders that the benchmarks use

package io.github.snovader.tafla.benchmarks.generated.flatbuffers;

import com.google.flatbuffers.FlatBufferBuilder;
import com.google.flatbuffers.Table;
import java.nio.ByteBuffer;

@SuppressWarnings("unused")
public final class Pair extends Table {
  public void __init(int _i, ByteBuffer _bb) { __reset(_i, _bb); }
  public Pair __assign(int _i, ByteBuffer _bb) { __init(_i, _bb); return this; }

  public float first() { int o = __offset(4); return o != 0 ? bb.getFloat(o + bb_pos) : 0.0f; }
  public float second() { int o = __offset(6); return o != 0 ? bb.getFloat(o + bb_pos) : 0.0f; }

  public static int createPair(FlatBufferBuilder builder,
      float first,
      float second) {
    builder.startTable(2);
    Pair.addSecond(builder, second);
    Pair.addFirst(builder, first);
    return Pair.endPair(builder);
  }

  public static void addFirst(FlatBufferBuilder builder, float first) { builder.addFloat(0, first, 0.0f); }
  public static void addSecond(FlatBufferBuilder builder, float second) { builder.addFloat(1, second, 0.0f); }
  public static int endPair(FlatBufferBuilder builder) {
    int o = builder.endTable();
    return o;
  }
}
