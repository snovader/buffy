# buffy for Java

`core/` holds `io.github.snovader.buffy:buffy-core`, the runtime, whose module
and package are `io.github.snovader.buffy`. Its package documentation has an
example.

`benchmarks/` compares buffy with FlatBuffers, protobuf, MessagePack and SBE
in JMH benchmarks, on the same workloads as the TypeScript comparison. Its
[README](benchmarks/README.md) describes them and has results.

## Development

This is a Gradle build. It compiles for Java 21 with an Oracle JDK 25, which
Gradle downloads when it isn't installed, and checks the code with Error Prone
and NullAway. Run the commands below from this directory.

```sh
./gradlew build
./gradlew javadoc
```

`./gradlew test -PtestJavaVersion=21` runs the tests in Java 21 rather than 25.
`./gradlew build` also checks that every codec in the benchmarks reads back
what it wrote, and `./gradlew :benchmarks:jmh` runs the benchmarks.

The test module holds the Java that buffy-gen generates from the example
schemas in `schemas/`, in a package of each schema,
`io.github.snovader.buffy.test.generated.<schema>`, which its tests run.
They're the generator's golden files, which `rust/gen/README.md` describes:
rewrite them with the generator rather than by hand.
