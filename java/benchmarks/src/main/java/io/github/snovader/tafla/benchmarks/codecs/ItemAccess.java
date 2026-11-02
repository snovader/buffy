package io.github.snovader.tafla.benchmarks.codecs;

/**
 * A way to read the items of tafla's vector of nested tables and of its array of structs, which
 * tafla and tafla-gen write alike: the last item alone, which a reader can find without reading the
 * others, and every item. Each sums the three float32s of the items it reads as doubles, as the
 * TypeScript comparison adds them, so that it returns a number rather than creating objects of its
 * own.
 */
public interface ItemAccess {
    double lastTable(byte[] bytes);

    double everyTable(byte[] bytes);

    double lastStruct(byte[] bytes);

    double everyStruct(byte[] bytes);
}
