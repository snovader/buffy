# Security

buffy's readers decode bytes from untrusted sources. On malformed input, any
of these is a vulnerability:

- a read outside the bytes it was given
- an error other than the implementation's decode error, a crash or a panic
- a hang, or an allocation out of proportion to the input
- implementations reading the same bytes as different values

## Supported versions

The latest release of each package receives security fixes.

## Reporting a vulnerability

Report it privately through the repository's Security tab, under "Report a
vulnerability", rather than in a public issue. Include the bytes, the
implementation and version, and what happened.
