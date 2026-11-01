# buffy for TypeScript

`core/` holds `@buffy/core`, the runtime. Its tests also generate the
repository's conformance suite.

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
