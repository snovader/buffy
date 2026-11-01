package io.github.snovader.tafla.benchmarks.codecs;

/** One library's encoding of one of the workloads' shapes */
public interface Workload<T> {
    /** {@return the input's bytes, in an array that the caller owns} */
    byte[] serialize(T input);

    /** Reads every field, with columns as primitive arrays */
    T deserialize(byte[] bytes);
}
