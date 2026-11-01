import org.jspecify.annotations.*;

@NullMarked
open module io.github.snovader.buffy.test {
    requires io.github.snovader.buffy;
    requires jakarta.json;
    requires net.jqwik.api;
    requires org.junit.jupiter.api;
    requires static org.jspecify;
}
