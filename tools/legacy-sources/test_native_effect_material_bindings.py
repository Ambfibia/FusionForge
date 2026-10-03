"""Regression for material animation ownership on native glTF renderers."""
import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    'effect_publisher', Path(__file__).with_name('publish-nano-status-effects.py'))
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)


class MaterialCurveBindings(unittest.TestCase):
    def test_shared_geometry_keeps_separate_animation_owners(self):
        mesh = {'name': 'Plane', 'primitives': [{'indices': 3,
                'attributes': {'POSITION': 1, 'TEXCOORD_0': 2}, 'material': 4}]}
        doc = {'nodes': [{'name': 'Plane', 'mesh': 0}, {'name': 'Plane', 'mesh': 0}],
               'meshes': [copy.deepcopy(mesh)]}
        first = publisher.material_curve_surface_name(doc, 0)
        self.assertEqual(doc['meshes'][0], mesh)
        second = publisher.material_curve_surface_name(doc, 1)
        self.assertNotEqual(first, second)
        for node, name in zip(doc['nodes'], [first, second]):
            self.assertEqual(node['name'], name)
            self.assertEqual(doc['meshes'][node['mesh']]['name'], name)
            self.assertEqual(doc['meshes'][node['mesh']]['primitives'], mesh['primitives'])
        before = copy.deepcopy(doc)
        self.assertEqual(publisher.material_curve_surface_name(doc, 0), first)
        self.assertEqual(doc, before, 'later curves must reuse the same renderer identity')

    def test_non_renderer_source_curve_does_not_create_geometry(self):
        doc = {'nodes': [{'name': 'Cylinder'}], 'meshes': []}
        self.assertEqual(publisher.material_curve_surface_name(doc, 0), 'Cylinder')
        self.assertEqual(doc['meshes'], [])


if __name__ == '__main__':
    unittest.main()
