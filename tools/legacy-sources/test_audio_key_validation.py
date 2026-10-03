import unittest

from audio_key_validation import renamed_aliases, validate_key_plan


class AudioKeyValidationTests(unittest.TestCase):
    def test_rename_back_removes_self_alias_but_retains_previous_key(self):
        entry = {"logicalKey": "old", "trueName": "line", "aliases": ["new"]}
        self.assertEqual(renamed_aliases(entry, "new"), ["old"])
        validate_key_plan([entry], {id(entry): "new"})
        self.assertEqual(entry["aliases"], ["new"])

    def test_cross_entry_swap_is_rejected_before_mutation(self):
        a = {"logicalKey": "base", "trueName": "original"}
        b = {"logicalKey": "base_take_b", "trueName": "other"}
        with self.assertRaises(SystemExit):
            validate_key_plan([a, b], {id(a): "base_take_b", id(b): "base"})
        self.assertEqual(a["logicalKey"], "base")
        self.assertEqual(b["logicalKey"], "base_take_b")

    def test_unchanged_self_alias_is_rejected(self):
        entry = {"logicalKey": "line", "trueName": "line", "aliases": ["LINE"]}
        with self.assertRaises(SystemExit):
            validate_key_plan([entry], {})

    def test_alias_collision_is_case_insensitive(self):
        a = {"logicalKey": "a", "trueName": "a", "aliases": ["OLD"]}
        b = {"logicalKey": "b", "trueName": "b", "aliases": ["old"]}
        with self.assertRaises(SystemExit):
            validate_key_plan([a, b], {})

    def test_unique_aliases_remain_valid(self):
        entries = [
            {"logicalKey": "a", "trueName": "a", "aliases": ["old_a"]},
            {"logicalKey": "b", "trueName": "b", "aliases": ["old_b"]},
        ]
        validate_key_plan(entries, {})


if __name__ == "__main__":
    unittest.main()
