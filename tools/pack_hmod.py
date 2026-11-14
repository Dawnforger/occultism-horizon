#!/usr/bin/env python3
"""Packs the mod as one native .hmod, as Horizon's docs/api/mod-files.md lays it out: the content
(horizon-mod.toml, assets/, data/ from content/) and the native code (horizon-native.json and the
bundle under mixins/<id>/, from native/), naming the same mod id.

    python3 tools/pack_hmod.py [--mixins <commit>] [--out occultism-<version>.hmod]
    python3 tools/pack_hmod.py --split [--native-id occultism_native]

--split packs the two halves apart, for launchers that build a combined file without its content
(horizon-rust/horizon#15933): occultism-<version>.hmod with the content only, and
<native-id>-<version>.hnmod with the code only, and occultism_pack-<version>.hpack, the modpack that
installs both by their SHA-256 (the registry takes it once both versions are in its index). Two registry entries can't share an id and the
content must stay under its own id's namespace, so the code half takes another id.

The id, version and Horizon commit come from native/horizon-mixin.toml; the horizon-mixins commit
is the one the launcher builds with (native_mods::MIXINS_REV). The zip's entries are sorted and
dated alike, so the same tree packs to the same bytes.
"""
import argparse
import hashlib
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


def write_zip(out, entries):
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        for name, data in sorted(entries):
            info = zipfile.ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            z.writestr(info, data)
    print(f"packed {out} ({len(entries)} files)")


def modpack(version, mods):
    """horizon-modpack.toml (docs/api/mod-index.md, `.hpack`) naming each file by id, version and SHA-256."""
    lines = [
        "format = 1",
        'id = "occultism_pack"',
        'name = "Occultism for Horizon"',
        f'version = "{version}"',
        'description = "The Occultism port in its two halves: the content (occultism) and the native code (occultism_native)."',
        'authors = ["Dawnforger", "klikli-dev (Occultism)"]',
        'game = "26.3"',
    ]
    for mod_id, path in mods:
        lines += ["", "[[mods]]", f'id = "{mod_id}"', f'version = "{version}"', f'sha256 = "{hashlib.sha256(path.read_bytes()).hexdigest()}"']
    return ("\n".join(lines) + "\n").encode()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mixins", default=MIXINS)
    ap.add_argument("--out", type=Path)
    ap.add_argument("--split", action="store_true")
    ap.add_argument("--native-id", default="occultism_native")
    args = ap.parse_args()

    native, content = HERE / "native", HERE / "content"
    mixin = (native / "horizon-mixin.toml").read_text()
    mod_id, version = field(mixin, "mod", "id"), field(mixin, "mod", "version")
    horizon = field(mixin, "compatibility", "horizon_revision")
    content_version = re.search(r'^version\s*=\s*"([^"]*)"', (content / "horizon-mod.toml").read_text(), re.M).group(1)
    if content_version != version:
        sys.exit(f"content/horizon-mod.toml says {content_version}, native/horizon-mixin.toml {version}")

    code_id = args.native_id if args.split else mod_id
    manifest = {"schema": 1, "id": code_id, "version": version, "game": "26.3", "horizon": horizon, "mixins": args.mixins}
    code = [("horizon-native.json", (json.dumps(manifest, indent=2) + "\n").encode())]
    for rel in bundle_files(native):
        data = (native / rel).read_bytes()
        if rel.as_posix() == "horizon-mixin.toml" and code_id != mod_id:
            data = re.sub(rb'(\[mod\][^\[]*?\nid = ")[^"]*', lambda m: m.group(1) + code_id.encode(), data, count=1)
        code.append((f"mixins/{code_id}/{rel.as_posix()}", data))
    # The licences and credits travel with the file, beside the bundle's manifest.
    notices = {"LICENSE": HERE / "LICENSE", "NOTICE": HERE / "NOTICE", "NOTICE-third-party.md": content / "THIRD_PARTY_NOTICES-occultism.md"}
    for name, path in notices.items():
        code.append((f"mixins/{code_id}/{name}", path.read_bytes()))

    content_entries = [("horizon-mod.toml", (content / "horizon-mod.toml").read_bytes())]
    for top in ("assets", "data"):
        for path in sorted((content / top).rglob("*")):
            if path.is_file():
                content_entries.append((path.relative_to(content).as_posix(), path.read_bytes()))

    if args.split:
        halves = [(mod_id, HERE / f"{mod_id}-{version}.hmod"), (code_id, HERE / f"{code_id}-{version}.hnmod")]
        write_zip(halves[0][1], content_entries)
        write_zip(halves[1][1], code)
        write_zip(HERE / f"occultism_pack-{version}.hpack", [("horizon-modpack.toml", modpack(version, halves))])
    else:
        write_zip(args.out or HERE / f"{mod_id}-{version}.hmod", code + content_entries)

if __name__ == "__main__":
    main()
