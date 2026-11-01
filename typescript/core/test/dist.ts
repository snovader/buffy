// Imports the build as a dependent package would, without the source
// condition, and reads back a table it wrote. `yarn package:check` runs it.
import { ByteBuffer, TableBuilder } from 'tafla';

const builder = new TableBuilder(1);
builder.setString('tafla');
const buffer = new ByteBuffer(builder.bytes());

if (buffer.getString(buffer.rootOffset(0)) !== 'tafla') {
    throw new Error('The build read back another string');
}
