#!/usr/bin/env python3
from __future__ import annotations

import argparse
import subprocess
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SDK_MANIFEST = "sdk/Cargo.toml"


def sdk_version(revision: str) -> str:
    result = subprocess.run(
        ["git", "show", f"{revision}:{SDK_MANIFEST}"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise SystemExit(
            f"failed to read {SDK_MANIFEST} from {revision}: {result.stderr.strip()}"
        )

    manifest = tomllib.loads(result.stdout)
    try:
        return manifest["package"]["version"]
    except KeyError as error:
        raise SystemExit(
            f"failed to read package.version from {revision}:{SDK_MANIFEST}"
        ) from error


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Require a plugin release to use the host SDK version."
    )
    parser.add_argument("--release", default="HEAD")
    parser.add_argument("--host", default="origin/main")
    args = parser.parse_args()

    release_version = sdk_version(args.release)
    host_version = sdk_version(args.host)
    if release_version != host_version:
        raise SystemExit(
            "plugin release SDK does not match the host SDK: "
            f"release={release_version} host={host_version}. "
            "Sync main into publish before releasing."
        )

    print(f"plugin release SDK matches host SDK: {release_version}")


if __name__ == "__main__":
    main()
