#!/usr/bin/env python3
"""Stage credential-demo as a source-bound local runtime bundle."""

import argparse
import hashlib
import json
import os
import platform
from pathlib import Path
import shutil
import tempfile
import tomllib


APP_NAME = "credential-demo"


def temporary_sibling(destination: Path) -> tuple[int, Path]:
    file_descriptor, name = tempfile.mkstemp(
        dir=destination.parent,
        prefix=f".{destination.name}.",
    )
    return file_descriptor, Path(name)


def atomic_copy(source: Path, destination: Path) -> str:
    file_descriptor, temporary = temporary_sibling(destination)
    os.close(file_descriptor)
    try:
        shutil.copy2(source, temporary)
        digest = hashlib.sha256(temporary.read_bytes()).hexdigest()
        os.replace(temporary, destination)
        return digest
    finally:
        temporary.unlink(missing_ok=True)


def atomic_write_text(destination: Path, content: str) -> None:
    file_descriptor, temporary = temporary_sibling(destination)
    try:
        with os.fdopen(file_descriptor, "w") as output:
            output.write(content)
            output.flush()
            os.fsync(output.fileno())
        os.chmod(temporary, 0o644)
        os.replace(temporary, destination)
    finally:
        temporary.unlink(missing_ok=True)


def sdk_version() -> str:
    manifest_path = Path(__file__).resolve().parents[2] / "Cargo.toml"
    manifest = tomllib.loads(manifest_path.read_text())
    return manifest["package"]["version"]


def host_target() -> str:
    key = (platform.system(), platform.machine())
    targets = {
        ("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
        ("Linux", "aarch64"): "aarch64-unknown-linux-gnu",
        ("Darwin", "arm64"): "aarch64-apple-darwin",
        ("Darwin", "x86_64"): "x86_64-apple-darwin",
    }
    try:
        return targets[key]
    except KeyError:
        raise SystemExit(f"unsupported host for local bundle: {key[0]} {key[1]}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--plugins-root", type=Path, required=True)
    parser.add_argument("--application-id", type=int, required=True)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--release-tag", required=True)
    args = parser.parse_args()

    if args.application_id <= 0:
        raise SystemExit("application id must be positive")
    library = args.library.resolve(strict=True)
    if library.suffix not in {".so", ".dylib", ".dll"}:
        raise SystemExit("library must be a .so, .dylib, or .dll")

    source_dir = (
        args.plugins_root.resolve()
        / "sources"
        / f"application-{args.application_id}"
    )
    source_dir.mkdir(parents=True, exist_ok=True)
    destination = source_dir / f"credential_demo{library.suffix}"
    digest = atomic_copy(library, destination)
    manifest = {
        "app_release_tag": args.release_tag,
        "sdk_version": sdk_version(),
        "target": host_target(),
        "commit": "working-tree",
        "plugins": {
            APP_NAME: {
                "file": destination.name,
                "sha256": digest,
            }
        },
    }
    atomic_write_text(
        source_dir / "manifest.json",
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
    )
    print(source_dir)


if __name__ == "__main__":
    main()
