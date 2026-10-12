#!/usr/bin/env python3
"""End-to-end randomized robustness harness for the MCE `request` pipeline.

Proves the invariants the recurring create-vs-update bug violated — *an edit UPDATES the named creation
and never duplicates it*, plus *rename* and *delete* target exactly the right creation — over inputs
generated at runtime (random names, types, values), NOT the reported example values. Baking a specific
"Sting"/"1600 durability" case into a test only confirms that one case and hides brittleness, so every
value here is drawn fresh each run.

This exercises the FULL pipeline (intent -> validate -> state -> generate -> build), so it needs JDK 21 +
the Gradle wrapper and is slow — run it manually or nightly, not on every commit. The fast, CI-tier
version of these same invariants (intent layer only, no Gradle) lives as a Rust test in
crates/mce-intent/src/randomized_tests.rs and runs under `cargo test`.

    Usage:  python tools/randtest.py [N]        # N random items, default 3
    Env:    JAVA_HOME                            # defaults to the Android Studio JBR on Windows
            MCE_BIN                              # override the mce binary path
"""
import subprocess, json, random, string, os, sys, tempfile, shutil
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]


def mce_bin():
    if os.environ.get("MCE_BIN"):
        return os.environ["MCE_BIN"]
    exe = "mce.exe" if os.name == "nt" else "mce"
    for prof in ("release", "debug"):
        p = REPO / "target" / prof / exe
        if p.exists():
            return str(p)
    sys.exit("mce binary not found — run `cargo build --release -p mce-cli` first (or set MCE_BIN)")


MCE = mce_bin()
ENV = dict(os.environ)
if not ENV.get("JAVA_HOME") and os.name == "nt":
    ENV["JAVA_HOME"] = r"C:\Program Files\Android\Android Studio\jbr"

WEAPONS = ["sword", "dagger", "mace", "blade", "cutlass", "rapier", "katana", "spear"]
TOOLS = ["pickaxe", "axe", "shovel", "hoe", "drill", "hatchet"]
FOODS = ["apple", "bread", "cookie", "steak", "pie", "muffin", "carrot"]
MATS = ["ruby", "copper", "diamond", "emerald", "obsidian", "bronze", "gold", "iron", "crystal", "cobalt", "mithril"]
# Vanilla ingredients the deterministic recipe parser resolves to minecraft ids.
INGREDIENTS = ["stick", "iron", "gold", "diamond", "emerald", "redstone", "coal", "leather", "string"]


def mce(args, cwd):
    r = subprocess.run([MCE, "--json"] + args, capture_output=True, text=True, cwd=cwd, env=ENV)
    try:
        return json.loads(r.stdout)
    except Exception:
        return {"status": "PARSE_ERR", "raw": (r.stdout or r.stderr)[:300]}


def rand_name(exclude=()):
    while True:
        n = random.choice(string.ascii_uppercase) + "".join(
            random.choice(string.ascii_lowercase) for _ in range(random.randint(3, 8))
        )
        if n not in exclude:
            return n


def items(cwd):
    d = mce(["list"], cwd)
    return d if isinstance(d, list) else (d.get("items") or d.get("concepts") or [])


def read_state(cwd):
    """The persisted canonical state (concepts + recipes) — recipes aren't exposed by `mce list`."""
    try:
        return json.loads((Path(cwd) / ".mce" / "state.json").read_text())
    except Exception:
        return {}


def active_recipes_for(cwd, result_name):
    """Count active recipes whose result concept has display name `result_name`."""
    st = read_state(cwd)
    canon = st.get("canonical", st)
    concepts = canon.get("concepts", {})
    n = 0
    for r in canon.get("recipes", {}).values():
        if r.get("status") != "active":
            continue
        ref = r.get("result", {})
        val = ref.get("value")
        got = concepts.get(val, {}).get("display_name") if ref.get("reference_type") == "concept_id" else val
        if got == result_name:
            n += 1
    return n


def stored(cwd, name, key):
    hits = [c for c in items(cwd) if c.get("name") == name]
    if len(hits) != 1:
        return ("COUNT=%d" % len(hits), None)
    return ("ok", hits[0].get("properties", {}).get(key))


def ok_status(res):
    return res.get("status") in ("built", "verified")


def main():
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 3
    root = Path(tempfile.mkdtemp(prefix="mce-randtest-"))
    proj = root / "P"
    mce(["init", "P"], root)
    fails = []
    print(f"=== RANDOMIZED e2e: {n} items × create/update/recipe/rename/delete — values generated at runtime ===")
    print(f"    binary: {MCE}")

    for i in range(n):
        cat = random.choice(["weapon", "tool", "food"])
        if cat == "weapon":
            sub, key, stat, lo, hi = random.choice(WEAPONS), "attack_damage", "attack damage", 1, 50
        elif cat == "tool":
            sub, key, stat, lo, hi = random.choice(TOOLS), "durability", "durability", 100, 2000
        else:
            sub, key, stat, lo, hi = random.choice(FOODS), "nutrition", None, 1, 20

        seen = set()
        def val():
            while True:
                v = random.randint(lo, hi)
                if v not in seen:
                    seen.add(v)
                    return v

        name = rand_name()
        newname = rand_name(exclude={name})
        mat = random.choice(MATS)
        v1, v2, v3 = val(), val(), val()

        if cat == "food":
            create = f"a {mat} {sub} called {name} that heals {v1}"
            editform = f"a {sub} called {name} that heals {v2}"
            update = f"update {name} to heal {v3}"
        else:
            create = f"a {mat} {sub} called {name} with {v1} {stat}"
            editform = f"a {sub} called {name} with {v2} {stat}"
            update = f"update {name} {stat} to {v3}"

        def check(label, want_name, want_val, extra=None):
            st, got = stored(proj, want_name, key)
            good = st == "ok" and got == want_val and (extra is None or extra())
            if not good:
                fails.append((label, f"{want_name}={key}", f"want {want_val} got {got} ({st})"))
            return good

        # CREATE — new name → a create with the right value.
        cr = mce(["request", create, "--project", str(proj), "--no-verify"], root)
        c_ok = ok_status(cr) and check("CREATE", name, v1)

        # EDIT-FORM UPDATE — a create-shaped definition for the now-existing name must UPDATE, not duplicate.
        mce(["request", editform, "--project", str(proj), "--no-verify"], root)
        u1_ok = check("UPDATE-editform", name, v2, extra=lambda: len([c for c in items(proj) if c["name"] == name]) == 1)

        # NATURAL UPDATE — prose naming the creation.
        mce(["request", update, "--project", str(proj), "--no-verify"], root)
        u2_ok = check("UPDATE-natural", name, v3)

        # ADD RECIPE — a shapeless recipe crafting the creation from vanilla ingredients (quantities and
        # ingredients drawn fresh each run). Assert an active recipe now targets exactly this creation.
        q = random.randint(1, 3)
        ing_a, ing_b = random.sample(INGREDIENTS, 2)
        recipe_req = f"craft a {name} from {q} {ing_a} and a {ing_b}"
        rc = mce(["request", recipe_req, "--project", str(proj), "--no-verify"], root)
        addr_ok = ok_status(rc) and active_recipes_for(proj, name) == 1
        if not addr_ok:
            fails.append(("ADD-RECIPE", name, f"status={rc.get('status')} recipes={active_recipes_for(proj, name)} req={recipe_req!r}"))

        # REMOVE RECIPE — the recipe for the creation is gone (no active recipe targets it).
        rmr = mce(["request", f"remove the recipe for {name}", "--project", str(proj), "--no-verify"], root)
        rmr_ok = ok_status(rmr) and active_recipes_for(proj, name) == 0
        if not rmr_ok:
            fails.append(("REMOVE-RECIPE", name, f"status={rmr.get('status')} recipes={active_recipes_for(proj, name)}"))

        # RENAME — the creation is now known by newname (and no longer by name).
        rn = mce(["request", f"rename {name} to {newname}", "--project", str(proj), "--no-verify"], root)
        names_now = {c["name"] for c in items(proj)}
        r_ok = ok_status(rn) and newname in names_now and name not in names_now
        if not r_ok:
            fails.append(("RENAME", f"{name}->{newname}", f"status={rn.get('status')} names={sorted(names_now)}"))

        # DELETE — the creation disappears from the active list.
        dl = mce(["request", f"delete {newname}", "--project", str(proj), "--no-verify"], root)
        gone = newname not in {c["name"] for c in items(proj)}
        d_ok = ok_status(dl) and gone
        if not d_ok:
            fails.append(("DELETE", newname, f"status={dl.get('status')} still_present={not gone}"))

        flag = lambda b: "OK" if b else "FAIL"
        print(
            f"[{i+1}/{n}] {name:10}->{newname:10} {cat:6} {key:13} "
            f"create {flag(c_ok)} · edit {flag(u1_ok)} · update {flag(u2_ok)} · "
            f"recipe+ {flag(addr_ok)} · recipe- {flag(rmr_ok)} · rename {flag(r_ok)} · delete {flag(d_ok)}"
        )

    print(f"\n=== {7*n} operations attempted; {7*n - len(fails)} passed ===")
    if fails:
        print("FAILURES:")
        for kind, what, why in fails:
            print(f"  [{kind}] {what}  ->  {why}")
    else:
        print("ALL RANDOM OPERATIONS PASSED")
    shutil.rmtree(root, ignore_errors=True)
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
