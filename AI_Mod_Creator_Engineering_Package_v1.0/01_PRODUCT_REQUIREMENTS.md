
# Product Requirements Specification

**Specification ID:** SPEC-001
**Status:** Draft 0.1

## Purpose

This document defines **what** the AI-mod creator (AMC) must do.
It intentionally avoids implementation details, which belong in later specifications.

---

# 1. Product Vision

AI-mod creator allows a Minecraft player to create and modify a working Minecraft mod using ordinary English.

The player describes gameplay.

AMC implements, verifies, and runs it.

---

# 2. Product Promise

> "Describe the Minecraft experience you want. AMC builds it."

The player never needs to understand:

- Java
- NeoForge
- Fabric
- Gradle
- registries
- source code
- JSON
- internal SDKs

---

# 3. Primary User

A Minecraft player with little or no programming experience who wants to create or evolve gameplay.

---

# 4. Primary Workflow

1. Create a project.
2. Describe a new gameplay concept in English.
3. Review AMC's interpretation.
4. Accept the change.
5. Build automatically.
6. Verify automatically.
7. Launch Minecraft.
8. Continue refining the mod using additional English requests.

---

# 5. Functional Requirements

## FR-001 English Interface

The system shall accept creation and modification requests written in ordinary English.

## FR-002 Canonical State

The system shall maintain a persistent canonical representation of the project independent of generated code.

## FR-003 Incremental Modification

The system shall modify existing concepts instead of recreating projects whenever possible.

## FR-004 Deterministic Generation

Supported concepts shall be generated deterministically from canonical state.

## FR-005 Automatic Verification

Every accepted change shall be validated, built, and verified before being reported as successful.

## FR-006 Rollback

Users shall be able to revert to any previously verified state.

## FR-007 Repeatability

Generating the same canonical state with the same generator version shall produce identical outputs.

---

# 6. Non-Functional Requirements

## NFR-001 Offline Operation

After installation, core workflows shall function without Internet connectivity.

## NFR-002 Transparency

Errors shall explain gameplay implications instead of implementation details whenever possible.

## NFR-003 Reliability

Failed requests shall not corrupt the last verified state.

## NFR-004 Local Performance

The common workflow shall execute on a consumer desktop using a small local language model.

---

# 7. MVP Capability Matrix

Included:

- Items
- Weapons
- Tools
- Food
- Basic blocks
- Recipes
- Simple loot
- Trigger-condition-action behaviors
- Appearance themes
- Build
- Verification
- Launch
- Modification
- Rollback

Deferred:

- Entities
- Dimensions
- Biomes
- Structures
- Networking
- GUIs
- Custom rendering
- Arbitrary scripting

---

# 8. Acceptance Criteria

The MVP is accepted when a user can:

1. Create a supported feature using English.
2. Launch Minecraft with the generated mod.
3. Modify that feature using English.
4. Relaunch Minecraft with the verified modification.
5. Revert to an earlier verified version.

The system must never falsely report success for an unverified implementation.

---

# 9. Product Success Metrics

Primary metric:

**Verified Request Completion Rate**

Supporting metrics:

- Intent accuracy
- Verification pass rate
- Build success rate
- Regression pass rate
- False-success rate
- Average end-to-end latency

---

# 10. Open Product Questions

These remain intentionally deferred until later specifications:

- Marketplace integration
- Multiplayer collaboration
- Asset generation quality
- Multiple loaders
- Multiple Minecraft versions
- Desktop GUI design
- Plugin ecosystem
