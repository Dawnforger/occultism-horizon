#!/usr/bin/env python3
"""Writes mod-sunrise/, the port's package for Sunrise, from mod/, the package for Horizon.

Sunrise (github.com/sunrise-org/sunrise) is a fork of Horizon whose mod system is Horizon's under
its own names: a `.smod` with `sunrise-mod.toml`, the `sunrise-mod-api` crate from `.sunrise/sdk`,
the `sunrise_mod` and `sunrise_part` attributes, `data/<id>/sunrise/` for content declarations and
`[native] sunrise = "<build>"`. Everything else is mod/ as it is: one source, two packages. Where
the two APIs differ, the code goes through `platform`: src/platform/horizon.rs in mod/,
src/platform/sunrise.rs here (a native package may not choose Cargo features).

    python3 tools/make_sunrise.py          # then: cd mod-sunrise && cargo sunrise-mod build

mod-sunrise/ is generated: edit mod/ (or tools/build_content.py) and run this again. Its fetched
SDK (mod-sunrise/.sunrise/) is kept between runs.
"""
import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "mod"
OUT = ROOT / "mod-sunrise"
# The Sunrise build this is made for (Nightly 2026-11-19, whose signed SDK it compiles against).
# Move it forward deliberately, to a Nightly, as horizon-mod.toml's pin.
SUNRISE = "520fc4882de3abb8ecd8b98293f4d7263bc88181"
# Kept between runs: the fetched SDK and the build output.
KEEP = {".sunrise", "target", "Cargo.lock"}


def die(message):
    sys.exit(f"make_sunrise: {message}")


def replace_once(text, old, new, what):
    if old not in text:
        die(f"{what}: {old!r} not found; mod/ changed shape, update this script")
    return text.replace(old, new)


def manifest(text):
    """horizon-mod.toml as sunrise-mod.toml: the same mod, pinned to a Sunrise build."""
    text = re.sub(r'(?m)^horizon = "[0-9a-f]{40}"$', f'sunrise = "{SUNRISE}"', text)
    if f'sunrise = "{SUNRISE}"' not in text:
        die("horizon-mod.toml has no [native] horizon pin")
    text = re.sub(r"(?m)^# The Horizon build this is made for.*\n(# .*\n)*",
                  "# The Sunrise build this is made for (written by tools/make_sunrise.py from mod/horizon-mod.toml).\n", text)
    text = replace_once(text, "port to Horizon of Occultism", "port to Sunrise of Occultism", "description")
    text = replace_once(text, "Dawnforger (Horizon port)", "Dawnforger (Sunrise port)", "authors")
    return text


def cargo(text):
    """Cargo.toml against Sunrise's SDK."""
    text = replace_once(text, 'horizon-mod-api = { path = ".horizon/sdk/api/crates/horizon-mod-api" }',
                        'sunrise-mod-api = { path = ".sunrise/sdk/api/crates/sunrise-mod-api" }', "Cargo.toml")
    text = re.sub(r"(?m)^# The SDK of the build.*\n(# .*\n)*",
                  "# The SDK of the build sunrise-mod.toml's [native] sunrise pins: any cargo sunrise-mod command\n"
                  "# fetches it into .sunrise/sdk/.\n", text)
    return text


def strip_horizon_only(text, what):
    """Drops the lines from `// #horizon-begin` to `// #horizon-end`: hooks only Horizon has, whose
    Sunrise forms are in src/platform/sunrise.rs."""
    out, skipping = [], False
    for line in text.split("\n"):
        mark = line.strip()
        if mark.startswith("// #horizon-begin"):
            if skipping:
                die(f"{what}: #horizon-begin inside another")
            skipping = True
        elif mark.startswith("// #horizon-end"):
            if not skipping:
                die(f"{what}: #horizon-end without a begin")
            skipping = False
        elif not skipping:
            out.append(line)
    if skipping:
        die(f"{what}: #horizon-begin without an end")
    return "\n".join(out)


def source(text):
    """A Rust source as Sunrise names the attributes, with Sunrise's `platform`."""
    return text.replace("mod_api::horizon_mod]", "mod_api::sunrise_mod]").replace(
        "mod_api::horizon_part]", "mod_api::sunrise_part]").replace(
        '#[path = "platform/horizon.rs"]', '#[path = "platform/sunrise.rs"]')


def main():
    if not (SRC / "horizon-mod.toml").exists():
        die("run tools/build_content.py first: mod/horizon-mod.toml is missing")
    OUT.mkdir(exist_ok=True)
    for old in OUT.iterdir():
        if old.name not in KEEP:
            shutil.rmtree(old) if old.is_dir() else old.unlink()
    for item in SRC.iterdir():
        if item.name in {".horizon", "target", "Cargo.lock", "horizon-mod.toml", "Cargo.toml"}:
            continue
        if item.is_dir():
            shutil.copytree(item, OUT / item.name)
        else:
            shutil.copy2(item, OUT / item.name)
    (OUT / "sunrise-mod.toml").write_text(manifest((SRC / "horizon-mod.toml").read_text()))
    (OUT / "Cargo.toml").write_text(cargo((SRC / "Cargo.toml").read_text()))
    for rs in (OUT / "src").rglob("*.rs"):
        rs.write_text(strip_horizon_only(source(rs.read_text()), rs.relative_to(OUT)))
    (OUT / "src" / "platform" / "horizon.rs").unlink()
    # Content declarations: data/<id>/horizon/ is data/<id>/sunrise/ there.
    for declared in (OUT / "data").glob("*/horizon"):
        declared.rename(declared.with_name("sunrise"))
    print(f"mod-sunrise written for Sunrise {SUNRISE[:7]}")


if __name__ == "__main__":
    main()
