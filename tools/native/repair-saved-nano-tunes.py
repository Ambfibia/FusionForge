"""Repair only unambiguous saved skills caused by the proven tuning-map collision.

Run against an idle local server. Preview is read-only; --apply requires an
unused backup path below Editor work and uses one SQLite transaction.
"""
import argparse
import collections
import json
from pathlib import Path
import sqlite3


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--database", type=Path, required=True)
    parser.add_argument("--previous-table", type=Path, required=True)
    parser.add_argument("--native-table", type=Path, required=True)
    parser.add_argument("--backup", type=Path, required=True)
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()
    def nano_table(path):
        document = json.loads(path.read_text(encoding="utf-8"))
        return next(t["value"]["m_pNanoTable"] for t in document["tables"]
                    if "m_pNanoTable" in t["value"])
    before = nano_table(args.previous_table)
    after = nano_table(args.native_table)
    wire = {t["m_iTuneNumber"]: t for t in before["m_pNanoTuneData"]}
    candidates = collections.defaultdict(set)
    for nano in before["m_pNanoData"]:
        nano_id = nano["m_iNanoNumber"]
        if nano_id <= 0:
            continue
        allowed = {after["m_pNanoTuneData"][i]["m_iSkillID"] for i in nano["m_iTune"]}
        for index in nano["m_iTune"]:
            tune = before["m_pNanoTuneData"][index]
            wrong = wire[tune["m_iTuneNumber"]]["m_iSkillID"]
            intended = after["m_pNanoTuneData"][index]["m_iSkillID"]
            if wrong > 0 and wrong not in allowed and intended > 0:
                candidates[nano_id, wrong].add(intended)
    fixes = [(n, wrong, next(iter(values))) for (n, wrong), values in candidates.items()
             if len(values) == 1]
    uri = args.database.resolve().as_uri()
    with sqlite3.connect(uri + "?mode=ro", uri=True) as read:
        counts = [(n, wrong, intended, read.execute(
            "SELECT count(*) FROM Nanos WHERE ID=? AND Skill=?", (n, wrong)).fetchone()[0])
            for n, wrong, intended in fixes]
        counts = [row for row in counts if row[3]]
        if args.apply and counts:
            backup = args.backup.resolve()
            backup.relative_to((Path.cwd() / "work").resolve())
            assert not backup.exists(), "Never overwrite a save backup"
            backup.parent.mkdir(parents=True, exist_ok=True)
            with sqlite3.connect(backup) as destination:
                read.backup(destination)
    if args.apply and counts:
        with sqlite3.connect(uri + "?mode=rw", uri=True) as database:
            database.execute("BEGIN IMMEDIATE")
            for n, wrong, intended, count in counts:
                result = database.execute("UPDATE Nanos SET Skill=? WHERE ID=? AND Skill=?",
                                          (intended, n, wrong))
                assert result.rowcount == count, "Save changed since preview; roll back"
    print(json.dumps({"applied": args.apply, "corrections": counts}))


if __name__ == "__main__":
    main()
