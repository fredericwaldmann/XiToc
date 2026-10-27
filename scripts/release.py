"""Package and verify the single Cargo binary; no third-party Python dependencies."""

import argparse
import json
from pathlib import Path
import struct
import subprocess
import tarfile
import zipfile

TARGETS = {
    "x86_64-pc-windows-msvc": "XiToc-windows-x64.zip",
    "x86_64-unknown-linux-gnu": "XiToc-linux-x64.tar.gz",
    "aarch64-apple-darwin": "XiToc-macos-arm64.tar.gz",
    "x86_64-apple-darwin": "XiToc-macos-x64.tar.gz",
}


def metadata():
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
        check=True, capture_output=True, text=True,
    )
    data = json.loads(result.stdout)
    packages = [p for p in data["packages"] if p["id"] in data["workspace_default_members"]]
    binaries = [(p, t) for p in packages for t in p["targets"] if "bin" in t["kind"]]
    if len(binaries) != 1:
        raise ValueError("Release packaging requires exactly one default-workspace binary")
    package, binary = binaries[0]
    return binary["name"], package["version"], Path(data["target_directory"])


def validate_tag(tag, version):
    if tag != f"v{version}":
        raise ValueError(f"Release tag {tag!r} must match Cargo.toml version: v{version}")


def executable_name(binary, target):
    return binary + (".exe" if target.endswith("windows-msvc") else "")


def verify_machine(data, target):
    if target.endswith("windows-msvc"):
        if data[:2] != b"MZ" or len(data) < 64:
            raise ValueError("Expected a Windows PE executable")
        offset = struct.unpack_from("<I", data, 60)[0]
        valid = data[offset:offset + 6] == b"PE\0\0\x64\x86"
    elif target.endswith("linux-gnu"):
        valid = data[:6] == b"\x7fELF\x02\x01" and data[18:20] == b"\x3e\x00"
    else:
        cpu = b"\x0c\x00\x00\x01" if target.startswith("aarch64") else b"\x07\x00\x00\x01"
        valid = data[:8] == b"\xcf\xfa\xed\xfe" + cpu
    if not valid:
        raise ValueError(f"Executable format/CPU does not match {target}")


def verify_archive(path, binary, target):
    expected = executable_name(binary, target)
    if path.suffix == ".zip":
        with zipfile.ZipFile(path) as archive:
            if archive.namelist() != [expected]:
                raise ValueError(f"{path}: expected only {expected}")
            data = archive.read(expected)  # Also verifies the ZIP CRC.
    else:
        with tarfile.open(path, "r:gz") as archive:
            members = archive.getmembers()
            if len(members) != 1 or members[0].name != expected:
                raise ValueError(f"{path}: expected only {expected}")
            member = members[0]
            if not member.isfile() or member.mode & 0o111 != 0o111:
                raise ValueError(f"{path}: expected a regular executable file")
            data = archive.extractfile(member).read()
    if not data:
        raise ValueError(f"{path}: empty executable")
    verify_machine(data, target)
    print(f"Verified {path}: {expected}, {len(data)} bytes, {target}")
    return data


def package(target, output, binary, target_directory):
    executable = target_directory / target / "release" / executable_name(binary, target)
    if not executable.is_file() or executable.is_symlink():
        raise ValueError(f"Missing regular executable: {executable}")
    original = executable.read_bytes()
    verify_machine(original, target)
    output.mkdir(parents=True, exist_ok=True)
    destination = output / TARGETS[target]
    if destination.suffix == ".zip":
        with zipfile.ZipFile(destination, "w", zipfile.ZIP_DEFLATED) as archive:
            archive.write(executable, executable.name)
    else:
        with tarfile.open(destination, "w:gz") as archive:
            info = archive.gettarinfo(str(executable), arcname=executable.name)
            info.mode = 0o755
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            with executable.open("rb") as source:
                archive.addfile(info, source)
    if verify_archive(destination, binary, target) != original:
        raise ValueError(f"{destination}: archived bytes differ from build output")


def verify_all(directory, binary):
    actual = {p.name for p in directory.iterdir()}
    if actual != set(TARGETS.values()):
        raise ValueError(f"Expected exactly four release archives; found {sorted(actual)}")
    for target, filename in TARGETS.items():
        verify_archive(directory / filename, binary, target)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    meta = commands.add_parser("metadata")
    meta.add_argument("--tag", required=True)
    pack = commands.add_parser("package")
    pack.add_argument("--target", choices=TARGETS, required=True)
    pack.add_argument("--output", type=Path, default=Path("release-assets"))
    verify = commands.add_parser("verify")
    verify.add_argument("--directory", type=Path, default=Path("release-assets"))
    args = parser.parse_args()
    binary, version, target_directory = metadata()
    if args.command == "metadata":
        validate_tag(args.tag, version)
        print(f"binary={binary}")
        print(f"tag={args.tag}")
        print(f"sha={subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()}")
    elif args.command == "package":
        package(args.target, args.output, binary, target_directory)
    else:
        verify_all(args.directory, binary)


if __name__ == "__main__":
    main()
