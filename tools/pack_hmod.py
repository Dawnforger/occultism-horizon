#!/usr/bin/env python3
"""Packs the mod as one native .hmod, as Horizon's docs/api/mod-files.md lays it out: the content
(horizon-mod.toml, assets/, data/ from content/) and the native code (horizon-native.json and the
bundle under mixins/<id>/, from native/), naming the same mod id.

    python3 tools/pack_hmod.py [--mixins <commit>] [--out occultism-<version>.hmod]

The id, version and Horizon commit come from native/horizon-mixin.toml; the horizon-mixins commit
is the one the launcher builds with (native_mods::MIXINS_REV). The zip's entries are sorted and
dated alike, so the same tree packs to the same bytes.
"""
import argparse
import json
import re
import sys
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent
# horizon-rust/horizon launcher 0.2.63's native_mods::MIXINS_REV.
MIXINS = "5a64af938ec86b00d53d16a408aabc0038fb3e57"
BUNDLE_TEXT = re.compile(r"^(README|LICENSE|LICENCE|COPYING|NOTICE).*|\.gitignore$")


def field(text, table, key):
    section = re.search(rf"^\[{table}\]\s*$(.*?)(?=^\[|\Z)", text, re.M | re.S)
    value = section and re.search(rf'^{key}\s*=\s*"([^"]*)"', section.group(1), re.M)
    if not value:
        sys.exit(f"native/horizon-mixin.toml: no [{table}] {key}")
    return value.group(1)


def bundle_files(native):
    """What mod-files.md lets under mixins/<id>/: .rs and .toml files, data/ folders, the notices."""
    for path in sorted(native.rglob("*")):
        if not path.is_file() or "target" in path.relative_to(native).parts:
            continue
        rel = path.relative_to(native)
        in_data = rel.parts[0] == "data" or (len(rel.parts) > 2 and rel.parts[1] == "data")
        notice = len(rel.parts) <= 2 and BUNDLE_TEXT.match(rel.name)
        if path.suffix in (".rs", ".toml") or in_data or notice:
            yield rel


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mixins", default=MIXINS)
    ap.add_argument("--out", type=Path)
    args = ap.parse_args()

    native, content = HERE / "native", HERE / "content"
    mixin = (native / "horizon-mixin.toml").read_text()
    mod_id, version = field(mixin, "mod", "id"), field(mixin, "mod", "version")
    horizon = field(mixin, "compatibility", "horizon_revision")
    content_version = re.search(r'^version\s*=\s*"([^"]*)"', (content / "horizon-mod.toml").read_text(), re.M).group(1)
    if content_version != version:
        sys.exit(f"content/horizon-mod.toml says {content_version}, native/horizon-mixin.toml {version}")

    manifest = {"schema": 1, "id": mod_id, "version": version, "game": "26.3", "horizon": horizon, "mixins": args.mixins}
    entries = [("horizon-native.json", (json.dumps(manifest, indent=2) + "\n").encode())]
    entries.append(("horizon-mod.toml", (content / "horizon-mod.toml").read_bytes()))
    for top in ("assets", "data"):
        for path in sorted((content / top).rglob("*")):
            if path.is_file():
                entries.append((path.relative_to(content).as_posix(), path.read_bytes()))
    for rel in bundle_files(native):
        entries.append((f"mixins/{mod_id}/{rel.as_posix()}", (native / rel).read_bytes()))
    # The licences and credits travel with the file, beside the bundle's manifest.
    notices = {"LICENSE": HERE / "LICENSE", "NOTICE": HERE / "NOTICE", "NOTICE-third-party.md": content / "THIRD_PARTY_NOTICES-occultism.md"}
    for name, path in notices.items():
        entries.append((f"mixins/{mod_id}/{name}", path.read_bytes()))

    out = args.out or HERE / f"{mod_id}-{version}.hmod"
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        for name, data in sorted(entries):
            info = zipfile.ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            z.writestr(info, data)
    print(f"packed {out} ({len(entries)} files)")


if __name__ == "__main__":
    main()
