# buffy for TypeScript

`core/` holds `@buffy/core`, the runtime. Its tests also generate the
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
repository from `core/test/conformance/cases.ts`.

Tests run in Node's own module loader, which strips the types of `.ts` files.
CI runs them in Node 24 and 26.

## Source and build

Within this project, the `@buffy/source` condition resolves `@buffy/core` and
its `#` imports to its TypeScript source: tests pass it to Node, and the
TypeScript configs set it in `customConditions`. Without it, as for the
package's users, they resolve to the build in `dist`.

`yarn workspace @buffy/core build` compiles the package to `dist`, and packing
it builds it first. `yarn package:check` packs it, checks the archive with Are
the Types Wrong and publint, and imports the build to read back a table.
