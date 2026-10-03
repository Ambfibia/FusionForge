"""Regression coverage for the native character resolver used by the census."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("asset_census", Path(__file__).with_name("asset-census.py"))
census = importlib.util.module_from_spec(spec)
spec.loader.exec_module(census)


def model(category="npc", package="npc_ben", logical="npc_ben", aliases=None):
    return {"id": f"{category}/{package}", "category": category, "logicalName": logical,
            "legacyAliases": aliases or [], "glb": f"characters/{category}/{package}/{logical}.glb"}


class CharacterResolutionTests(unittest.TestCase):
    def test_case_and_variant_ownership_are_not_folded(self):
        routes = census.character_routes({"models": [
            model(), model(package="ben_omniverse", aliases=["ben_omniverse"]),
        ]})
        self.assertEqual(routes["npc_ben"]["id"], "npc/npc_ben")
        self.assertEqual(routes["ben_omniverse"]["id"], "npc/ben_omniverse")
        self.assertNotIn("NPC_BEN", routes)
        self.assertNotIn("npc_ben", census.character_routes({"models": [
            model(package="ben_omniverse", aliases=["ben_omniverse"])]}))

    def test_unique_npc_category_wins_but_two_non_npc_categories_are_ambiguous(self):
        entries = [model(category="shared"), model(category="mob")]
        self.assertIsNone(census.character_routes({"models": entries})["npc_ben"])
        entries.append(model())
        self.assertEqual(census.character_routes({"models": entries})["npc_ben"]["category"], "npc")

    def test_ignored_categories_cannot_satisfy_an_npc_reference(self):
        for category in ("nano", "player", "tutorial"):
            self.assertEqual(census.character_routes({"models": [model(category=category)]}), {})

    def test_same_category_conflict_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "Contradictory"):
            census.character_routes({"models": [model(), model(package="other")]})

    def test_census_reports_case_mismatch_and_ignores_invisible_location_probes(self):
        table = {"m_pNpcTable": {
            "m_pNpcData": [{}, {"m_iMesh": 1, "m_iNpcName": 1},
                           {"m_iMesh": 2, "m_iNpcType": 100, "m_iNpcName": 1}],
            "m_pNpcMeshData": [{}, {"m_pstrMMeshModelString": "NPC_BEN"},
                               {"m_pstrMMeshModelString": "HiddenProbe"}],
            "m_pNpcStringData": [{}, {"m_strName": "Ben"}],
        }}
        report = census.census_characters(table, {"npc_ben": [("source", "mob/npc_ben.kfm")]}, {},
                                         {"models": [model()]})
        self.assertEqual(len(report["unresolved"]), 1)
        self.assertEqual(report["unresolved"][0]["name"], "NPC_BEN")
        self.assertEqual(report["unresolved"][0]["origin"], "primary")
        self.assertEqual(report["unresolved"][0]["resolution"], "missing-exact-route")


if __name__ == "__main__":
    unittest.main()
