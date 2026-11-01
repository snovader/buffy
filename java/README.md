# buffy for Java

`core/` holds `io.github.snovader.buffy:buffy-core`, the runtime, whose module
and package are `io.github.snovader.buffy`. Its package documentation has an
example.

## Development

This is a Gradle build. It compiles for Java 21 with an Oracle JDK 25, which
Gradle downloads when it isn't installed, and checks the code with Error Prone
and NullAway. Run the commands below from this directory.

```sh
./gradlew build
./gradlew javadoc
```

`./gradlew test -PtestJavaVersion=21` runs the tests in Java 21 rather than 25.
