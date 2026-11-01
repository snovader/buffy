package io.github.snovader.tafla.benchmarks.jmh;

import io.github.snovader.tafla.benchmarks.*;
import io.github.snovader.tafla.benchmarks.codecs.*;
import org.openjdk.jmh.annotations.*;

/**
 * Documents of 100 and 100,000 rows: a sized, versioned table of two float64s and a string, with a
 * nested table of an int64 and three columns, a uint32 and two float32s
 */
@State(Scope.Benchmark)
public class DocumentBenchmark {
    @Param({"tafla", "flatbuffers", "protobuf", "msgpack"})
    public String codec;

    @Param({"100", "100000"})
    public int rows;

    private DocumentInput input;
    private DocumentWorkload workload;
    private byte[] bytes;

    @Setup
    public void setUp() {
        input = Workloads.documentInput(rows);
        workload = Codecs.named(codec).document();
        bytes = Codecs.serializeChecked(workload, input);
    }

    @Benchmark
    public byte[] serialize() {
        return workload.serialize(input);
    }

    @Benchmark
    public DocumentInput deserialize() {
        return workload.deserialize(bytes);
    }

    /** Sums column {@code a} through the library's per-element accessor */
    @Benchmark
    public double sumA() {
        return workload.sumA(bytes);
    }
}
