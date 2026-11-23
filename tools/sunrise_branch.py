#!/usr/bin/env python3
"""Commits mod-sunrise/, as tools/make_sunrise.py last wrote it, to the `sunrise` branch: the source
the Sunrise registry's Nightly source list builds (sunrise-org/sunrise-mods, `sources/occultism.toml`).

Sunrise lists a native mod for a Nightly by its source, not by a file: a player's launcher fetches
the branch and builds it. The branch holds only the generated crate, at its root, and moves at each
release. It is checked out in .work/sunrise-branch (made here if missing).

    python3 tools/make_sunrise.py && python3 tools/sunrise_branch.py
    git -C .work/sunrise-branch push origin sunrise      # an outward step: at a release
"""
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CRATE = ROOT / "mod-sunrise"
BRANCH = "sunrise"
TREE = ROOT / ".work" / "sunrise-branch"
# Build output and the fetched SDK stay out of the branch.
SKIP = {"target", ".sunrise", ".git"}

README = """# Occultism for Sunrise (source)

The Sunrise build of [Occultism for Horizon](https://github.com/Dawnforger/occultism-horizon), an unofficial port of
[Occultism](https://github.com/klikli-dev/occultism) by KliKli ([klikli-dev](https://github.com/klikli-dev)), made
with their permission. Please report problems [to the port](https://github.com/Dawnforger/occultism-horizon/issues),
not to KliKli.

This branch is generated: `tools/make_sunrise.py` writes it from the main line's source at each release, and it
is what the Sunrise registry's Nightly source list builds. Don't edit it here; changes go to the main line.

Build it with `cargo sunrise-mod build`, or let the Sunrise launcher build it for you. The licences and notices for
Occultism's code and art are in `LICENSE-occultism.txt`, `NOTICE.txt` and `NOTICE-occultism-third-party.txt`.
"""


def git(*args, cwd=ROOT):
    return subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


def main():
    if not (CRATE / "sunrise-mod.toml").exists():
        sys.exit("run tools/make_sunrise.py first: mod-sunrise/sunrise-mod.toml is missing")
    if not TREE.exists():
        git("worktree", "add", str(TREE), BRANCH)
    for old in TREE.iterdir():
        if old.name not in SKIP:
            shutil.rmtree(old) if old.is_dir() else old.unlink()
    for item in CRATE.iterdir():
        if item.name in SKIP:
            continue
        if item.is_dir():
            shutil.copytree(item, TREE / item.name)
        else:
            shutil.copy2(item, TREE / item.name)
    (TREE / ".gitignore").write_text("target/\n.sunrise/\n")
    (TREE / "README.md").write_text(README)
    git("add", "-A", cwd=TREE)
    if not git("status", "--porcelain", cwd=TREE):
        print("sunrise branch: already up to date")
        return
    version = (ROOT / "VERSION").read_text().strip()
    head = git("rev-parse", "--short", "HEAD")
    git("commit", "-q", "-m", f"Occultism {version} for Sunrise, generated from main {head}", cwd=TREE)
    print(f"sunrise branch: committed {version} from {head}; push it with: git -C {TREE.relative_to(ROOT)} push origin {BRANCH}")


if __name__ == "__main__":
    main()
