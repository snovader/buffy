import org.jspecify.annotations.*;

/**
 * buffy's Java runtime, which writes and reads binary tables of scalars, strings and arrays. Its
 * package, {@link io.github.snovader.buffy}, describes the format and has an example.
 */
@NullMarked
module io.github.snovader.buffy {
    requires static org.jspecify;

    exports io.github.snovader.buffy;
}
