#!/usr/bin/env python3
"""Sets the mod's version everywhere it is written, from VERSION or an argument.

    python3 tools/set_version.py            # write VERSION's version into every file
    python3 tools/set_version.py 0.2.0      # set VERSION to 0.2.0, then the same

The version is semver (MAJOR.MINOR.PATCH, optional -prerelease). It goes into the native mod's
`[mod] version` (what Horizon checks dependencies against), the content mod's manifest and the
generator that writes it, and the two Cargo packages. CHANGELOG.md is written by hand.
"""
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent
SEMVER = re.compile(r"^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$")

# (file, pattern whose group 1 is kept before the new version, how many matches it must have)
TARGETS = [
    ("native/horizon-mixin.toml", r'(\[mod\][^\[]*?\nversion = ")[^"]*', 1),
    ("content/horizon-mod.toml", r'(\nversion = ")[^"]*', 1),
    ("tools/build_content.py", r'(\nversion = ")[^"]*', 1),
    ("native/engine/Cargo.toml", r'(\[package\][^\[]*?\nversion = ")[^"]*', 1),
    ("tests/logic/Cargo.toml", r'(\[package\][^\[]*?\nversion = ")[^"]*', 1),
    ("tests/logic/Cargo.lock", r'(name = "altar-logic-tests"\nversion = ")[^"]*', 1),
]


def main() -> int:
    version_file = HERE / "VERSION"
    if len(sys.argv) > 1:
        version = sys.argv[1]
    else:
        version = version_file.read_text().strip()
    if not SEMVER.match(version):
        print(f"not a semver version: {version!r}", file=sys.stderr)
        return 1
    version_file.write_text(version + "\n")
    for rel, pattern, expected in TARGETS:
        path = HERE / rel
        text = path.read_text()
        new, count = re.subn(pattern, lambda m: m.group(1) + version, text, flags=re.S)
        if count != expected:
            print(f"{rel}: expected {expected} version field(s), found {count}", file=sys.stderr)
            return 1
        path.write_text(new)
    print(f"version {version}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
