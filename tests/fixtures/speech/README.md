# Original synthetic speech-provider fixture

SPDX-License-Identifier: MIT

`whisper-segments.json` is manually authored test input matching the inspected
whisper.cpp JSON shape. It contains no user recording, scraped transcript,
downloaded speech sample, or model output. It is not a successful ASR result.

The fixture exercises segment offsets, Unicode text, explicit language,
whitespace normalization, and intentionally unused token-probability data.
Tests mutate it to author refusal cases. The repository's MIT license applies.
All #428 tests remain unrun under the current implementation-first budget.
