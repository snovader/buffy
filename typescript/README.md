# tafla for TypeScript

`core/` holds `tafla`, the runtime. Its tests also generate the
repository's conformance suite. `benchmarks/` times it, and compares it with
other libraries; its README describes how.

## Development

This is a Yarn project with its release in `.yarn/releases`, so `yarn` runs the
pinned version from here. Run the commands below from this directory.

```sh
yarn install
yarn format:check
yarn lint:check
yarn typecheck
yarn test
```

`yarn format:write` and `yarn lint:write` fix what they can.
`yarn conformance:generate` regenerates the conformance suite at the root of the
repository from `core/test/conformance/cases.ts`, and the tables of the
generated code in its `generated` directory from
`core/test/generated/shapesCases.ts`.

`core/test/generated` holds the TypeScript and JavaScript that tafla-gen
generates from the example schemas in `schemas/`, in a directory of each
schema, and the tests that run them, of the JavaScript through its JSDoc
types. They're the generator's golden files, which `rust/gen/README.md`
describes: rewrite them with the generator rather than by hand.

Tests run in Node's own module loader, which strips the types of `.ts` files.
CI runs them in Node 24 and 26.

## Source and build

Within this project, the `tafla-source` condition resolves `tafla` and
its `#` imports to its TypeScript source: tests pass it to Node, and the
TypeScript configs set it in `customConditions`. Without it, as for the
package's users, they resolve to the build in `dist`.

`yarn workspace tafla build` compiles the package to `dist`, and packing
it builds it first. The package holds only `dist`, so its `publishConfig`
replaces `exports` and `imports` with copies without the condition, which
Yarn writes into the packed manifest. Pack and publish it with Yarn, since npm
doesn't apply them. `yarn package:check` packs it, checks the archive with Are
the Types Wrong and publint, and imports the build to read back a table.
