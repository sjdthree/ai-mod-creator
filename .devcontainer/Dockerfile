# AMC dev container — full toolchain for building & verifying generated NeoForge mods, isolated
# from the host. JDK 21 (ADR-0003), Rust, Gradle, and the headless-client libs live ONLY in this
# image and its named volumes. Nothing here touches the host's Java/Gradle/Rust or other repos.
FROM eclipse-temurin:21-jdk-jammy

ARG GRADLE_VERSION=8.10.2
ARG USER=dev
ARG UID=1000
ARG GID=1000

# System packages: git + build tools for Rust, unzip for Gradle, and Xvfb + GL libs for the
# future headless Minecraft client smoke test (SPEC-005 §15 L6 / ADR-0004).
RUN apt-get update && apt-get install -y --no-install-recommends \
        git curl ca-certificates build-essential pkg-config unzip \
        xvfb libxi6 libxrender1 libxtst6 libxxf86vm1 libgl1 libglu1-mesa \
    && rm -rf /var/lib/apt/lists/*

# Gradle — used once to materialize a project's wrapper jar via `gradle wrapper`; builds then run
# through the project's own `./gradlew`.
RUN curl -fsSL "https://services.gradle.org/distributions/gradle-${GRADLE_VERSION}-bin.zip" \
        -o /tmp/gradle.zip \
    && unzip -q /tmp/gradle.zip -d /opt \
    && ln -s "/opt/gradle-${GRADLE_VERSION}/bin/gradle" /usr/local/bin/gradle \
    && rm /tmp/gradle.zip

# Non-root user (bind-mount ownership is virtualized on macOS Docker Desktop, so UID need not match
# the host; override with --build-arg UID=... on Linux if you want an exact match).
RUN groupadd -g "${GID}" "${USER}" && useradd -m -u "${UID}" -g "${GID}" -s /bin/bash "${USER}"

# Pre-create the cache/output dirs owned by the dev user. When the named volumes are first mounted
# here, Docker seeds them from these dirs, so the volumes inherit dev ownership (avoids root-owned
# volumes the non-root user can't write to).
RUN mkdir -p /home/${USER}/.cargo/registry /home/${USER}/.gradle /home/${USER}/target \
    && chown -R "${UID}:${GID}" /home/${USER}

USER ${USER}

# Rust toolchain for the amc-* workspace (stable + fmt + clippy), installed into the user's home.
ENV PATH=/home/${USER}/.cargo/bin:${PATH}
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain stable --profile minimal -c rustfmt -c clippy

# Keep build artifacts and caches OFF the host tree — these paths are backed by named Docker
# volumes at run time (see scripts/dev-container.sh and .devcontainer/devcontainer.json).
ENV CARGO_TARGET_DIR=/home/${USER}/target \
    GRADLE_USER_HOME=/home/${USER}/.gradle

WORKDIR /workspace
CMD ["/bin/bash"]
