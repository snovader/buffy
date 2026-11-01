// Written by hand in the shape that flatc 25.2.10 generates with `--java` from
// src/main/flatbuffers/workloads.fbs, with only the accessors and builders that the benchmarks use

package io.github.snovader.tafla.benchmarks.generated.flatbuffers;

import com.google.flatbuffers.FlatBufferBuilder;
import com.google.flatbuffers.Table;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;

@SuppressWarnings("unused")
public final class Sample extends Table {
  public static Sample getRootAsSample(ByteBuffer _bb) { return getRootAsSample(_bb, new Sample()); }
  public static Sample getRootAsSample(ByteBuffer _bb, Sample obj) { _bb.order(ByteOrder.LITTLE_ENDIAN); return (obj.__assign(_bb.getInt(_bb.position()) + _bb.position(), _bb)); }
  public void __init(int _i, ByteBuffer _bb) { __reset(_i, _bb); }
  public Sample __assign(int _i, ByteBuffer _bb) { __init(_i, _bb); return this; }

  public long key() { int o = __offset(4); return o != 0 ? (long)bb.getInt(o + bb_pos) & 0xFFFFFFFFL : 0L; }
  public float a() { int o = __offset(6); return o != 0 ? bb.getFloat(o + bb_pos) : 0.0f; }
  public float b() { int o = __offset(8); return o != 0 ? bb.getFloat(o + bb_pos) : 0.0f; }

  public static int createSample(FlatBufferBuilder builder,
      long key,
      float a,
      float b) {
    builder.startTable(3);
    Sample.addB(builder, b);
    Sample.addA(builder, a);
    Sample.addKey(builder, key);
    return Sample.endSample(builder);
  }

  public static void addKey(FlatBufferBuilder builder, long key) { builder.addInt(0, (int) key, (int) 0L); }
  public static void addA(FlatBufferBuilder builder, float a) { builder.addFloat(1, a, 0.0f); }
  public static void addB(FlatBufferBuilder builder, float b) { builder.addFloat(2, b, 0.0f); }
  public static int endSample(FlatBufferBuilder builder) {
    int o = builder.endTable();
    return o;
  }
}
