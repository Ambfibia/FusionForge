"""Owner-selected Retrobution 20260613 assets that primary migrations must keep.

The declaration lives in recipes/native/retention/previous-build-retention.json.
Publishers that replay a primary (20260821) revision import this module and skip
the declared routes, NPC rows and texture overrides; retain-previous-build-assets.py
re-derives and verifies the retained payloads from the clean previous build.
"""

from __future__ import annotations

import json
from pathlib import Path

DECLARATION = (
    Path(__file__).resolve().parents[2]
    / "recipes/native/retention/previous-build-retention.json"
)


def load(path: Path | None = None) -> dict:
    return json.loads(Path(path or DECLARATION).read_text(encoding="utf-8"))


def retained_ui_routes(declaration: dict | None = None) -> set[str]:
    """Native UI routes in both the current ui/en and the pre-20260911 ui/ layout."""
    routes = set()
    for entry in (declaration or load())["ui"]:
        route = entry["route"]
        routes.add(route)
        if route.startswith("ui/en/"):
            routes.add("ui/" + route[len("ui/en/"):])
    return routes


def retained_model_paths(declaration: dict | None = None) -> set[str]:
    return {entry["path"] for entry in (declaration or load())["models"]}


def retained_npc_rows(declaration: dict | None = None) -> set[int]:
    return set((declaration or load())["npcMeshRows"]["rows"])


def retained_npc_textures(declaration: dict | None = None) -> set[str]:
    return {entry["trueName"] for entry in (declaration or load())["npcTextureOverrides"]}
