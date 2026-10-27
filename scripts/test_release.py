"""Archive safety and completeness checks; fixtures are headers, not runnable apps."""

import json
from pathlib import Path
import struct
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import release


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.output = self.root / "assets"

    def fixture(self, target, binary="renamed-app"):
        if target.endswith("windows-msvc"):
            data = bytearray(128)
            data[:2] = b"MZ"
            struct.pack_into("<I", data, 60, 64)
            data[64:70] = b"PE\0\0\x64\x86"
        elif target.endswith("linux-gnu"):
            data = bytearray(64)
            data[:6] = b"\x7fELF\x02\x01"
            data[18:20] = b"\x3e\x00"
        else:
            cpu = b"\x0c\x00\x00\x01" if target.startswith("aarch64") else b"\x07\x00\x00\x01"
            data = b"\xcf\xfa\xed\xfe" + cpu + b"fixture"
        path = self.root / target / "release" / release.executable_name(binary, target)
        path.parent.mkdir(parents=True)
        path.write_bytes(data)
        return path

    def test_all_four_packages_preserve_bytes_and_names(self):
        for target in release.TARGETS:
            self.fixture(target)
            release.package(target, self.output, "renamed-app", self.root)
        release.verify_all(self.output, "renamed-app")

    def test_missing_and_empty_build_fail(self):
        target = "x86_64-unknown-linux-gnu"
        with self.assertRaises(ValueError):
            release.package(target, self.output, "renamed-app", self.root)
        path = self.fixture(target)
        path.write_bytes(b"")
        with self.assertRaises(ValueError):
            release.package(target, self.output, "renamed-app", self.root)

    def test_incomplete_asset_set_fails(self):
        self.output.mkdir()
        with self.assertRaises(ValueError):
            release.verify_all(self.output, "renamed-app")

    def test_wrong_cpu_fails(self):
        path = self.fixture("x86_64-apple-darwin")
        with self.assertRaises(ValueError):
            release.verify_machine(path.read_bytes(), "aarch64-apple-darwin")

    def test_unexpected_zip_paths_and_corruption_fail(self):
        archive = self.root / "bad.zip"
        with zipfile.ZipFile(archive, "w") as output:
            output.writestr("../renamed-app.exe", b"unsafe")
        with self.assertRaises(ValueError):
            release.verify_archive(archive, "renamed-app", "x86_64-pc-windows-msvc")
        archive.write_bytes(b"not a ZIP")
        with self.assertRaises(zipfile.BadZipFile):
            release.verify_archive(archive, "renamed-app", "x86_64-pc-windows-msvc")

    def test_nonexecutable_tar_member_fails(self):
        archive = self.root / "bad.tar.gz"
        with tarfile.open(archive, "w:gz") as output:
            info = tarfile.TarInfo("renamed-app")
            info.mode = 0o644
            output.addfile(info)
        with self.assertRaises(ValueError):
            release.verify_archive(archive, "renamed-app", "x86_64-unknown-linux-gnu")

    def test_tag_must_match_manifest_version(self):
        release.validate_tag("v0.1.0", "0.1.0")
        for tag in ["main", "v0.2.0", "v0.1.0\nsha=unexpected"]:
            with self.assertRaises(ValueError):
                release.validate_tag(tag, "0.1.0")

    @patch("release.subprocess.run")
    def test_binary_name_comes_from_cargo_metadata(self, run):
        run.return_value.stdout = json.dumps({
            "workspace_default_members": ["app"],
            "packages": [{"id": "app", "version": "0.1.0", "targets": [
                {"name": "custom-binary", "kind": ["bin"]},
            ]}],
            "target_directory": str(self.root),
        })
        self.assertEqual(release.metadata(), ("custom-binary", "0.1.0", self.root))


if __name__ == "__main__":
    unittest.main()
