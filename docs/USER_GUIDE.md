# User guide

Make Minecraft mods by describing them in plain English. Every creation is **built and tested
inside Minecraft** before it's saved — if AMC says it works, it works.

New here? Do the [setup](GETTING_STARTED.md) first.

---

## The loop

1. **Create** — describe something. AMC interprets → generates → **builds → tests it in Minecraft** →
   saves it only if it passes.
2. **Play** — launch Minecraft and use it (you start in Creative so your items are right there).
3. **Refine** — after playing, fine-tune it: *"make it stronger," "faster," "rename it Sting."*
4. **Export** — drop the mod into your real Minecraft so you (and friends) can play it for keeps.

Nothing you make can break what already works: a failed request leaves your last good version
untouched, and you can **undo** any change.

---

## What you can make

| Kind | Example request | What you get |
|---|---|---|
| **Weapon** | "a ruby sword called Ruby Blade, 1200 durability, 9 damage" | a real sword (attack damage, durability, speed) |
| **Tool** | "a ruby pickaxe with 1500 durability" | pickaxe / axe / shovel / hoe that mines |
| **Food** | "a golden apple that restores 8 hunger" | an edible item (nutrition + saturation) |
| **Block** | "a glowing ruby ore block, hardness 5" | a placeable block that drops itself when broken |
| **Item** | "a ruby gem" | a basic item / ingredient |
| **Recipe** | "a recipe: two rubies and a stick make a Ruby Blade" | a crafting/smelting recipe |

You can name things ("called Sting"), set numbers ("9 damage", "1500 durability"), and combine
requests. Ambiguous or unsupported asks come back as a clear question instead of a wrong guess.

---

## Desktop app

- **Describe your creation** in the box → **Create**. The pipeline animates
  (understand → validate → update → write → compile → **test in Minecraft**), then your item appears in
  the sidebar with its icon.
- **Refine:** click any creation to enter *Refining* mode, then type a tweak ("make it faster",
  "bump damage to 12", "rename it Sting") and hit **Update** — it modifies that item in place.
- **↶ undo** on a card rolls back its last change.
- **▶ Play in Minecraft** opens the client (auto-Creative). **Export** copies the mod into your
  Minecraft `mods` folder.

## Command line

```bash
amc init myworld                              # start a project
amc request "<english>" --project myworld     # create or refine (the engine decides which)
amc list   --project myworld                  # your creations
amc play   --project myworld                  # launch Minecraft (add --interactive for a visible window)
amc test   --project myworld                  # rebuild + re-run the in-game tests
amc undo   --project myworld                  # undo the last change
amc revert tx-000001 --project myworld        # go back to a specific earlier version
amc export --project myworld [--to <dir>]     # copy the .jar into the Minecraft mods folder
```

Add `--json` to any command for machine-readable output. Refinement is just a `request` on an
existing project — the model resolves "it" / "the Ruby Blade" from what's already there:

```bash
amc request "a ruby sword called Ruby Blade, 1200 durability, 9 damage" --project myworld
amc request "make the Ruby Blade much stronger, 13 damage"             --project myworld   # updates it
amc request "rename it to Sting"                                        --project myworld   # renames it
amc undo --project myworld                                                                 # undo the rename
```

---

## Playing your creation for real

`amc play` runs a **development** client for quick testing. To keep the mod in your normal game:

1. `amc export --project myworld` — copies the `.jar` into your Minecraft `mods` folder
   (macOS `~/Library/Application Support/minecraft/mods`, or `--to <folder>` for a modpack/instance).
2. Launch Minecraft **1.21.1 with NeoForge** from your usual launcher (or with **Fabric Loader + Fabric
   API** if you built with `--loader fabric`). Your items are in the game.

---

## Good to know

- **Creative mode** is where creative-menu items live. `amc play` starts you in Creative; in your own
  world, `/gamemode creative`, then **E** → the matching tab (sword → **Combat**) or search the name.
- **"Verified" means it works.** AMC runs real in-game tests (the block places, the weapon is a
  weapon, the food is edible) — not just "it compiled."
- **Placeholder art.** Textures are simple generated silhouettes (a sword looks like a sword); swap in
  your own art later by replacing the PNGs under the generated project's `textures/`.

## Troubleshooting

- **"the request is ambiguous"** — add specifics (a name, numbers, the type): "a *sword* called X with
  *9 damage*".
- **A refine hit the wrong item** — name it explicitly: "make **the Ruby Pickaxe** faster".
- **Nothing in the creative menu** — you're in Survival. Switch to Creative (above).
- **Model / build issues** — see [Getting Started → Troubleshooting](GETTING_STARTED.md#troubleshooting).
