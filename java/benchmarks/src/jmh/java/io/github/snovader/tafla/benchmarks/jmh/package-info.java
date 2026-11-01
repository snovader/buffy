/**
 * JMH benchmarks of each workload, with a parameter for the codec. JMH runs each codec in its own
 * forked JVMs, so that the JIT compiles a codec's call sites without the others' type profiles.
 */
@NullMarked
package io.github.snovader.tafla.benchmarks.jmh;

import org.jspecify.annotations.*;
