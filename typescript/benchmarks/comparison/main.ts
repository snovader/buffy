import { tafla } from './codecs/tafla.ts';
import { flatbuffers } from './codecs/flatbuffers.ts';
import { msgpackr } from './codecs/msgpackr.ts';
import { protobufjs } from './codecs/protobuf.ts';
import { runSuite } from './suite.ts';

const args = process.argv.slice(2);
const rounds = Number(option('--rounds') ?? 31);

console.log(
    `Node ${process.version}, V8 ${process.versions.v8}; minimum of ${rounds} interleaved rounds`
);

runSuite([tafla, flatbuffers, protobufjs, msgpackr], {
    rounds,
    warmupRounds: 10,
    sampleTime: Number(option('--sample-ms') ?? 1),
    filter: new RegExp(option('--filter') ?? ''),
    print: line => console.log(line)
});

function option(name: string): string | undefined {
    const index = args.indexOf(name);
    return index === -1 ? undefined : args[index + 1];
}
