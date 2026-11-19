#!/usr/bin/env python3
# Whether a crate packed here is the one crates.io serves at the same version, for
# crates-publish.sh, which skips a version crates.io holds: a crate that changed since needs a new
# version, or no project that builds from crates.io would ever get the change.
#
#   .github/scripts/crate-matches.py <crate> <version> <packed .crate>
#
# Exits 0 where the packed file is the one crates.io serves, by the SHA-256 its index records, or
# holds the same files but two `cargo package` writes of its own: .cargo_vcs_info.json, the commit
# it packed from, and Cargo.lock, the workspace's lock cut to the crate, which moves with any other
# crate's dependencies and which a library's dependents never read. So a crate unchanged since,
# packed from a later commit, still matches. Exits 1, naming the files that differ, where any other
# file does, its manifest as cargo writes it included; 2 where crates.io cannot say.
import hashlib
import io
import json
import sys
import tarfile
import urllib.request

AGENT = "reylo-release (github.com/reylosql/reylo)"
# What the packing writes of the checkout rather than of the crate.
PACKING = {".cargo_vcs_info.json", "Cargo.lock"}


def fetch(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": AGENT})
    with urllib.request.urlopen(request, timeout=60) as answer:
        return answer.read()


def files(archive: bytes, root: str) -> dict[str, bytes]:
    """Each regular file of a .crate by its path inside the crate, those the packing writes aside."""
    out = {}
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as tar:
        for member in tar.getmembers():
            if not member.isfile():
                continue
            name = member.name.removeprefix(f"{root}/")
            if name not in PACKING:
                out[name] = tar.extractfile(member).read()
    return out


def main() -> int:
    crate, version, packed_path = sys.argv[1:4]
    with open(packed_path, "rb") as f:
        packed = f.read()
    try:
        cksum = json.loads(fetch(f"https://crates.io/api/v1/crates/{crate}/{version}"))["version"]["checksum"]
    except Exception as e:  # noqa: BLE001
        print(f"crates.io did not give the checksum of {crate} {version}: {e}", file=sys.stderr)
        return 2
    if hashlib.sha256(packed).hexdigest() == cksum:
        print(f"{crate} {version} is on crates.io already, as packed here")
        return 0
    try:
        served = fetch(f"https://static.crates.io/crates/{crate}/{crate}-{version}.crate")
    except Exception as e:  # noqa: BLE001
        print(f"crates.io did not serve {crate} {version}: {e}", file=sys.stderr)
        return 2
    if hashlib.sha256(served).hexdigest() != cksum:
        print(f"crates.io served a file for {crate} {version} that is not the one its index records", file=sys.stderr)
        return 2
    root = f"{crate}-{version}"
    ours, theirs = files(packed, root), files(served, root)
    differ = sorted(name for name in ours.keys() | theirs.keys() if ours.get(name) != theirs.get(name))
    if not differ:
        print(f"{crate} {version} is on crates.io already, with the same files, packed from another commit or lock")
        return 0
    shown = ", ".join(differ[:10]) + (f" and {len(differ) - 10} more" if len(differ) > 10 else "")
    print(f"{crate} {version} is on crates.io with other files than this checkout packs ({shown}), and crates.io never takes a version twice: give {crate} a new version (a parser copy's in its own Cargo.toml and where the root Cargo.toml's [workspace.dependencies] pins it; any other crate's with the release's)", file=sys.stderr)
    return 1


sys.exit(main())
