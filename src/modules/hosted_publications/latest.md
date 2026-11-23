# Latest version and equivocation

A reader chooses the best-known version of a publisher's page or feed-post stream from candidates gathered from several hosts. `selectLatest(publisher, kind, candidates, bounds)` ignores (and counts) candidates that are another
publisher's or kind or that fail verification, treats identical signed bytes from several hosts as one publication, and groups the rest by sequence. A sequence with two different verified publications is **equivocation**
(including the same publication ID signed with different content). The choice is the greatest sequence without a conflict, so the reader never picks between conflicting versions and keeps the latest unconflicted one below them;
`equivocating` lists every conflicted sequence and `isEquivocating` reports the publisher. `isProvenLatest` is always false: absence proves nothing, a host may be offline or serve an older version, so the result is the best known,
never a proven latest. Resources have no sequence and are `Invalid` here.
