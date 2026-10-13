# Security Policy

## Reporting a vulnerability

Please **do not open a public issue** for security problems.

Report them privately through GitHub: go to the **Security** tab of this repository and choose
**Report a vulnerability**. That opens a private advisory only the maintainers can see.

Please include what you found, how to reproduce it, and what an attacker could do with it. We aim to
reply within a week and will keep you updated until it's fixed. With your permission, we'll credit
you in the advisory and the release notes.

## Supported versions

AI-mod creator is in alpha. Only the **latest release** gets security fixes; please check that the
issue still happens there before reporting.

## Scope

In scope: the desktop app, the `amc` engine and CLI, the build and release scripts in this
repository, and the mod source code the engine generates.

Out of scope: vulnerabilities in Minecraft, Fabric, NeoForge, Gradle, llama.cpp or the base Gemma
model themselves — please report those to their own projects.
