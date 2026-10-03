"""Plan which source AudioClips belong to which installed nano.

Matching clip names against the nano's name does not work, and failing quietly is
its worst property. A nano's clips can be spelled three different ways at once:
the model is `nano_princessbubblegum`, the table calls her `P.Bubblegum`, and her
clips are `PBubblegum_*` and `P.Bubblegum_*`. Ampfibian's whole set is spelled
`Amphibian_*`. Buttercup's is `BtrCup_*`, Samurai Jack's `SJack_*`, Johnny Bravo's
`JBravo_*`. No normalisation of a display name produces those.

What does work is the packaging. Each `Nano_0NN.resourceFile` carries exactly one
nano's voice set, and `NN` is that nano's row in the *source build's own* nano
table -- so the bundle name states the ownership and no spelling is involved. The
rule is checked before it is used: for 48 of 57 bundles the dominant clip prefix
also matches the row's name or mesh outright, and every remaining bundle is
confirmed by the clips FFOne has already installed for that nano. Any bundle that
satisfies neither is reported rather than assigned.

Clips inside a bundle that do not share its dominant prefix -- a stray
`NanDismiss` trio filed next door -- are resolved by looking their prefix up
among the other bundles' dominant prefixes, so they follow the same evidence.

    python tools/legacy-sources/plan-nano-voice.py --native-target ../FFOneClient/assets/game \\
      --source-table work/legacy-sources/nano-voice-20260903/donor-xdt.json \\
      --bundle-index ../builds/6543a2bb-d154-4087-b9ee-3c8aa778580a.ffclient/cache/bundle-index.json \\
      --listing work/legacy-sources/nano-voice-20260903/listing \\
      --out work/legacy-sources/nano-voice-20260903/clips.tsv
"""

from __future__ import annotations

import argparse
import collections
import json
import re
import sys
from pathlib import Path

CLIP_PREFIX = re.compile(r"_(?=nan|pwr|comm)")
BUNDLE = re.compile(r"Nano_(\d+)\.resourceFile")


def normalise(value: str) -> str:
    return re.sub(r"[^a-z0-9]", "", (value or "").lower())


def clip_prefix(stem: str) -> str:
    return normalise(CLIP_PREFIX.split(stem.lower(), maxsplit=1)[0])


def find_nano_table(document):
    if isinstance(document, dict):
        if "m_pNanoTable" in document:
            return document["m_pNanoTable"]
        for value in document.values():
            found = find_nano_table(value)
            if found is not None:
                return found
    elif isinstance(document, list):
        for value in document:
            found = find_nano_table(value)
            if found is not None:
                return found
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-target", type=Path, required=True)
    parser.add_argument("--source-table", type=Path, required=True, help="dump-xdt of the source build")
    parser.add_argument("--bundle-index", type=Path, required=True)
    parser.add_argument("--listing", type=Path, required=True, help="fusionforge list-contents dumps")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--report", type=Path, default=None)
    parser.add_argument(
        "--trust-bundle-rule",
        action="store_true",
        help="assign a bundle whose spelling nothing else confirms, once the rule itself is confirmed elsewhere",
    )
    args = parser.parse_args()

    assets = args.native_target
    registry = json.loads((assets / "_runtime/characters.json").read_text("utf-8"))
    catalog = json.loads((assets / "_runtime/audio.json").read_text("utf-8"))
    owner_of_model = {
        model["logicalName"]: model["glb"].split("/")[-2]
        for model in registry["models"]
        if model["category"] == "nano"
    }

    table = find_nano_table(json.loads(args.source_table.read_text("utf-8")))
    if table is None:
        raise SystemExit("source table has no m_pNanoTable")
    rows, meshes, strings = table["m_pNanoData"], table["m_pNanoMeshData"], table["m_pNanoStringData"]
    row_model, row_name = {}, {}
    for index in range(1, len(rows)):
        mesh_index, name_index = rows[index].get("m_iMesh", 0), rows[index].get("m_iNanoName", 0)
        if 0 <= mesh_index < len(meshes):
            row_model[index] = (meshes[mesh_index].get("m_pstrMMeshModelString") or "").strip()
        if 0 <= name_index < len(strings):
            row_name[index] = (strings[name_index].get("m_strName") or "").strip()

    # What FFOne already installed, so a bundle whose spelling is an abbreviation
    # can still be confirmed against real installed clips.
    installed_prefixes: dict[str, set[str]] = collections.defaultdict(set)
    installed_true_names = set()
    for entry in catalog["assets"]:
        installed_true_names.add(normalise(entry["trueName"]))
        installed_prefixes[entry["owner"]].add(clip_prefix(entry["trueName"]))

    index_document = json.loads(args.bundle_index.read_text("utf-8"))
    dominant: dict[int, tuple[str, int]] = {}
    for bundle in index_document["bundles"]:
        match = BUNDLE.fullmatch(bundle["name"])
        if not match:
            continue
        counts = collections.Counter()
        for asset in bundle.get("assets", []):
            for route in asset.get("containerPaths") or []:
                lowered = route.lower()
                if lowered.endswith(".wav"):
                    counts[clip_prefix(lowered.rsplit("/", 1)[-1][:-4])] += 1
        if counts:
            dominant[int(match.group(1))] = counts.most_common(1)[0]

    catalog_owners = {entry["owner"] for entry in catalog["assets"]}
    prefix_owner: dict[str, str] = {}
    evidence, unresolved, trusted = [], [], []
    for index, (prefix, count) in sorted(dominant.items()):
        model = row_model.get(index, "")
        owner = owner_of_model.get(model)
        if owner is None and model in catalog_owners:
            # The model is not installed but the catalogue already owns clips
            # under that exact name, which is evidence enough to keep them
            # together rather than split the set.
            owner = model
        if owner is None:
            unresolved.append({"bundle": f"Nano_{index:03}", "prefix": prefix, "reason": f"source row {index} names model {model!r}, which is neither installed nor an existing audio owner"})
            continue
        by_name = prefix in {normalise(row_name.get(index, "")), normalise(model.removeprefix("nano_"))}
        by_installed = prefix in installed_prefixes.get(owner, set())
        if by_name or by_installed:
            prefix_owner[prefix] = owner
            evidence.append({
                "bundle": f"Nano_{index:03}",
                "sourceRow": index,
                "sourceName": row_name.get(index),
                "model": model,
                "owner": owner,
                "prefix": prefix,
                "clips": count,
                "confirmedBy": "source row name" if by_name else "clips already installed for this nano",
            })
            continue
        # Nothing but the packaging says who these belong to. The bundle rule
        # itself is confirmed by every other bundle, so applying it here is a
        # decision the caller makes explicitly and the report records.
        record = {
            "bundle": f"Nano_{index:03}",
            "sourceRow": index,
            "sourceName": row_name.get(index),
            "model": model,
            "owner": owner,
            "prefix": prefix,
            "clips": count,
            "reason": "the clip spelling is an abbreviation that neither the source row name nor any installed clip repeats",
        }
        if args.trust_bundle_rule:
            prefix_owner[prefix] = owner
            trusted.append(record)
        else:
            unresolved.append(record)

    # A stray trio is usually filed under the nano's display name rather than the
    # bundle spelling ("Unstable Nano" beside the Unstable set spelled
    # "Unstable_*"). The source table already states that name, so resolving by it
    # stays inside the same evidence.
    for index, name in row_name.items():
        model = row_model.get(index, "")
        owner = owner_of_model.get(model) or (model if model in catalog_owners else None)
        key = normalise(name)
        if owner and key and key not in prefix_owner:
            prefix_owner[key] = owner
            evidence.append({
                "bundle": None,
                "sourceRow": index,
                "sourceName": name,
                "model": model,
                "owner": owner,
                "prefix": key,
                "clips": 0,
                "confirmedBy": "source row name",
            })

    planned, seen, orphan = [], set(), collections.Counter()
    for listing in sorted(args.listing.glob("*.txt")):
        for line in listing.read_text("utf-8", errors="replace").splitlines():
            parts = line.split("\t")
            if len(parts) < 4 or parts[2] != "AudioClip":
                continue
            path_id, true_name = parts[0], parts[3]
            prefix = clip_prefix(true_name)
            owner = prefix_owner.get(prefix)
            if owner is None:
                orphan[prefix] += 1
                continue
            if normalise(true_name) in installed_true_names or (owner, true_name) in seen:
                continue
            seen.add((owner, true_name))
            planned.append((owner, listing.stem, path_id, true_name))

    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("w", encoding="utf-8", newline="\n") as handle:
        for row in planned:
            handle.write("\t".join(row) + "\n")

    counts = collections.Counter(row[0] for row in planned)
    report = {
        "schema": "ffone.nano-voice-plan.v2",
        "clips": len(planned),
        "owners": sorted(counts),
        "ownershipEvidence": evidence,
        "bundleRuleConfirmations": len(evidence),
        "assignedOnTheBundleRuleAlone": trusted,
        "unresolvedBundles": unresolved,
        "clipPrefixesWithNoOwner": [
            {"prefix": prefix, "clips": count} for prefix, count in orphan.most_common() if count >= 3
        ],
    }
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=1, ensure_ascii=False) + "\n", "utf-8")
    print(f"ownership proved for {len(prefix_owner)} nano voice sets; {len(unresolved)} bundles unresolved")
    print(f"clips to publish: {len(planned)}")
    for owner, count in counts.most_common():
        print(f"  {owner:26} {count:4}")
    for entry in trusted:
        print(f"  BUNDLE RULE ONLY {entry['bundle']} prefix={entry['prefix']} -> {entry['owner']} ({entry['clips']} clips)")
    for entry in unresolved:
        print(f"  UNRESOLVED {entry}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
