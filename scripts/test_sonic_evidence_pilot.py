"""Observer contract: a crash or unrelated error is not native refusal proof."""
import subprocess
import unittest

from run_sonic_evidence_pilot import require_native_outcome


class NativeRefusalObserverTests(unittest.TestCase):
    def observe(self, status, stderr, stdout=b""):
        result = subprocess.CompletedProcess([], status, stdout, stderr)
        require_native_outcome(result, False, "sonic.output.exists")

    def test_accepts_only_the_owning_native_refusal(self):
        self.observe(1, b"Error: sonic.output.exists\n")

    def test_same_code_looking_panic_is_not_proof(self):
        for status in (1, 101):
            with self.subTest(status=status), self.assertRaises(ValueError):
                self.observe(status, b"thread 'main' panicked: sonic.output.exists\n")

    def test_unrelated_error_containing_code_is_not_proof(self):
        for diagnostic in (b"Error: unrelated failure (sonic.output.exists)\n",
                           b"Error: sonic.output.exists\nthread 'main' panicked\n"):
            with self.subTest(diagnostic=diagnostic), self.assertRaises(ValueError):
                self.observe(1, diagnostic)

    def test_exact_line_with_wrong_exit_or_extra_stdout_is_not_proof(self):
        for status, stdout in ((0, b""), (2, b""), (101, b""), (1, b"panic text\n")):
            with self.subTest(status=status, stdout=stdout), self.assertRaises(ValueError):
                self.observe(status, b"Error: sonic.output.exists\n", stdout)


if __name__ == "__main__":
    unittest.main()
