package io.github.snovader.buffy.benchmarks;

/** A small table: two float64s, an int64 and a 16-byte string */
public record RecordInput(double x, double y, long id, String name) {}
