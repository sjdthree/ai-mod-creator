# 0001 — Build Manager (mce-build)

- **Phase:** 3
- **Workstream:** mc-integration
- **Effort:** Large
- **Owner:** unassigned
- **Status:** in progress

## Objective

Execute the pinned Gradle build of a generated project and classify build results, with the
orchestration + log-parsing logic fully unit-testable without a live Minecraft toolchain.

## Background

`SPEC-003 §10` (Build Manager: inputs generated project + pinned JDK + Gradle wrapper; outputs
build result, logs, JAR, checksum, classified failure). ADR-0003 pin.

## Dependencies

`phase2/*` (a generated project to build). Gradle wrapper scaffolding (add to generator).

## Implementation Notes

- Split into pure logic (command construction, log classification) and effectful invocation.
- `build_command(project_root)` → the `./gradlew build` invocation + args, deterministic.
- `classify(log)` → `BuildOutcome` mapping known Gradle/javac signatures to a failure category
  (compile error, missing dependency, toolchain/JDK mismatch, gradle failure, unknown) with the
  `SPEC-005 §17` owner (`BUILD_ENVIRONMENT` vs `GENERATOR`).
- `run(project_root)` shells out to the wrapper; **requires JDK 21 + network** — gated behind an
  env check that returns a clear `Unimplemented`/`environment` error when unmet, never a false
  success.
- Prohibited (`SPEC-003 §10`): must not edit generated files, reinterpret intent, or report
  gameplay success.

## Acceptance Tests

- `classify` maps fixture logs (success, javac error, JDK mismatch, missing NeoForge dep) to the
  right category + owner. (Unit-testable now.)
- `run` on a real generated project with JDK 21 produces a JAR. (Env-gated; spike/CI.)

## Benchmark Coverage

`COMP-BUILD-*` (log classification); integration `INT-*` (generate→build).

## Traceability

- Product requirement(s): `FR-005`, `NFR-003`.
- Component(s): `mce-build`, consumes `mce-generator` output.
