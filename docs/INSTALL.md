# Install the Mod Creation Engine

Turn plain English into working Minecraft mods. Download, install, and create — the app ships with
everything it needs (the AI model runs **on your machine**, no accounts, no Python).

---

## 1. One prerequisite: Java 21

The app builds real Minecraft mods, which needs a Java 21 development kit.

- **Get it once (free):** [Adoptium Temurin **21** (LTS)](https://adoptium.net/temurin/releases/?version=21)
  → download the installer for your OS and run it.
- That's the only thing you need to install yourself. The app finds it automatically.

---

## 2. macOS (Apple Silicon)

1. **Download** `Mod Creation Engine_0.1.0_aarch64.dmg` from the
   [**Releases page**](https://github.com/sjdthree/ai-mod-creator/releases/latest).
2. **Open the `.dmg`** and **drag *Mod Creation Engine* into the *Applications* folder.**
3. **First launch** (the app isn't code-signed yet, so macOS asks the first time):
   - **Right-click** the app in Applications → **Open** → **Open** in the dialog.
   - *(Or, if macOS refuses, run once in Terminal: `xattr -cr "/Applications/Mod Creation Engine.app"` then open it.)*
4. The app opens; the footer shows **“starting the MCE Model… → MCE Model ready”** (a few seconds the
   first time while it loads the model).

Requires an Apple-Silicon Mac (M1 or newer) and macOS 12+.

---

## 3. Windows 11

1. **Download** `Mod Creation Engine_0.1.0_x64-setup.exe` (recommended) — or the `.msi` — from the
   [**Releases page**](https://github.com/sjdthree/ai-mod-creator/releases/latest).
   *(No Windows build published yet? It’s produced from source on a Windows box with
   [`apps/desktop/scripts/build_windows.ps1`](../apps/desktop/scripts/build_windows.ps1), then uploaded to
   the Releases page with
   [`apps/desktop/scripts/publish_windows_release.ps1`](../apps/desktop/scripts/publish_windows_release.ps1).)*
2. **Run the installer.**
   - The `-setup.exe` installs **per-user — no admin needed**; the `.msi` is for machine-wide/managed installs.
   - **It isn’t code-signed yet**, so Windows **SmartScreen** shows *“Windows protected your PC / unknown
     publisher.”* This is expected for an unsigned app — click **More info**, then **Run anyway**. (A signed
     build is planned; until then this warning is normal and not a sign of a bad download.)
3. **Launch “Mod Creation Engine”** from the Start menu. The footer shows **“starting the MCE Model… →
   MCE Model ready”** (a few seconds the first time while it loads the model).
4. The **first Quick Test** downloads the Minecraft/NeoForge files once (a few minutes); after that it’s
   about a minute.

Your creations are saved in `%USERPROFILE%\ModCreationEngine\`; uninstall via *Settings → Apps*. Needs
64-bit Windows 10/11 (WebView2 ships with Windows 11) and Java 21 from step 1.

---

## 4. Make your first mod

1. Type what you want, e.g. **`a ruby sword called Sting with 9 attack damage`** — or click one of the
   example chips — and press **Create**. First build takes ~30–60s (it downloads Minecraft/NeoForge
   once and caches it); after that it's ~10s.
2. When it says **verified ✓**, click **▶ Quick Test** — this launches a bundled **Minecraft 1.21.1 +
   NeoForge** sandbox so you can play your creation immediately, no setup, without touching your own worlds.
3. In-world you're in Creative mode: press **E**, open the **Combat / Tools / Building-Blocks** tab (or
   search your item's name) — there it is.
4. Keep going: create more items, refine them, and **Undo** anything. When you're happy, **Add to my
   Minecraft** copies the mod into your own game (it offers to set up Minecraft 1.21.1 + NeoForge for you
   if you don't have it yet).

**Tips**
- Give creations a **Capitalized name** and a number or two for best results
  (*"a golden apple that heals 8"*, *"a copper pickaxe"*).
- If a request isn't understood, the app tells you and offers close alternatives to pick from — nothing
  ever silently fails.

---

## Troubleshooting

| Symptom | Fix |
|---|---|
| “can’t be opened — unidentified developer” | Right-click → **Open** (step 3), or `xattr -cr` the app. |
| Footer stuck on “starting the MCE Model…” | Give it ~30s on first launch; if it says *unavailable*, reopen the app. |
| Create fails / “didn’t pass its test” | Tap **↻ Try again** (the small model occasionally has an off run). |
| “Add to my Minecraft” says it won’t load | Your Minecraft isn’t 1.21.1 + NeoForge — use **Quick Test**, or let the app **set it up for you**. |
| Build never finishes | Make sure **Java 21** (step 1) is installed. |
