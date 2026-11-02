// Written by hand in the shape that flatc 25.2.10 generates with `--java` from
// src/main/flatbuffers/workloads.fbs, with only the accessors and builders that the benchmarks use

package io.github.snovader.tafla.benchmarks.generated.flatbuffers;

import com.google.flatbuffers.FlatBufferBuilder;
import com.google.flatbuffers.Struct;
import java.nio.ByteBuffer;

@SuppressWarnings("unused")
public final class Triple extends Struct {
  public void __init(int _i, ByteBuffer _bb) { __reset(_i, _bb); }
  public Triple __assign(int _i, ByteBuffer _bb) { __init(_i, _bb); return this; }

  public float value() { return bb.getFloat(bb_pos + 0); }
  public float first() { return bb.getFloat(bb_pos + 4); }
  public float second() { return bb.getFloat(bb_pos + 8); }

  public static int createTriple(FlatBufferBuilder builder, float value, float first, float second) {
    builder.prep(4, 12);
    builder.putFloat(second);
    builder.putFloat(first);
    builder.putFloat(value);
    return builder.offset();
  }
}
