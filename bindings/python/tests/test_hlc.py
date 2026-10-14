import unittest

import bothy
from bothy import Hlc

T = 1_700_000_000_000


class HlcTest(unittest.TestCase):
    def test_matches_rust_vector(self):
        # Same vector as crates/bothy/src/hlc.rs::hex_round_trip_and_ordering.
        self.assertEqual(Hlc(T, 1).to_hex(), "018bcfe568000001")
        self.assertEqual(Hlc.from_hex("018bcfe568000001"), Hlc(T, 1))

    def test_u64_round_trip(self):
        h = Hlc(T, 7)
        self.assertEqual(Hlc.from_u64(h.to_u64()), h)

    def test_ordering_matches_hex_ordering(self):
        a, b = Hlc(T, 1), Hlc(T + 1, 0)
        self.assertLess(a, b)
        self.assertLess(a.to_hex(), b.to_hex())

    def test_rejects_malformed_hex(self):
        for s in ["", "018bcfe56800000", "018bcfe5680000011", "018BCFE568000001", "018bcfe56800000g"]:
            with self.assertRaises(ValueError, msg=s):
                Hlc.from_hex(s)

    def test_rejects_out_of_range_parts(self):
        with self.assertRaises(ValueError):
            Hlc(1 << 48, 0).to_u64()
        with self.assertRaises(ValueError):
            Hlc(T, 1 << 16).to_u64()

    def test_metadata(self):
        self.assertEqual(bothy.__version__, "0.0.1")
        self.assertEqual(bothy.FORMAT_VERSION, 1)


if __name__ == "__main__":
    unittest.main()
