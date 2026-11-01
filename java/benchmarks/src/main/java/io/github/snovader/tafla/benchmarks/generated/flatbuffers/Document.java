// Written by hand in the shape that flatc 25.2.10 generates with `--java` from
// src/main/flatbuffers/workloads.fbs, with only the accessors and builders that the benchmarks use

package io.github.snovader.tafla.benchmarks.generated.flatbuffers;

import com.google.flatbuffers.FlatBufferBuilder;
import com.google.flatbuffers.Table;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;

@SuppressWarnings("unused")
public final class Document extends Table {
  public static Document getRootAsDocument(ByteBuffer _bb) { return getRootAsDocument(_bb, new Document()); }
  public static Document getRootAsDocument(ByteBuffer _bb, Document obj) { _bb.order(ByteOrder.LITTLE_ENDIAN); return (obj.__assign(_bb.getInt(_bb.position()) + _bb.position(), _bb)); }
  public void __init(int _i, ByteBuffer _bb) { __reset(_i, _bb); }
  public Document __assign(int _i, ByteBuffer _bb) { __init(_i, _bb); return this; }

  public double x() { int o = __offset(4); return o != 0 ? bb.getDouble(o + bb_pos) : 0.0; }
  public double y() { int o = __offset(6); return o != 0 ? bb.getDouble(o + bb_pos) : 0.0; }
  public String name() { int o = __offset(8); return o != 0 ? __string(o + bb_pos) : null; }
  public Columns columns() { return columns(new Columns()); }
  public Columns columns(Columns obj) { int o = __offset(10); return o != 0 ? obj.__assign(__indirect(o + bb_pos), bb) : null; }

  public static int createDocument(FlatBufferBuilder builder,
      double x,
      double y,
      int nameOffset,
      int columnsOffset) {
    builder.startTable(4);
    Document.addY(builder, y);
    Document.addX(builder, x);
    Document.addColumns(builder, columnsOffset);
    Document.addName(builder, nameOffset);
    return Document.endDocument(builder);
  }

  public static void addX(FlatBufferBuilder builder, double x) { builder.addDouble(0, x, 0.0); }
  public static void addY(FlatBufferBuilder builder, double y) { builder.addDouble(1, y, 0.0); }
  public static void addName(FlatBufferBuilder builder, int nameOffset) { builder.addOffset(2, nameOffset, 0); }
  public static void addColumns(FlatBufferBuilder builder, int columnsOffset) { builder.addOffset(3, columnsOffset, 0); }
  public static int endDocument(FlatBufferBuilder builder) {
    int o = builder.endTable();
    return o;
  }
}
