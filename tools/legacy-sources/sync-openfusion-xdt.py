"""Sync an OpenFusion `tdata/xdt.json` to the native client's installed tables.

The shard reads its gameplay numbers out of `xdt.json`, so every row FFOne added
above the stock table set -- nanos, skills, every equipment family, the added NPC
mesh and icon rows -- is invisible to the server until that file carries them
too. This tool replaces each shared table section with the installed
`data/tables/table-set.json` rows and keeps everything the server document has
that the client one does not (the Unity MonoBehaviour wrapper keys `m_Name`,
`m_Script`, and friends).

It is deliberately not a blind copy. Before writing anything it proves:

* no section shrinks, and no section the server has disappears;
* no row field the server document carries is missing from the client rows, so
  no field OpenFusion reads can vanish;
* every index reference (`m_iItemName` into `m_pItemStringData`, `m_iMesh` into
  `m_pNpcMeshData`, and the rest below) still lands inside its section;
* every value difference in a section the server already had is reported, split
  into differences that are float re-serialisation only and differences that are
  real, so a behavioural change can never slip through unnoticed.

The pre-image is written outside the OpenFusion repository before the file is
replaced.

    python tools/legacy-sources/sync-openfusion-xdt.py ../OpenFusion/tdata
    python tools/legacy-sources/sync-openfusion-xdt.py ../OpenFusion/tdata --apply
"""

from __future__ import annotations

import argparse
import json
import math
import os
import shutil
import sys
from pathlib import Path

FFONE_TABLE_SET = Path(
    r"D:/CodexProject/FusionFallProject/FFOneClient/assets/game/data/tables/table-set.json"
)
BACKUP_ROOT = Path("work/legacy-sources/openfusion-xdt")

# Index fields and the section each one indexes. Anything not listed here is a
# plain value, not a reference; the audit says how many references it checked so
# a silently empty map cannot pass for a clean result.
REFERENCE_FIELDS = {
    "m_pNpcData": {
        "m_iNpcName": "m_pNpcStringData",
        "m_iMesh": "m_pNpcMeshData",
        "m_iIcon1": "m_pNpcIconData",
        "m_iIcon2": "m_pNpcIconData",
        "m_iBarkerNumber": "m_pNpcBarkerData",
        "m_iServiceNumber": "m_pNpcServiceData",
        "m_iGroupNumber": "m_pNpcGroupData",
    },
    "m_pItemData": {
        "m_iItemName": "m_pItemStringData",
        "m_iComment": "m_pItemStringData",
        "m_iMesh": "m_pItemMeshData",
        "m_iIcon": "m_pItemIconData",
        "m_iIcon1": "m_pItemIconData",
        "m_iIcon2": "m_pItemIconData",
        "m_iSound1": "m_pItemSoundData",
        "m_iSound2": "m_pItemSoundData",
    },
    "m_pNanoData": {
        "m_iNanoName": "m_pNanoStringData",
        "m_iComment": "m_pNanoStringData",
        "m_iMesh": "m_pNanoMeshData",
        "m_iIcon1": "m_pNanoIconData",
        "m_iTune": "m_pNanoTuneData",
    },
    "m_pNanoTuneData": {
        "m_iTuneName": "m_pNanoTuneStringData",
    },
    "m_pSkillData": {
        "m_iIcon": "m_pSkillIconData",
    },
}


def load(path: Path) -> dict:
    return json.loads(path.read_text("utf-8"))


def float_equal(left, right) -> bool:
    """True when two JSON numbers are the same 32-bit float.

    The two documents were serialised by different writers, so the same `f32`
    reaches JSON as `99.80000305175781` in one and `99.8000030517578` in the
    other. That is not a data difference and must not be reported as one.
    """
    if not isinstance(left, (int, float)) or not isinstance(right, (int, float)):
        return False
    if isinstance(left, bool) or isinstance(right, bool):
        return False
    if left == right:
        return True
    if math.isnan(left) and math.isnan(right):
        return True
    import struct

    def as_f32(value: float) -> bytes:
        try:
            return struct.pack("<f", float(value))
        except (OverflowError, ValueError):
            return b""

    packed = as_f32(left)
    return packed != b"" and packed == as_f32(right)


def rows_differ(left, right, real: list, cosmetic: list, path: str) -> None:
    if isinstance(left, dict) and isinstance(right, dict):
        for key in sorted(set(left) | set(right)):
            rows_differ(left.get(key), right.get(key), real, cosmetic, f"{path}.{key}")
        return
    if isinstance(left, list) and isinstance(right, list):
        if len(left) != len(right):
            real.append(path)
            return
        for index, (a, b) in enumerate(zip(left, right)):
            rows_differ(a, b, real, cosmetic, f"{path}[{index}]")
        return
    if left == right:
        return
    if float_equal(left, right):
        cosmetic.append(path)
        return
    real.append(path)


def audit_references(document: dict) -> tuple[int, list[str]]:
    checked = 0
    broken = []
    for table_name, table in document.items():
        if not isinstance(table, dict):
            continue
        for section_name, rows in table.items():
            fields = REFERENCE_FIELDS.get(section_name)
            if not fields or not isinstance(rows, list):
                continue
            for field, target_section in fields.items():
                target = table.get(target_section)
                if not isinstance(target, list):
                    continue
                for index, row in enumerate(rows):
                    if not isinstance(row, dict) or field not in row:
                        continue
                    value = row[field]
                    if not isinstance(value, int) or isinstance(value, bool):
                        continue
                    checked += 1
                    if value < 0 or value >= len(target):
                        broken.append(
                            f"{table_name}.{section_name}[{index}].{field}={value} "
                            f"is outside {target_section} ({len(target)})"
                        )
    return checked, broken


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tdata", type=Path, help="OpenFusion tdata directory")
    parser.add_argument("--apply", action="store_true", help="write the merged xdt.json")
    parser.add_argument("--report", type=Path, default=None)
    parser.add_argument("--table-set", type=Path, default=FFONE_TABLE_SET)
    args = parser.parse_args()

    server_path = args.tdata / "xdt.json"
    server = load(server_path)
    client = load(args.table_set)["tables"][0]["value"]

    merged = dict(server)
    sections = []
    real_changes = []
    cosmetic_changes = 0
    added_rows = 0

    for table_name in sorted(server):
        table = server[table_name]
        if not isinstance(table, dict) or table_name not in client:
            continue
        client_table = client[table_name]
        merged_table = dict(table)
        for section_name, server_rows in table.items():
            if section_name not in client_table:
                continue
            client_rows = client_table[section_name]
            if not isinstance(server_rows, list) or not isinstance(client_rows, list):
                merged_table[section_name] = client_rows
                continue
            if len(client_rows) < len(server_rows):
                return fail(
                    f"{table_name}.{section_name} would shrink from "
                    f"{len(server_rows)} to {len(client_rows)} rows"
                )
            server_fields = set()
            client_fields = set()
            for row in server_rows:
                if isinstance(row, dict):
                    server_fields |= set(row)
            for row in client_rows:
                if isinstance(row, dict):
                    client_fields |= set(row)
            lost = sorted(server_fields - client_fields)
            if lost:
                return fail(f"{table_name}.{section_name} would lose fields {lost}")

            real: list[str] = []
            cosmetic: list[str] = []
            for index in range(len(server_rows)):
                rows_differ(
                    server_rows[index],
                    client_rows[index],
                    real,
                    cosmetic,
                    f"{table_name}.{section_name}[{index}]",
                )
            cosmetic_changes += len(cosmetic)
            if real:
                real_changes.append(
                    {
                        "section": f"{table_name}.{section_name}",
                        "changedValues": len(real),
                        "examples": real[:4],
                    }
                )
            added = len(client_rows) - len(server_rows)
            added_rows += added
            if added or real or cosmetic:
                sections.append(
                    {
                        "section": f"{table_name}.{section_name}",
                        "serverRows": len(server_rows),
                        "clientRows": len(client_rows),
                        "addedRows": added,
                        "changedValues": len(real),
                        "floatReserialisations": len(cosmetic),
                    }
                )
            merged_table[section_name] = client_rows
        merged[table_name] = merged_table

    checked, broken = audit_references(merged)
    if broken:
        return fail(
            f"{len(broken)} index references would fall outside their section, "
            f"first: {broken[0]}"
        )

    report = {
        "schema": "ffone.openfusion-xdt-sync.v1",
        "server": str(server_path),
        "tableSet": str(args.table_set),
        "applied": False,
        "counts": {
            "sectionsTouched": len(sections),
            "addedRows": added_rows,
            "changedValues": sum(entry["changedValues"] for entry in real_changes),
            "floatReserialisations": cosmetic_changes,
            "referencesChecked": checked,
            "brokenReferences": 0,
        },
        "sections": sections,
        "realValueChanges": real_changes,
    }

    if args.apply:
        BACKUP_ROOT.mkdir(parents=True, exist_ok=True)
        backup = BACKUP_ROOT / "xdt.before.json"
        shutil.copyfile(server_path, backup)
        staged = server_path.with_suffix(".json.next")
        # Match the server document's byte conventions exactly -- two-space
        # indent, CRLF -- so the diff is the data change and nothing else.
        body = json.dumps(merged, indent=2, ensure_ascii=False) + "\n"
        staged.write_bytes(body.replace("\n", "\r\n").encode("utf-8"))
        os.replace(staged, server_path)
        report["applied"] = True
        report["backup"] = str(backup)

    text = json.dumps(report, indent=1, ensure_ascii=False)
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(text + "\n", "utf-8")
    print(f"sections touched : {len(sections)}")
    print(f"rows added       : {added_rows}")
    print(f"real value changes: {report['counts']['changedValues']}")
    print(f"float re-serialisations: {cosmetic_changes}")
    print(f"index references checked: {checked}, broken: 0")
    print(f"applied          : {report['applied']}")
    for entry in sections[:20]:
        print(
            f"  {entry['section']:44} +{entry['addedRows']:4} rows, "
            f"{entry['changedValues']} changed, {entry['floatReserialisations']} float"
        )
    for entry in real_changes[:10]:
        print(f"  REAL CHANGE {entry['section']}: {entry['examples']}")
    return 0


def fail(message: str) -> int:
    print(f"refusing to sync: {message}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
