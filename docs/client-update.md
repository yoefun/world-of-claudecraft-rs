# Client update packages (Linux x86_64)

Packaged Bevy clients ship as a signed full archive plus an optional bsdiff delta from the immediately previous release. Players run `woc-updater`; the title screen can launch it when the realm reports a newer rewrite via `/version`.

## First install

1. Download `woc-rs-VERSION-x86_64-unknown-linux-gnu.tar.zst` from the GitHub Release for your rewrite version (for example `woc-rs-1.5.0-x86_64-unknown-linux-gnu.tar.zst`).
2. Extract the archive into a directory of your choice.
3. Run `./woc-updater` to launch `./woc-client` (no download when already current).

The archive contains `woc-client`, `woc-updater`, `install.json`, and the runtime
asset closure under `assets/` listed in [`assets/runtime-manifest.txt`](../assets/runtime-manifest.txt).

## Subsequent updates

From the same install prefix, run `./woc-updater`.

With no arguments, the updater uses the directory that contains the binary as `--prefix` and launches `./woc-client` (no download). To apply an update, pass the signed manifest URL (the title **Update** button does this) or set `WOC_UPDATE_MANIFEST_URL`:

```bash
./woc-updater --manifest https://github.com/yoefun/world-of-claudecraft-rs/releases/download/v1.5.0/woc-rs-1.5.0-x86_64-unknown-linux-gnu.manifest.json
```

- When your installed rewrite is **N−1** and a delta exists on the release, the updater downloads the smaller `.wocdelta` artifact.
- When you skipped a version or no delta was published (first release tag), it downloads the full `.tar.zst` instead.
- If the runtime asset closure changed, the release publishes a full archive without a binary delta; this keeps asset additions, removals, and replacements atomic.
- If a delta fails hash or apply checks, the updater retries once with the full archive.

After a successful apply, `woc-updater` execs `./woc-client`.

## Signing keys (CI secrets)

Release packing uses ed25519. Generate a key pair once:

```bash
cargo run -p woc-update --bin woc-pack -- --gen-key
```

This prints two lines and exits:

```text
seed <64-hex-chars>    # 32-byte signing seed — CI secret only
pubkey <64-hex-chars>  # 32-byte public key — baked into the updater at compile time
```

Configure GitHub Actions repository secrets:

| Secret | Purpose |
| --- | --- |
| `WOC_UPDATE_SIGNING_KEY` | 32-byte seed hex (`woc-pack --key` in `client-release.yml`) |
| `WOC_UPDATE_PUBKEY` | 32-byte public key hex (compile-time `env!("WOC_UPDATE_PUBKEY")` for `woc-updater`) |

Never commit the signing seed. Dev/unit tests use ephemeral keys.

## Server manifest URL

Set `WOC_UPDATE_MANIFEST_URL` on **`woc-server`** to the HTTPS URL of the uploaded manifest on the GitHub Release, for example:

```text
https://github.com/yoefun/world-of-claudecraft-rs/releases/download/v1.5.0/woc-rs-1.5.0-x86_64-unknown-linux-gnu.manifest.json
```

`GET /version` then includes `update_manifest_url`. When the packaged client is incompatible with the realm, the title screen shows **Update** and execs the sibling `woc-updater` with `--once --manifest <url>`.

For local `cargo run -p woc-client` development, leave `WOC_UPDATE_MANIFEST_URL` unset (empty default).

## CI publish

Tag pushes matching `v*.*.*` run [`.github/workflows/client-release.yml`](../.github/workflows/client-release.yml): release-build `woc-client` and `woc-updater`, stage a layout, optionally unpack the previous release for a delta, run `woc-pack`, and upload assets to the GitHub Release.

Design: [`docs/superpowers/specs/2026-08-13-client-update-packages-design.md`](superpowers/specs/2026-08-13-client-update-packages-design.md)
