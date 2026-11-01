// Written by hand in the shape that flatc 25.2.10 generates with `--java` from
// src/main/flatbuffers/workloads.fbs, with only the accessors and builders that the benchmarks use

package io.github.snovader.buffy.benchmarks.generated.flatbuffers;

import com.google.flatbuffers.FlatBufferBuilder;
import com.google.flatbuffers.IntVector;
import com.google.flatbuffers.Table;
import java.nio.ByteBuffer;

@SuppressWarnings("unused")
public final class Columns extends Table {
  public void __init(int _i, ByteBuffer _bb) { __reset(_i, _bb); }
  public Columns __assign(int _i, ByteBuffer _bb) { __init(_i, _bb); return this; }

  public long base() { int o = __offset(4); return o != 0 ? bb.getLong(o + bb_pos) : 0L; }
  public IntVector keysVector() { return keysVector(new IntVector()); }
  public IntVector keysVector(IntVector obj) { int o = __offset(6); return o != 0 ? obj.__assign(__vector(o), bb) : null; }
  public float a(int j) { int o = __offset(8); return o != 0 ? bb.getFloat(__vector(o) + j * 4) : 0; }
  public int aLength() { int o = __offset(8); return o != 0 ? __vector_len(o) : 0; }
  public ByteBuffer aAsByteBuffer() { return __vector_as_bytebuffer(8, 4); }
  public ByteBuffer bAsByteBuffer() { return __vector_as_bytebuffer(10, 4); }

  public static int createColumns(FlatBufferBuilder builder,
      long base,
      int keysOffset,
      int aOffset,
      int bOffset) {
    builder.startTable(4);
    Columns.addBase(builder, base);
    Columns.addB(builder, bOffset);
    Columns.addA(builder, aOffset);
    Columns.addKeys(builder, keysOffset);
    return Columns.endColumns(builder);
  }

  public static void addBase(FlatBufferBuilder builder, long base) { builder.addLong(0, base, 0L); }
  public static void addKeys(FlatBufferBuilder builder, int keysOffset) { builder.addOffset(1, keysOffset, 0); }
  public static void startKeysVector(FlatBufferBuilder builder, int numElems) { builder.startVector(4, numElems, 4); }
  public static void addA(FlatBufferBuilder builder, int aOffset) { builder.addOffset(2, aOffset, 0); }
  public static int createAVector(FlatBufferBuilder builder, float[] data) { builder.startVector(4, data.length, 4); for (int i = data.length - 1; i >= 0; i--) builder.addFloat(data[i]); return builder.endVector(); }
  public static void addB(FlatBufferBuilder builder, int bOffset) { builder.addOffset(3, bOffset, 0); }
  public static int createBVector(FlatBufferBuilder builder, float[] data) { builder.startVector(4, data.length, 4); for (int i = data.length - 1; i >= 0; i--) builder.addFloat(data[i]); return builder.endVector(); }
  public static int endColumns(FlatBufferBuilder builder) {
    int o = builder.endTable();
    return o;
  }
}
