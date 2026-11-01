import org.jspecify.annotations.*;

@NullMarked
open module io.github.snovader.tafla.test {
    requires io.github.snovader.tafla;
    requires jakarta.json;
    requires net.jqwik.api;
    requires org.junit.jupiter.api;
    requires static org.jspecify;
}
