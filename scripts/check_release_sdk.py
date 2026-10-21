#!/usr/bin/env python3
from __future__ import annotations

import argparse
import subprocess
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SDK_MANIFEST = "sdk/Cargo.toml"


def sdk_manifest(revision: str) -> dict:
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

    return tomllib.loads(result.stdout)


def sdk_version(revision: str) -> str:
    manifest = sdk_manifest(revision)
    try:
        return manifest["package"]["version"]
    except KeyError as error:
        raise SystemExit(
            f"failed to read package.version from {revision}:{SDK_MANIFEST}"
        ) from error


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Require a plugin release to use a host-compatible FFI version."
    )
    parser.add_argument("--release", default="HEAD")
    parser.add_argument("--host", default="origin/main")
    args = parser.parse_args()

    release_version = sdk_version(args.release)
    host_version = sdk_version(args.host)
    host_ffi = sdk_manifest(args.host).get("package", {}).get("metadata", {}).get("aomi", {}).get("ffi_version")
    release_ffi = sdk_manifest(args.release).get("package", {}).get("metadata", {}).get("aomi", {}).get("ffi_version")
    if release_ffi is None:
        release_ffi = {"5.1.1": 1, "5.1.2": 1}.get(release_version)
    if host_ffi is not None:
        if type(host_ffi) is not int or type(release_ffi) is not int or host_ffi <= 0 or release_ffi != host_ffi:
            raise SystemExit(f"plugin release FFI mismatch: release={release_ffi} host={host_ffi}")
        print(f"plugin release FFI matches host: {release_ffi} (SDK {release_version})")
    elif release_version != host_version:
        raise SystemExit(
            "plugin release SDK does not match the host SDK: "
            f"release={release_version} host={host_version}. "
            "Sync main into publish before releasing."
        )

    else:
        print(f"plugin release SDK matches legacy host SDK: {release_version}")


if __name__ == "__main__":
    main()
