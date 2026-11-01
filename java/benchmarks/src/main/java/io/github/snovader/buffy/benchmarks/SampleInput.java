package io.github.snovader.buffy.benchmarks;

/**
 * A fixed-size record of a uint32 and two float32s. The uint32 is held in a {@code long}, as buffy
 * reads it.
 */
public record SampleInput(long key, float a, float b) {}
