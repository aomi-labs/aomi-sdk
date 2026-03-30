#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VERSION_RE = re.compile(r'(?m)^version = "([^"]+)"$')


def run(*args: str) -> str:
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def parse_version(cargo_toml_text: str, label: str) -> str:
    match = VERSION_RE.search(cargo_toml_text)
    if not match:
        raise SystemExit(f"failed to read version from {label}")
    return match.group(1)


def current_sdk_version() -> str:
    data = tomllib.loads((ROOT / "sdk" / "Cargo.toml").read_text())
    return data["package"]["version"]


def base_sdk_version(base_rev: str) -> str | None:
    show = subprocess.run(
        ["git", "show", f"{base_rev}:sdk/Cargo.toml"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if show.returncode != 0:
        return None
    return parse_version(show.stdout, f"{base_rev}:sdk/Cargo.toml")


def changed_sdk_files(base_rev: str, head_rev: str) -> list[str]:
    output = run("git", "diff", "--name-only", base_rev, head_rev, "--", "sdk")
    return [line for line in output.splitlines() if line]


def write_output(path: str | None, key: str, value: str) -> None:
    if not path:
        return
    with open(path, "a", encoding="utf-8") as fh:
        fh.write(f"{key}={value}\n")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", required=True)
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--github-output")
    args = parser.parse_args()

    if not args.base or re.fullmatch(r"0+", args.base):
        raise SystemExit("base revision is required")

    current_version = current_sdk_version()
    changed_files = changed_sdk_files(args.base, args.head)
    sdk_changed = bool(changed_files)
    base_version = base_sdk_version(args.base)
    version_bumped = base_version is None or current_version != base_version
    publish_sdk = sdk_changed and version_bumped

    write_output(args.github_output, "sdk_version", current_version)
    write_output(args.github_output, "sdk_changed", str(sdk_changed).lower())
    write_output(args.github_output, "publish_sdk", str(publish_sdk).lower())

    if not sdk_changed:
        print(f"sdk unchanged; current version {current_version}")
        return

    print("sdk files changed:")
    for path in changed_files:
        print(f"  - {path}")

    if not version_bumped:
        print(
            f"sdk version was not bumped: base={base_version} current={current_version}",
            file=sys.stderr,
        )
        raise SystemExit(1)

    print(f"sdk version bump OK: {base_version or 'missing'} -> {current_version}")


if __name__ == "__main__":
    main()
