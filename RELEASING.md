# Releasing buffy

buffy's four runtimes are released together, with one version, from a tag
`vX.Y.Z`:

| Language   | Package                               | Registry      | Directory         |
| ---------- | ------------------------------------- | ------------- | ----------------- |
| TypeScript | `@buffy/core`                         | npm           | `typescript/core` |
| Python     | `buffy-core`                          | PyPI          | `python/core`     |
| Java       | `io.github.snovader.buffy:buffy-core` | Maven Central | `java/core`       |
| Rust       | `buffy-core`                          | crates.io     | `rust/core`       |

`scripts/set-version` sets the version in every package's manifest, and in
the lockfiles that record it. Pushing the tag starts
`.github/workflows/release.yml`, which publishes the packages.

## The release workflow

The workflow runs these jobs:

1. **Verify** checks that every manifest has the tag's version, with
   `scripts/set-version --check`, and waits for the TypeScript, Python, Java
   and Rust workflows to pass on the tag's commit. They run on every push to
   `main`, and run their jobs when the push changes their language, as the
   commit that sets the version does in every language. A tag on a commit
   that isn't on `main` fails here.
2. **Build for npm** and **Build for PyPI** pack the packages, check them,
   and upload them for the jobs that publish them, so that those build
   nothing. `cargo publish` and the Gradle plugin build what they publish,
   so the crates.io and Maven Central jobs build their packages themselves.
3. **Publish to npm**, **Publish to PyPI**, **Publish to crates.io** and
   **Publish to Maven Central** each run in a GitHub environment of their
   own, `npm`, `pypi`, `crates-io` and `maven-central`, with only the
   permissions they need. npm, PyPI and crates.io authenticate the job with
   trusted publishing, which exchanges the job's OpenID Connect token for a
   short-lived one, so they need no secrets once they're set up. Maven
   Central needs a token and a signing key.
4. **Create the GitHub release** runs once every package is published, and
   writes notes that link them, followed by GitHub's notes of the pull
   requests since the last release.

A publishing job skips a version that its registry already has, so a job
that failed can be run again from the workflow run's page. Registries don't
let a version be published twice, and most don't let one be deleted, so a
release that was wrong is fixed by the next version.

## One-time setup

Do these in order. The steps for npm and crates.io finish after the first
release, since their trusted publishing is set up for a package that exists.
Each registry's steps link its documentation, which may have changed since
this was written.

### GitHub

1. Create the environments `npm`, `pypi`, `crates-io` and `maven-central`
   under the repository's Settings → Environments
   ([documentation](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/manage-environments)).
   For each:
   - Under **Deployment branches and tags**, choose **Selected branches and
     tags** and add the tag rule `v*`, so that only the release workflow's
     tags can deploy to it.
   - Optionally, add yourself as a **Required reviewer**, so that each
     registry waits for your approval in the workflow run. Leave **Prevent
     self-review** off, or you can't approve your own releases.
2. Optionally, protect the tags `v*` with a tag ruleset under Settings →
   Rules → Rulesets, which restricts who can create, update and delete them
   ([documentation](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/creating-rulesets-for-a-repository)).

The environments hold these secrets, as environment secrets:

| Environment     | Secret                               | Value                                            |
| --------------- | ------------------------------------ | ------------------------------------------------ |
| `npm`           | `NPM_TOKEN`                          | An npm access token, for the first version only  |
| `maven-central` | `MAVEN_CENTRAL_USERNAME`             | The username of a Central Portal user token      |
| `maven-central` | `MAVEN_CENTRAL_PASSWORD`             | The password of the same user token              |
| `maven-central` | `MAVEN_CENTRAL_SIGNING_KEY`          | The ASCII-armored secret key that signs releases |
| `maven-central` | `MAVEN_CENTRAL_SIGNING_KEY_PASSWORD` | The key's passphrase, or empty without one       |

### npm

npm's trusted publishing
([documentation](https://docs.npmjs.com/trusted-publishers)) needs npm
11.5.1 or later, which the workflow's Node 26 has, and is configured in a
package's settings, so it can't publish a package's first version. The
workflow publishes that one with `NPM_TOKEN` instead: npm tries trusted
publishing first and falls back to the token.

1. Sign in to npmjs.com, with two-factor authentication.
2. Create the organization `buffy`, on the free plan for public packages
   ([documentation](https://docs.npmjs.com/creating-an-organization)). Its
   name is the scope of `@buffy/core`; npm user and organization names
   share one namespace, so the name must be free.
3. Create a granular access token
   ([documentation](https://docs.npmjs.com/creating-and-viewing-access-tokens))
   with **Read and write (publish and stage)** permission for the `@buffy`
   scope, **Bypass two-factor authentication** checked, since the workflow
   can't answer a one-time password, and an expiry of a few days. Save it as
   `NPM_TOKEN` in the `npm` environment.
4. Release the first version, as below.
5. In `@buffy/core`'s settings on npmjs.com, add a trusted publisher for
   GitHub Actions: organization or user `snovader`, repository `buffy`,
   workflow filename `release.yml`, environment name `npm`, and allow
   `npm publish`. The workflow publishes with `npm publish`; a configuration
   that allows only `npm stage publish` rejects it. `npm trust github` sets
   the same from the command line.
6. Delete the `NPM_TOKEN` secret and revoke the token. Then, under the
   package's Settings → Publishing access, choose **Require two-factor
   authentication and disallow tokens**, which trusted publishing isn't
   affected by.

### PyPI

PyPI's trusted publishing can create the project on its first upload, from
a pending publisher
([documentation](https://docs.pypi.org/trusted-publishers/creating-a-project-through-oidc/)).

1. Sign in to pypi.org, with two-factor authentication, which PyPI requires.
2. Under your account's **Publishing** page, add a pending publisher for
   GitHub: PyPI project name `buffy-core`, owner `snovader`, repository
   name `buffy`, workflow name `release.yml`, environment name `pypi`.

A pending publisher doesn't reserve the name until it publishes, so add it
shortly before the first release. Once the project exists, it's an ordinary
trusted publisher of the project.

### crates.io

crates.io's trusted publishing
([documentation](https://crates.io/docs/trusted-publishing)) is configured
in a crate's settings, so the crate's first version is published by hand,
with an API token
([documentation](https://doc.rust-lang.org/cargo/reference/publishing.html)).

1. Sign in to crates.io with GitHub, and verify your email address under
   Account Settings, which publishing requires.
2. Create an API token under Account Settings → API Tokens, with the scope
   `publish-new`, the crate `buffy-core`, and an expiry of a few days.
3. Release the first version, as below, publishing `buffy-core` by hand
   before pushing the tag.
4. In `buffy-core`'s Settings → Trusted Publishing, add a GitHub
   configuration: repository owner `snovader`, repository name `buffy`,
   workflow filename `release.yml`, environment `crates-io`.
5. Revoke the API token.

### Maven Central

Maven Central publishes through the Central Portal, which Gradle reaches
with the `com.vanniktech.maven.publish` plugin
([documentation](https://vanniktech.github.io/gradle-maven-publish-plugin/central/)).
The Portal needs a token, and every file signed with a key whose public half
is on a key server
([requirements](https://central.sonatype.org/publish/requirements/)).

1. Sign in to [central.sonatype.com](https://central.sonatype.com) with the
   GitHub account `snovader`. The Portal verifies the namespace
   `io.github.snovader` for a GitHub sign-in
   ([documentation](https://central.sonatype.org/register/namespace/));
   check that it's listed as verified under **Namespaces**.
2. Generate a user token at
   [central.sonatype.com/usertoken](https://central.sonatype.com/usertoken)
   ([documentation](https://central.sonatype.org/publish/generate-portal-token/)),
   and save its username and password as `MAVEN_CENTRAL_USERNAME` and
   `MAVEN_CENTRAL_PASSWORD` in the `maven-central` environment. The Portal
   shows them only once.
3. Create a signing key
   ([documentation](https://central.sonatype.org/publish/requirements/gpg/)).
   Maven Central checks signatures of the primary key only, so create a
   key without subkeys, and publish its public key:

   ```sh
   gpg --quick-generate-key "snovader (buffy releases) <email>" rsa4096 sign 2y
   gpg --list-keys "buffy releases"  # prints the key's ID
   gpg --keyserver keyserver.ubuntu.com --send-keys <key ID>
   gpg --export-secret-keys --armor <key ID>
   ```

   Save the exported secret key, from `-----BEGIN PGP PRIVATE KEY BLOCK-----`
   to its end line, as `MAVEN_CENTRAL_SIGNING_KEY`, and its passphrase as
   `MAVEN_CENTRAL_SIGNING_KEY_PASSWORD`, in the `maven-central` environment.
   Before the key expires, extend it and send it to the key server again.

The job runs `./gradlew :core:publishAndReleaseToMavenCentral`, which
uploads the signed files, waits for the Portal to validate them, and
releases them. Maven Central lists them some minutes later. The build signs
only when it has the key, so `./gradlew publishToMavenLocal` works without
one, and refuses to publish to Maven Central unsigned.

## Making a release

1. On a branch from `main`, set the version and open a pull request:

   ```sh
   scripts/set-version X.Y.Z
   git commit -am "Release X.Y.Z"
   ```

2. Merge it once every language's CI passes.
3. Tag the commit that it made on `main`, and push the tag:

   ```sh
   git fetch origin
   git tag -a vX.Y.Z -m "buffy X.Y.Z" <the commit>
   git push origin vX.Y.Z
   ```

4. In the release workflow's run, approve each environment's deployment if
   it requires a reviewer, and check that every job passes.

A push to `main` cancels the CI that's still running for the commit before
it. If that cancels CI on the tagged commit, re-run that CI, and then the
release's failed jobs.

For the first release, set up `NPM_TOKEN` as the npm steps describe, and
between steps 2 and 3, publish `buffy-core` to crates.io by hand from the
merged commit:

```sh
git switch --detach <the commit>
cd rust
cargo login  # asks for the API token
cargo publish -p buffy-core --locked
```

The workflow's crates.io job then finds that version and skips it. Then
finish the npm and crates.io setup above, so that later releases use trusted
publishing alone.

Versions are `X.Y.Z`; the script and the workflow accept nothing else, since
the registries spell pre-releases differently.
