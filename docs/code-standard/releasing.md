# Releasing

A release is cut by pushing a tag. Everything else — cross-compiling five
targets, generating installers, updating the Homebrew formula — is automated.
The two steps that are **not** automated are the crates.io publishes, and that
is deliberate.

## Where the configuration lives

| File | Role |
| ---- | ---- |
| `dist-workspace.toml` | All `dist` settings, in a `[dist]` table |
| `Cargo.toml` → `[profile.dist]` | Build profile used for release binaries (`inherits = "release"`, `lto = "thin"`) |
| `.github/workflows/release.yml` | **Generated** by `dist generate` — do not hand-edit; regenerate instead |

`dist` v0.32.0 writes a dedicated `dist-workspace.toml` rather than a
`[workspace.metadata.dist]` table in `Cargo.toml`. The version key is
`cargo-dist-version`, not `dist-version`, even though the binary is now
invoked as `dist`.

```toml
[dist]
cargo-dist-version = "0.32.0"   # pinned so CI cannot drift onto a newer dist
ci = "github"                   # a bare string, not an array
installers = ["shell", "powershell", "homebrew"]
tap = "buiducnhat/homebrew-tap"
publish-jobs = ["homebrew"]
targets = [...]                 # five targets, see below
hosting = "github"
install-path = "CARGO_HOME"
install-updater = false
```

**`publish-jobs = ["homebrew"]` is the setting most easily lost.** With only
`installers` set, `dist` builds a formula and never pushes it: the release
looks complete while the Homebrew channel stays dead. If `brew install` starts
failing after a release, check this key first.

`homepage` must be set in `[workspace.package]` — the generated formula carries
a homepage field, and `dist` warns without it.

After editing `dist-workspace.toml`, run `dist init -y --hosting github` to
regenerate the workflow. It is safe to re-run; it preserves settings and
normalizes the file.

## Toolchain

`rust-toolchain.toml` pins the compiler to the MSRV (`1.97`) with the `rustfmt`
and `clippy` components, for local builds, CI, and release builds alike.

It is load-bearing, not cosmetic. `dist`'s generated workflow installs Rust
**only inside containers** — on native macOS and Windows runners it uses
whatever the runner image ships. The v0.1.0 release failed the first time
because the `aarch64-apple-darwin` image carried rustc 1.96.0, below the
declared MSRV, while every Linux target passed precisely because those build
in containers with a freshly installed toolchain.

Pinned to the MSRV rather than `stable` on purpose: `stable` means "whatever
that runner happens to have", and a stale one is exactly what broke the first
attempt. Pinning also makes CI verify the minimum version the crates
advertise, instead of only ever testing a newer compiler.

**If you raise `rust-version` in `Cargo.toml`, raise `channel` here too.** They
are two halves of one claim, and a release build is where they diverging shows
up.

## Targets

Five targets are built:

- `aarch64-apple-darwin`
- `x86_64-apple-darwin`
- `aarch64-unknown-linux-gnu`
- `x86_64-unknown-linux-gnu`
- `x86_64-pc-windows-msvc`

`aarch64-unknown-linux-gnu` is the only one needing real cross-compilation and
therefore the likeliest to break. None have been dropped. If it ever fails to
build, drop it and ship the other four rather than blocking the release — and
say so in the release notes instead of quietly shipping four where five were
promised.

## The `HOMEBREW_TAP_TOKEN` secret

The Homebrew publish job checks out `buiducnhat/homebrew-tap`, commits
`Formula/lazytools.rb`, and pushes. It authenticates with a repository secret
named exactly `HOMEBREW_TAP_TOKEN`.

- The secret lives on the **publishing** repo, `buiducnhat/lazytools`.
- The **permissions** apply to the **tap** repo, `buiducnhat/homebrew-tap`.
  That inversion is easy to get backwards.

For a fine-grained token: repository access limited to `homebrew-tap`, with
**Contents: Read and write** (plus the mandatory Metadata: Read-only). Nothing
else — the job never touches the tap's `.github/workflows/`, so no Workflows
permission is needed. A classic token uses the `repo` scope.

Fine-grained tokens expire. **When it expires, releases still go green while
the tap silently stops updating.** If `brew install` serves a stale version
after a successful release, check the token before anything else.

The tap repository must exist *and* have at least one commit — `actions/checkout`
cannot check out a repository with no branch. A newly created empty repo needs
a README committed before the first release.

## Pre-flight

```sh
dist plan                                        # previews every artifact, builds nothing
cargo publish --dry-run -p lazytools-core --locked
cargo package -p lazytools --no-verify --list    # confirm LICENSE + README are included
cargo test --workspace
```

`dist plan` is the cheap check: it turns "are the targets configured right?"
from a 20-minute CI question into a seconds-long local one.

`cargo publish --dry-run -p lazytools` **will fail** with `no matching package
named lazytools-core` whenever the new core version is not yet on crates.io.
That is expected, not a defect — see the publish ordering below.

## Release sequence

1. Bump the version in **both** `crates/lazytools/Cargo.toml` and
   `crates/lazytools-core/Cargo.toml`, and in the `version` of the
   `lazytools-core` entry under `[workspace.dependencies]`.
2. Confirm all three agree, and that the tree is clean and pushed.
3. Run the pre-flight checks above.
4. Tag and push:

   ```sh
   git tag vX.Y.Z
   git push origin vX.Y.Z
   gh run watch
   ```

   `dist` builds every target, creates the GitHub Release with checksums and
   installer scripts, and pushes the updated formula to the tap. Expect a long
   run — it cross-compiles.

5. Verify the binary channels actually install, rather than assuming:

   ```sh
   gh release view vX.Y.Z --json assets --jq '.assets[].name'
   gh api repos/buiducnhat/homebrew-tap/contents/Formula --jq '.[].name'
   brew install buiducnhat/tap/lazytools && lazytools --version
   ```

6. **Only then**, publish to crates.io — in this order, one at a time:

   ```sh
   cargo publish -p lazytools-core --locked
   # wait for the index to catch up (usually under a minute), confirm it is live:
   curl -s -A "your-name (your@email)" https://crates.io/api/v1/crates/lazytools-core
   cargo publish -p lazytools --locked
   ```

### Why core publishes before the binary

`lazytools` depends on `lazytools-core`. When the binary crate is published,
Cargo resolves that dependency **from the registry**, not from the local path —
so the matching core version must already be indexed. Publishing them in the
wrong order fails outright; publishing them simultaneously races the index.

### Why crates.io goes last

Every other channel is revertible. A tag can be deleted, a GitHub Release can
be deleted, a tap commit can be reverted. **A crates.io version can be yanked
but never reused or replaced** — `0.1.0` can never again mean different bytes.
So crates.io is sequenced after the binary channels have already proven the
build works, which turns an irreversible step into a formality.

This asymmetry is the reason for the whole ordering. When in doubt about
anything, resolve it *before* step 6.

## Querying crates.io

Requests without an identifying `User-Agent` are rejected by the crates.io
data-access policy with a message that reads like an outage — easy to
misread as "the crate name is taken":

```sh
curl -s -A "lazytools-release (you@example.com)" https://crates.io/api/v1/crates/lazytools
```

## Note on the local path dependency

`[workspace.dependencies]` declares `lazytools-core` with **both** `path` and
`version`. Cargo builds locally from the path and publishes the version.
Dropping the `version` key makes the crate unpublishable — crates.io rejects
bare path dependencies.
