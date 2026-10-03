import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("omniverse", Path(__file__).with_name("publish-omniverse-npcs.py"))
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)


class AdditiveNpcTests(unittest.TestCase):
    def fixture(self):
        row = {"m_iNpcName": 0, "m_iMesh": 0, "m_iServiceNumber": 12,
               "m_iNpcType": 21, "m_iBarkerNumber": 8, "m_iIcon1": 0,
               "m_fScale": 1.0, "m_iHeight": 200, "m_fAnimationSpeed": 1.0,
               "m_fWalkAnimationSpeed": 1.0, "m_fRunAnimationSpeed": 1.0}
        native = {"m_pNpcData": [dict(row, m_iNpcNumber=i) for i in range(3464)],
                  "m_pNpcStringData": [{"m_strName": "Ben", "m_strComment": "Hello"}],
                  "m_pNpcMeshData": [{"m_pstrMMeshModelString": "npc_ben"}],
                  "m_pNpcIconData": [{"m_iIconNumber": 68, "m_iIconType": 4}]}
        primary = copy.deepcopy(native)
        for _, original, *_ in publisher.VARIANTS:
            primary["m_pNpcData"][original]["m_fScale"] = 1.15
        locales = {locale: {"entries": {"content.tabledata.npc.npc_string.0.str_comment": greeting}}
                   for locale, greeting in (("en", "Hello"), ("ru", "Привет"))}
        return native, primary, locales

    def test_original_guide_and_service_ownership_stays_on_original_types(self):
        native, primary, locales = self.fixture()
        original = copy.deepcopy(native)
        publisher.add_rows(native, primary, locales)
        for key, rows in original.items():
            self.assertEqual(native[key][:len(rows)], rows)
        for number, _, route, *_ in publisher.VARIANTS:
            variant = native["m_pNpcData"][number]
            self.assertEqual((variant["m_iNpcType"], variant["m_iServiceNumber"]), (3, 0))
            self.assertEqual(variant["m_fScale"], 1.15)
            self.assertEqual(native["m_pNpcMeshData"][variant["m_iMesh"]]["m_pstrMMeshModelString"], route)
            self.assertNotEqual(locales["en"]["entries"][f"content.npc.{number}.name"],
                                locales["ru"]["entries"][f"content.npc.{number}.name"])
            string_id = variant["m_iNpcName"]
            self.assertEqual(locales["ru"]["entries"][f"content.tabledata.npc.npc_string.{string_id}.str_comment"], "Привет")

    def test_stable_ids_cannot_overwrite_preexisting_extension(self):
        native, primary, locales = self.fixture()
        native["m_pNpcData"].append({"m_iNpcNumber": 3464})
        original = copy.deepcopy(native)
        with self.assertRaisesRegex(AssertionError, "append point"):
            publisher.add_rows(native, primary, locales)
        self.assertEqual(native, original)


if __name__ == "__main__":
    unittest.main()
