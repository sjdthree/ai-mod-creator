# Dev Container

A Docker container with the full AMC toolchain — **JDK 21** (ADR-0003), Rust, Gradle, and the
Xvfb/GL libraries needed for headless Minecraft client checks. Its purpose is **isolation**: the
JDK 21 and every build cache live only inside the image and named Docker volumes, so nothing leaks
to the host or to other repositories.

> The host already has Temurin 21 installed, but its **default** `java` is 17. This container
> sidesteps that entirely — inside it, `java -version` is always 21, and the host default stays 17
> for your other projects.

## What's isolated where

| Thing | Lives in | Touches host? |
|---|---|---|
| JDK 21, Gradle, Xvfb, Rust toolchain | the image (`amc-dev`) | no |
| Cargo registry cache | volume `amc-cargo` | no |
| Gradle / NeoForge cache | volume `amc-gradle` | no |
| Rust build outputs (`target/`) | volume `amc-target` | no (kept off the bind mount) |
| Your source code | bind-mounted `/workspace` | yes — it's your repo, edited normally |

## Usage (CLI)

```bash
./scripts/dev-container.sh build     # one-time: build the image (~1.5 GB download into Docker)
./scripts/dev-container.sh shell     # interactive shell inside the container
./scripts/dev-container.sh gate      # run the Rust CI gate (fmt/clippy/test/empty-bench) inside
./scripts/dev-container.sh clean     # remove the image + cache volumes (host stays untouched)
```

## Usage (VS Code / Cursor)

Open the repo and "Reopen in Container" — `.devcontainer/devcontainer.json` builds the same image
and mounts the same volumes, with rust-analyzer and the Java pack preinstalled.

## Running the Phase 3 build/verify spike inside the container

This is the environment the Phase 3 spike (`implementation/phase3/0000`, ADR-0004) needs. Inside
the container shell:

```bash
# 1. Build the engine and generate the Ruby Blade project from canonical state.
cargo build --workspace
#    (once `amc` can emit a project to disk; today: exercise the generator via its tests/example)

# 2. In the generated project directory, materialize the Gradle wrapper and build the mod.
cd <generated-project>
gradle wrapper --gradle-version 8.10.2
./gradlew --no-daemon build          # downloads NeoForge 21.1.x on first run

# 3. Later: dedicated-server boot + GameTest, and headless client under Xvfb, e.g.
xvfb-run -a ./gradlew --no-daemon runGameTestServer
```

Record the outcome in a follow-up ADR that supersedes ADR-0004.

## Notes

- **First build/run downloads a lot** (base image + apt + Gradle + NeoForge + crates). It all goes
  into Docker's storage and the named volumes, never the host filesystem.
- On Apple Silicon the container is `arm64`; JDK 21, Gradle, and NeoForge all run natively there.
- Bind-mount file ownership is virtualized by Docker Desktop on macOS, so the container's `dev`
  user (UID 1000) writes files that appear owned by you on the host. On Linux, rebuild with
  `--build-arg UID=$(id -u) --build-arg GID=$(id -g)` for an exact match.
