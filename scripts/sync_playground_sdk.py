#!/usr/bin/env python3
"""Align a template repo's aomi-sdk pin with a released SDK version.

Pass the template checkout as --root. Rewrites the exact `aomi-sdk = "=X"` pin in
every app `Cargo.toml` named by `.aomi/config.json`, regenerates each
`Cargo.lock` with cargo, and updates README mentions of the pin. Prints the
list of changed files, or nothing when the template already matches.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

PIN_RE = re.compile(r'(?m)^(aomi-sdk\s*=\s*)"=[^"]+"')
TABLE_PIN_RE = re.compile(r'(?m)^(aomi-sdk\s*=\s*\{[^}]*?version\s*=\s*)"=[^"]+"')
README_PIN_RE = re.compile(r'aomi-sdk = "=[0-9][^"]*"')


def app_dirs(root: Path) -> list[Path]:
    config = json.loads((root / ".aomi" / "config.json").read_text())
    return sorted({(root / path).parent for path in config["applications"]})


def current_pin(manifest: Path) -> str | None:
    dep = tomllib.loads(manifest.read_text()).get("dependencies", {}).get("aomi-sdk")
    if isinstance(dep, str):
        return dep
    if isinstance(dep, dict):
        return dep.get("version")
    return None


def rewrite_pin(manifest: Path, required: str) -> bool:
    text = manifest.read_text()
    replaced, count = PIN_RE.subn(rf'\g<1>"={required}"', text)
    if count == 0:
        replaced, count = TABLE_PIN_RE.subn(rf'\g<1>"={required}"', text)
    if count != 1:
        raise SystemExit(f"{manifest}: expected one exact aomi-sdk pin, found {count}")
    if replaced == text:
        return False
    manifest.write_text(replaced)
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--required-version", required=True)
    parser.add_argument("--root", default=".", type=Path)
    args = parser.parse_args()
    root: Path = args.root
    required: str = args.required_version
    changed: list[str] = []

    for app_dir in app_dirs(root):
        manifest = app_dir / "Cargo.toml"
        if current_pin(manifest) == f"={required}":
            continue
        rewrite_pin(manifest, required)
        changed.append(str(manifest.relative_to(root)))
        lock = app_dir / "Cargo.lock"
        if lock.exists():
            subprocess.run(
                ["cargo", "update", "--package", "aomi-sdk", "--precise", required],
                cwd=app_dir,
                check=True,
            )
            changed.append(str(lock.relative_to(root)))

    if changed:
        for readme in root.glob("**/README.md"):
            if ".git" in readme.parts or "target" in readme.parts:
                continue
            text = readme.read_text()
            replaced = README_PIN_RE.sub(f'aomi-sdk = "={required}"', text)
            if replaced != text:
                readme.write_text(replaced)
                changed.append(str(readme.relative_to(root)))

    print("\n".join(changed))
    return 0


if __name__ == "__main__":
    sys.exit(main())
