// Written by hand in the shape that flatc 25.2.10 generates with `--java` from
// src/main/flatbuffers/workloads.fbs, with only the accessors and builders that the benchmarks use

package io.github.snovader.tafla.benchmarks.generated.flatbuffers;

import com.google.flatbuffers.FlatBufferBuilder;
import com.google.flatbuffers.Table;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;

@SuppressWarnings("unused")
public final class Record extends Table {
  public static Record getRootAsRecord(ByteBuffer _bb) { return getRootAsRecord(_bb, new Record()); }
  public static Record getRootAsRecord(ByteBuffer _bb, Record obj) { _bb.order(ByteOrder.LITTLE_ENDIAN); return (obj.__assign(_bb.getInt(_bb.position()) + _bb.position(), _bb)); }
  public void __init(int _i, ByteBuffer _bb) { __reset(_i, _bb); }
  public Record __assign(int _i, ByteBuffer _bb) { __init(_i, _bb); return this; }

  public double x() { int o = __offset(4); return o != 0 ? bb.getDouble(o + bb_pos) : 0.0; }
  public double y() { int o = __offset(6); return o != 0 ? bb.getDouble(o + bb_pos) : 0.0; }
  public long id() { int o = __offset(8); return o != 0 ? bb.getLong(o + bb_pos) : 0L; }
  public String name() { int o = __offset(10); return o != 0 ? __string(o + bb_pos) : null; }

  public static int createRecord(FlatBufferBuilder builder,
      double x,
      double y,
      long id,
      int nameOffset) {
    builder.startTable(4);
    Record.addId(builder, id);
    Record.addY(builder, y);
    Record.addX(builder, x);
    Record.addName(builder, nameOffset);
    return Record.endRecord(builder);
  }

  public static void addX(FlatBufferBuilder builder, double x) { builder.addDouble(0, x, 0.0); }
  public static void addY(FlatBufferBuilder builder, double y) { builder.addDouble(1, y, 0.0); }
  public static void addId(FlatBufferBuilder builder, long id) { builder.addLong(2, id, 0L); }
  public static void addName(FlatBufferBuilder builder, int nameOffset) { builder.addOffset(3, nameOffset, 0); }
  public static int endRecord(FlatBufferBuilder builder) {
    int o = builder.endTable();
    return o;
  }
}
