/**
 * A comparison of buffy with other Java libraries that encode small tables and numeric columns, on
 * the same generic table shapes and values as the TypeScript comparison. {@link
 * io.github.snovader.buffy.benchmarks.Workloads} generates the inputs, and each library's
 * {@link io.github.snovader.buffy.benchmarks.codecs.Codec} serializes and deserializes them.
 */
@NullMarked
package io.github.snovader.buffy.benchmarks;

import org.jspecify.annotations.*;
