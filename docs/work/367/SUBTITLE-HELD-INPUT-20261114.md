# Held standalone subtitle input

This checkpoint follows draft Renderflow #517 under #367. The standalone
`subtitles convert` command previously checked path metadata and then called
blocking `File::open` without binding the opened identity to that selection.
A path exchanged for a FIFO between these calls could block before the byte
limit; an exchanged link could be followed. This source review does not claim
that a real user source was attacked or that a prior fixture reproduced it.

The owning command now captures the selected regular file with Unix
`NOFOLLOW | NONBLOCK | CLOEXEC`, verifies held descriptor identity before
reading, checks the 16 MiB bound before allocation, and reobserves descriptor
and current path metadata after capture. Size, modification time and Unix
device/inode/link-count/change-time remain bound to the selected observation.
The parser and output no-clobber owner are unchanged. Other platforms retain
regular-file/content metadata checks; native Windows identity/race behavior is
not qualified by the Linux-specific cases.

Meaningful authored cases exercise ordinary Unicode input/early oversized
refusal, a deterministic exchanged symlink, a different inode with identical
bytes/modification time, and a FIFO exchange with no writer. The FIFO trial
unblocks and joins a regressed worker before failing, so its negative path
cannot strand the test process. The private pre-open callback is not exposed
through configuration; normal conversion always supplies a no-op closure.

The combined stride will execute the selected source cases and a real manual
Unicode SRT → canonical JSON → WebVTT/plain-text CLI trial after the audio
configuration source checkpoint. Actual results and exact tested revisions
belong in that retained trial receipt; this source checkpoint alone grants no
format matrix, source authentication, human timing review or release authority.

No provider/model download, media acquisition, original mutation, signing,
release/publication or broad native-matrix claim is introduced. #367 and the
final Flow #88 acceptance owner retain their wider criteria.
