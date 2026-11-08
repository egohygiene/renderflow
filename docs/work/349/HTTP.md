# Local briefing service profile

The #349/#420 source checkpoint adds operator-selected `profile: briefing` to
the existing HTTP/SDK service. Register a bounded package containing exactly the
flat v2 config, its briefing model and every referenced local source plus optional
supplied narration. Model references resolve relative to the captured model,
including nested packages. Missing/extra inputs and traversal/URI references
refuse before the SDK planner can read them. File/count/byte and job limits stay
with the existing package/service owners; no new job engine is introduced.

The existing briefing source kind and six exact primary targets are supported:
`briefing_script`, `briefing_text`, `briefing_html`, `briefing_chapters`,
`briefing_feed`, `briefing_audio`. Normal inspect/plan/execute, status/events,
cancellation, native evidence and verified binary download routes are shared.
Explicit completed-artifact retention can recover those downloads after restart
through the existing core verifier; interrupted work is never replayed.

This profile refuses source uploads, local process permission, arbitrary
transforms/providers and implicit model/voice/network use. Caller-supplied source,
script and narration reviews are preserved exactly; `allow_draft` permits a local
candidate only. Stale pins, absent optional feed/audio or missing required review
remain core unavailable/refusal states. Operator source edits after registration
cannot alter the captured job package or silently inherit approval.

Primary target selection is not redaction: core retains its complete private
briefing bundle, including local model/source/auxiliary snapshots. Only explicit
native published targets become download rows. The bearer still grants one
trusted local operator scope; no tenant boundary or publication approval follows.

Four regressions are authored, uncompiled/unrun. No tests/builds, validators,
dependency resolution, product/native/SDK/listener execution or CI ran. Runtime,
socket, fault, native/semantic/accessibility and release gates remain #420/#349
and existing #458/#419 qualification work.
