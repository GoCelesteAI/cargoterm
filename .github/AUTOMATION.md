# Release automation

This document explains how cargoterm releases flow through CI, and the one-time setup required to enable automatic Homebrew-tap bumps.

## What runs on each tag push

Pushing a tag matching `v*` (e.g. `v0.5.0`) triggers `.github/workflows/release.yml`:

1. **`build`** — matrix job. Builds release binaries for three targets on GitHub-hosted runners, packages each as `cargoterm-<version>-<target>.tar.gz` with its SHA-256 sidecar, uploads as workflow artifacts.
2. **`release`** — downloads all artifacts and publishes a GitHub Release with auto-generated notes from commits since the previous tag.
3. **`bump-tap`** — *opt-in.* If `HOMEBREW_TAP_TOKEN` is configured, clones `GoCelesteAI/homebrew-tap`, rewrites `Formula/cargoterm.rb` with the new version + URLs + SHAs, and pushes a commit titled `cargoterm <version>`. Otherwise prints a skip notice and exits clean — the release itself is unaffected.

## Enabling automatic tap bumps

You only need to do this once.

1. **Create a fine-grained Personal Access Token**
   - Go to https://github.com/settings/personal-access-tokens/new
   - **Resource owner:** `GoCelesteAI`
   - **Repository access:** Only select repositories → `GoCelesteAI/homebrew-tap`
   - **Repository permissions:** Contents → **Read and write**
   - **Expiration:** your call — 90 days or 1 year are both reasonable. Put a reminder in your calendar.
   - Click **Generate token** and copy the value (shown once).

2. **Add the token as a secret in the cargoterm repo**
   - Go to https://github.com/GoCelesteAI/cargoterm/settings/secrets/actions
   - Click **New repository secret**
   - **Name:** `HOMEBREW_TAP_TOKEN`
   - **Secret:** paste the PAT value
   - Click **Add secret**

3. **Verify on the next release.** The next time you tag and push, the `bump-tap` job will run and commit to `GoCelesteAI/homebrew-tap`. You can dry-run without cutting a new version by using **Run workflow** on an existing tag via the Actions UI — though note that a re-run will attempt to re-push the same formula commit (a no-op).

## Without the token

The release still publishes normally. `bump-tap` just prints a skip notice and exits. You then bump the tap manually:

```sh
git clone https://github.com/GoCelesteAI/homebrew-tap.git
# edit Formula/cargoterm.rb — replace version, URLs, and the three sha256 values
git commit -am "cargoterm X.Y.Z"
git push
```

Fetch the SHAs from the release sidecar files:

```sh
for t in aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-gnu; do
  curl -sL "https://github.com/GoCelesteAI/cargoterm/releases/download/vX.Y.Z/cargoterm-X.Y.Z-${t}.tar.gz.sha256"
done
```

## Version bump workflow

1. Edit `version` in `Cargo.toml`.
2. `cargo build --release` to refresh `Cargo.lock`.
3. Commit, push main.
4. Tag and push:
   ```sh
   git tag -a vX.Y.Z -m "cargoterm X.Y.Z — short theme"
   git push origin vX.Y.Z
   ```
5. Watch the Actions tab. Release + tap-bump both run automatically.
6. Verify with `brew update && brew upgrade cargoterm`.
