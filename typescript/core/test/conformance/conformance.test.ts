import { readdir, readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import { DecodeError } from 'tafla';
import { SUITE } from './cases.ts';
import type { Suite } from './description.ts';
import { SUITE_DIRECTORY, writeSuite } from './files.ts';
import { buildTable, readTable, TableReader } from './tables.ts';

const directory = SUITE_DIRECTORY;

// The checked-in suite, which the other implementations of tafla load too
const json = await readFile(new URL('cases.json', directory), 'utf8');
const suite: Suite = JSON.parse(json);
const tables = await readTables();

async function readTables(): Promise<Map<string, Uint8Array>> {
    const tables = new Map<string, Uint8Array>();

    for (const subdirectory of ['valid/', 'invalid/']) {
        for (const name of await readdir(new URL(subdirectory, directory))) {
            const path = `${subdirectory}${name}`;
            // A plain Uint8Array rather than a Buffer, to compare with tafla's
            tables.set(path, new Uint8Array(await readFile(new URL(path, directory))));
        }
    }

    return tables;
}

function bytesOf(file: string): Uint8Array {
    const bytes = tables.get(file);
    if (bytes === undefined) {
        throw new Error(`Missing ${file}`);
    }

    return bytes;
}

describe('the checked-in suite (run `yarn conformance:generate` to update it)', () => {
    const generated = writeSuite(SUITE);

    it('should describe the cases that the generator writes', () => {
        expect(json).toBe(generated.json);
    });

    it('should hold the tables that the generator writes', () => {
        expect(new Set(tables.keys())).toStrictEqual(new Set(generated.tables.keys()));

        for (const [path, bytes] of generated.tables) {
            expect(tables.get(path), path).toStrictEqual(bytes);
        }
    });
});

describe('valid cases', () => {
    it.each(suite.valid)('should read $name', ({ file, table }) => {
        expect(readTable(bytesOf(file), table)).toStrictEqual(table);
    });

    it.each(suite.valid)('should rebuild $name byte for byte', ({ file, table }) => {
        expect(buildTable(table).bytes()).toStrictEqual(bytesOf(file));
    });
});

describe('invalid cases', () => {
    it.each(suite.invalid)(
        'should read $name up to field $field and then throw DecodeError',
        ({ file, case: name, field }) => {
            const { table } = suite.valid.find(validCase => validCase.name === name)!;
            const reader = new TableReader(bytesOf(file), table.layout);

            for (let index = 0; index < field; index++) {
                const read = reader.readRootField(index, table.fields[index]);
                expect(read).toStrictEqual(table.fields[index]);
            }

            expect(() => reader.readRootField(field, table.fields[field])).toThrow(DecodeError);
        }
    );
});
