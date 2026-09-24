import unittest

from pkg.util import double


class HiddenTest(unittest.TestCase):
    def test_double(self):
        self.assertEqual(double(21), 42)
