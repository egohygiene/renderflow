"""Synthetic source cases; not executed during the implementation sprint."""
import unittest
import native_cohort


class NativeCohortTests(unittest.TestCase):
    def test_mach_o_target_identity_cannot_be_swapped(self):
        binary = bytearray(32)
        binary[:4] = b"\xcf\xfa\xed\xfe"
        binary[4:8] = (0x100000c).to_bytes(4, "little")
        native_cohort.binary_identity(bytes(binary), "aarch64-apple-darwin")
        with self.assertRaises(ValueError):
            native_cohort.binary_identity(bytes(binary), "x86_64-apple-darwin")

    def test_elf_magic_does_not_establish_x86_64_identity(self):
        binary = bytearray(64)
        binary[:6] = b"\x7fELF\x02\x01"
        binary[18:20] = (183).to_bytes(2, "little")
        with self.assertRaises(ValueError):
            native_cohort.binary_identity(bytes(binary), "x86_64-unknown-linux-gnu")
