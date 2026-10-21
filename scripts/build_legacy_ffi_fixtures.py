#!/usr/bin/env python3
"""Build frozen plugin source against immutable historical SDK revisions.

These artifacts must never depend on the candidate SDK through a path patch.
The new host loads them without recompiling their SDK during compatibility tests.
"""

from pathlib import Path
import os
import subprocess


ROOT = Path(__file__).resolve().parents[1]
REVISIONS = {
    "5.1.1": "a8d8be5066e7f77273f6e918668f65fda7758198",
    "5.1.2": "507f9fd51f687fa4e3d2ed2727b2514adf05acf8",
}


def build() -> None:
    for version, revision in REVISIONS.items():
        directory = ROOT / "target" / "ffi-fixtures" / version
        (directory / "src").mkdir(parents=True, exist_ok=True)
        (directory / "src/lib.rs").write_bytes(
            (ROOT / "sdk/tests/fixtures/legacy-app.rs").read_bytes()
        )
        (directory / "Cargo.toml").write_text(
            '[workspace]\n[package]\nname = "ffi-legacy"\n'
            'version = "0.0.0"\nedition = "2024"\n'
            '[lib]\ncrate-type = ["cdylib"]\n[dependencies]\n'
            f'aomi-sdk = {{ git = "https://github.com/aomi-labs/aomi-sdk", rev = "{revision}", version = "={version}" }}\n'
            'serde = { version = "1", features = ["derive"] }\n'
            'schemars = "1.0.4"\nserde_json = "1"\n'
        )
        subprocess.run(
            [os.environ.get("CARGO", "cargo"), "build", "--manifest-path", str(directory / "Cargo.toml"),
             "--target-dir", str(directory / "target")],
            check=True,
        )


if __name__ == "__main__":
    build()
