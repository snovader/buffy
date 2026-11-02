package io.github.snovader.tafla.benchmarks.codecs;

import io.github.snovader.tafla.benchmarks.*;

import java.util.*;

/** The libraries that the comparison runs, with tafla first as the reference */
public final class Codecs {
    /** The name of SBE's encodings of the fixed-size record and of the array of structs */
    public static final String SBE = "sbe";

    private Codecs() {}

    /** {@return a new codec of each library that encodes every workload} */
    public static List<Codec> all() {
        return List.of(
            TaflaCodec.create(),
            TaflaGenCodec.create(),
            FlatBuffersCodec.create(),
            ProtobufCodec.create(),
            MessagePackCodec.create()
        );
    }

    /**
     * {@return a new codec of the library}
     *
     * @throws IllegalArgumentException if no codec has that name
     */
    public static Codec named(String name) {
        return all().stream()
            .filter(codec -> codec.name().equals(name))
            .findFirst()
            .orElseThrow(() -> new IllegalArgumentException("No codec is named " + name));
    }

    /**
     * {@return new encodings of the fixed-size record by name: every codec's, and SBE's, which
     * encodes only the fixed-size record and the array of structs}
     */
    public static SequencedMap<String, Workload<SampleInput>> samples() {
        var samples = new LinkedHashMap<String, Workload<SampleInput>>();
        for (var codec : all()) {
            samples.put(codec.name(), codec.sample());
        }
        samples.put(SBE, new SbeSampleWorkload());

        return samples;
    }

    /**
     * {@return a new encoding of the fixed-size record by a codec or by SBE}
     *
     * @throws IllegalArgumentException if no codec has that name
     */
    public static Workload<SampleInput> sample(String name) {
        var sample = samples().get(name);
        if (sample == null) {
            throw new IllegalArgumentException("No codec is named " + name);
        }

        return sample;
    }

    /**
     * {@return new encodings of the array of structs by name: every codec's, and SBE's, which
     * encodes it as a repeating group}
     */
    public static SequencedMap<String, Workload<ListInput>> structs() {
        var structs = new LinkedHashMap<String, Workload<ListInput>>();
        for (var codec : all()) {
            structs.put(codec.name(), codec.structs());
        }
        structs.put(SBE, new SbeStructsWorkload());

        return structs;
    }

    /**
     * {@return a new encoding of the array of structs by a codec or by SBE}
     *
     * @throws IllegalArgumentException if no codec has that name
     */
    public static Workload<ListInput> structs(String name) {
        var structs = structs().get(name);
        if (structs == null) {
            throw new IllegalArgumentException("No codec is named " + name);
        }

        return structs;
    }

    /**
     * Serializes the input after checking that the workload reads back exactly what it wrote, so
     * that a broken codec isn't timed.
     *
     * @throws IllegalStateException if it doesn't
     */
    public static <T> byte[] serializeChecked(Workload<T> workload, T input) {
        var bytes = workload.serialize(input);
        var output = workload.deserialize(bytes);

        if (!output.equals(input)) {
            throw new IllegalStateException(
                String.format("%s read back %s as %s", workload.getClass().getName(), input, output)
            );
        }

        return bytes;
    }
}
