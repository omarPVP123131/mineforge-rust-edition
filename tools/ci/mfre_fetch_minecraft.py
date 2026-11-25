#!/usr/bin/env python3
"""Downloads the pinned Minecraft for a CI runner that has none (MFRE fork only).

Upstream never downloads the game: its self-hosted runners hold the maintainer's own imported
copy (tools/ci/vanilla-data.sh). GitHub-hosted runners have nothing, so the fork's build imports
the pinned version itself, from Mojang's own public CDN, with every SHA-1 checked against
research-lock.json and the version JSON — the same pins the launcher's internal download uses
(D26: the fork downloads Mojang internally once the EULA is accepted; the fork's CI runs on the
maintainer's behalf, who accepted it).

  python3 tools/ci/mfre_fetch_minecraft.py --dest .local/ci-minecraft

Writes into --dest:
  26.3.json        the pinned version JSON (its SHA-1 is research-lock's manifest_sha1 or one
                   of the accepted republications)
  client.jar       the pinned client jar (SHA-1 from the version JSON)
  server.jar       the pinned server jar (SHA-1 from the version JSON)
  libraries/       every library of the JSON, in Maven layout (each SHA-1 checked)
  assets/          the asset store, indexes/ and objects/ (each SHA-1 checked)

A full `horizon-prepare --prepare` reads all of it (the resources come from the asset store),
which is what the fork's CI needs before `compile-exporters`.

Exits 0 when complete (already-downloaded, already-verified files are kept), non-zero on any
mismatch or failed download: nothing half-verified is ever left named as if it were.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sys
import urllib.request

PISTON_META = "https://piston-meta.mojang.com"
RESOURCES = "https://resources.download.minecraft.net"
USER_AGENT = "mineforge-rust-edition-ci (research-lock pinned; contact the fork maintainer)"


def fail(message: str) -> "NoReturn":  # type: ignore[name-defined]
    print(f"::error::{message}", file=sys.stderr)
    raise SystemExit(1)


def read(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=120) as answer:
        return answer.read()


def sha1(data: bytes) -> str:
    return hashlib.sha1(data).hexdigest()


def put_verified(path: pathlib.Path, data: bytes, expected: str, what: str) -> None:
    if path.is_file():
        have = sha1(path.read_bytes())
        if have == expected:
            print(f"  kept {what}: {path}")
            return
        path.unlink()
    actual = sha1(data)
    if actual != expected:
        fail(f"{what}: SHA-1 {actual} is not the pinned {expected}")
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".part")
    tmp.write_bytes(data)
    tmp.replace(path)
    print(f"  fetched {what}: {path} ({len(data)} bytes)")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--dest", required=True, type=pathlib.Path)
    parser.add_argument("--root", default=".", type=pathlib.Path,
                        help="the checkout with research-lock.json")
    parser.add_argument("--no-assets", action="store_true",
                        help="skip the asset store (jars and libraries only; a local smoke test)")
    args = parser.parse_args()

    lock = json.loads((args.root / "research-lock.json").read_text())["minecraft"]
    version = lock["version"]
    accepted = [lock["manifest_sha1"]] + list(lock.get("accepted_manifest_sha1", []))
    dest = args.dest
    dest.mkdir(parents=True, exist_ok=True)

    # The version JSON: Mojang's package URL names it by its own SHA-1; any accepted SHA-1 whose
    # JSON downloads is the pinned one (republications differ only in the asset index).
    json_path = dest / f"{version}.json"
    document = None
    if json_path.is_file():
        data = json_path.read_bytes()
        if sha1(data) in accepted:
            document = json.loads(data)
            print(f"  kept version JSON: {json_path}")
    if document is None:
        for digest in accepted:
            url = f"{PISTON_META}/v1/packages/{digest}/{version}.json"
            try:
                data = read(url)
            except Exception as error:  # noqa: BLE001 - try the next accepted republication
                print(f"  {url}: {error}")
                continue
            if sha1(data) != digest:
                fail(f"version JSON from {url}: SHA-1 does not match its URL")
            json_path.write_bytes(data)
            document = json.loads(data)
            print(f"  fetched version JSON: {json_path} ({digest[:8]})")
            break
    if document is None:
        fail(f"no accepted {version} version JSON could be downloaded")

    downloads = document.get("downloads", {})
    for name, path in (("client", dest / "client.jar"), ("server", dest / "server.jar")):
        entry = downloads.get(name)
        if not entry:
            fail(f"the version JSON has no {name} download")
        url = entry["url"]
        data = path.read_bytes() if path.is_file() and sha1(path.read_bytes()) == entry["sha1"] \
            else read(url)
        put_verified(path, data, entry["sha1"], f"{name} jar")

    libraries = dest / "libraries"
    count = 0
    for entry in document.get("libraries", []):
        # Every library with a plain artifact, whatever its rules name: the import copies the
        # client side and the server bundle unpacks its own; nothing here runs the game, and a
        # library missing that the import lists is a hard failure there.
        artifact = (entry.get("downloads") or {}).get("artifact")
        if not artifact:
            continue
        relative = artifact["path"]
        path = libraries / relative
        if path.is_file() and sha1(path.read_bytes()) == artifact["sha1"]:
            continue
        put_verified(path, read(artifact["url"]), artifact["sha1"], f"library {relative}")
        count += 1
    print(f"  libraries: {count} fetched")

    if args.no_assets:
        print(f"mfre_fetch_minecraft: {version} jars+libraries ready in {dest} (no assets)")
        return

    # The asset store: the index the version JSON names (its SHA-1 is one research-lock accepts)
    # and every object it lists, from Mojang's resource CDN, 32 downloads at a time.
    from concurrent.futures import ThreadPoolExecutor

    index_entry = document.get("assetIndex") or {}
    index_path = dest / "assets" / "indexes" / f"{index_entry.get('id', version)}.json"
    accepted_indexes = set(lock.get("accepted_asset_index_sha1", []))
    if not (index_path.is_file() and sha1(index_path.read_bytes()) in accepted_indexes):
        data = read(index_entry["url"])
        if accepted_indexes and sha1(data) not in accepted_indexes:
            fail(f"asset index: SHA-1 {sha1(data)} is none research-lock accepts")
        put_verified(index_path, data, sha1(data), f"asset index {index_entry.get('id')}")
    index = json.loads(index_path.read_bytes())
    objects = dest / "assets" / "objects"

    def fetch(item: tuple[str, dict]) -> str:
        name, entry = item
        digest = entry["hash"]
        path = objects / digest[:2] / digest
        if path.is_file() and sha1(path.read_bytes()) == digest:
            return ""
        put_verified(path, read(f"{RESOURCES}/{digest[:2]}/{digest}"), digest, f"asset {name}")
        return name

    with ThreadPoolExecutor(max_workers=32) as pool:
        fetched = [name for name in pool.map(fetch, index["objects"].items()) if name]
    print(f"mfre_fetch_minecraft: {version} ready in {dest} "
          f"({count} libraries, {len(fetched)} assets fetched, "
          f"{len(index['objects']) - len(fetched)} kept)")


if __name__ == "__main__":
    main()
