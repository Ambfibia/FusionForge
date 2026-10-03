"""Validate the complete key/alias namespace before a normalizer writes files."""


def renamed_aliases(entry: dict, wanted: str) -> list[str]:
    aliases = set(entry.get("aliases", [])) | {entry["logicalKey"]}
    return sorted(alias for alias in aliases if alias.lower() != wanted.lower())


def validate_key_plan(entries: list[dict], replacements: dict[int, str]) -> None:
    """Do not steal another entry's key, even if that entry is also renamed."""
    owners: dict[str, str] = {}
    for entry in entries:
        wanted = replacements.get(id(entry), entry["logicalKey"])
        aliases = (
            renamed_aliases(entry, wanted)
            if id(entry) in replacements
            else entry.get("aliases", [])
        )
        for key in [wanted, *aliases]:
            folded = key.lower()
            if not key.strip() or folded in owners:
                raise SystemExit(
                    f"audio key/alias collision at {key!r}: "
                    f"{owners.get(folded)!r} / {entry['trueName']!r}; nothing written"
                )
            owners[folded] = entry["trueName"]
