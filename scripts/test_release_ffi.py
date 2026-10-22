import sys
import unittest
from unittest.mock import patch

import check_release_sdk as guard


class ReleaseFfiTests(unittest.TestCase):
    def run_guard(self, host, release):
        with patch.object(sys, "argv", ["check_release_sdk", "--host", "host", "--release", "release"]), patch.object(
            guard, "sdk_manifest", side_effect=lambda revision: host if revision == "host" else release
        ):
            guard.main()

    def test_ffi_host_accepts_another_sdk_and_verified_legacy(self):
        host = {"package": {"version": "6.0.0", "metadata": {"aomi": {"ffi_version": 1}}}}
        self.run_guard(host, {"package": {"version": "99.0.0", "metadata": {"aomi": {"ffi_version": 1}}}})
        self.run_guard(host, {"package": {"version": "5.1.1"}})
        with self.assertRaises(SystemExit):
            self.run_guard(host, {"package": {"version": "6.0.0", "metadata": {"aomi": {"ffi_version": 2}}}})
        with self.assertRaises(SystemExit):
            self.run_guard(host, {"package": {"version": "5.0.0"}})

    def test_old_host_keeps_its_exact_sdk_requirement(self):
        host = {"package": {"version": "5.1.1"}}
        self.run_guard(host, host)
        with self.assertRaises(SystemExit):
            self.run_guard(host, {"package": {"version": "6.0.0", "metadata": {"aomi": {"ffi_version": 1}}}})


if __name__ == "__main__":
    unittest.main()
