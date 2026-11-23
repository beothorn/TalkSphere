//! Choosing the latest version of a publisher's stream across hosts, and detecting equivocation.
//!
//! Callers: the page reader and the feed reader. A publisher's page and feed-post streams are ordered by `sequence`. A reader asks several hosts,
//! verifies every candidate, and takes the greatest verified sequence; that is the best-known version, never a proven-latest one, because a
//! host may be offline, may have expired a newer placement or may serve an older version, and absence proves nothing. Two verified
//! publications with the same publisher, kind and sequence that are not the identical signed bytes are equivocation: the reader must not
//! choose between them, keeps the latest sequence that has no conflict, and reports the publisher.
use super::{
    error::Error,
    metadata::{Kind, Publication},
    verify::{Bounds, verify},
};
use crate::modules::identity::api::IdentityId;
use std::collections::{BTreeMap, BTreeSet};

/// What a reader concluded from the candidates it collected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// The publication with the greatest sequence that has no conflicting publication, if any candidate qualified.
    pub best: Option<Publication>,
    /// Every sequence at which two different verified publications exist; the publisher is equivocating when this is not empty.
    pub equivocating: Vec<u64>,
    /// How many candidates passed verification (identical copies from several hosts count once).
    pub verified: usize,
    /// How many candidates were ignored because they did not verify or were another publisher's or kind.
    pub ignored: usize,
}

impl Selection {
    /// Whether the publisher signed two different publications at one sequence.
    pub fn is_equivocating(&self) -> bool {
        !self.equivocating.is_empty()
    }

    /// Always false: a reader that sees the highest sequence among the hosts it asked cannot know no newer version exists elsewhere.
    pub fn is_proven_latest(&self) -> bool {
        false
    }
}

/// Select the best-known version of `publisher`'s stream of `kind` from `candidates` gathered from any hosts.
///
/// Candidates that are not of this publisher and kind, or that fail [`verify`], are ignored and counted. Only pages and feed posts form
/// ordered streams.
/// # Errors
/// `Invalid` when `kind` is a resource, which has no sequence.
pub fn select_latest(
    publisher: IdentityId,
    kind: Kind,
    candidates: &[Publication],
    bounds: Bounds,
) -> Result<Selection, Error> {
    tracing::trace!("Selecting the best-known version across hosts");
    if !kind.is_listed() {
        return Err(Error::Invalid);
    }
    let mut by_sequence: BTreeMap<u64, Vec<&Publication>> = BTreeMap::new();
    let mut ignored = 0;
    for candidate in candidates {
        let usable = candidate.metadata.kind == kind
            && candidate.metadata.publisher_id == publisher
            && verify(candidate, publisher, bounds).is_ok();
        if !usable {
            ignored += 1;
            continue;
        }
        let group = by_sequence.entry(candidate.metadata.sequence).or_default();
        // The same signed bytes from several hosts are one publication.
        if !group.contains(&candidate) {
            group.push(candidate);
        }
    }
    let equivocating: BTreeSet<u64> = by_sequence
        .iter()
        .filter(|(_, group)| group.len() > 1)
        .map(|(sequence, _)| *sequence)
        .collect();
    let best = by_sequence
        .iter()
        .rev()
        .find(|(sequence, _)| !equivocating.contains(sequence))
        .map(|(_, group)| group[0].clone());
    Ok(Selection {
        best,
        equivocating: equivocating.into_iter().collect(),
        verified: by_sequence.values().map(Vec::len).sum(),
        ignored,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::hosted_publications::{
        ids::PublicationId,
        metadata::Visibility,
        signing::{Draft, sign_publication, tests::VectorPublisher},
    };
    use crate::modules::messaging::ports::Signer;

    fn page(signer: &VectorPublisher, id: u8, sequence: u64, kind: Kind) -> Publication {
        sign_publication(
            signer,
            Draft {
                publication_id: PublicationId([id; 32]),
                kind,
                sequence,
                published_at: 1000,
                visibility: Visibility::Public,
                audience: vec![],
            },
            vec![id],
        )
        .unwrap()
    }

    // HP-10: with sequence 1 at one host and sequence 2 at another, the reader takes sequence 2; identical copies from several hosts are one;
    // the result is never claimed to be proven latest.
    #[test]
    fn the_greatest_verified_sequence_wins() {
        let publisher = VectorPublisher::new();
        let me = publisher.identity();
        let (one, two) = (
            page(&publisher, 1, 1, Kind::Page),
            page(&publisher, 2, 2, Kind::Page),
        );
        let selection = select_latest(
            me,
            Kind::Page,
            &[one.clone(), two.clone(), two.clone()],
            Bounds::default(),
        )
        .unwrap();
        assert_eq!(selection.best, Some(two));
        assert_eq!(
            (
                selection.verified,
                selection.ignored,
                selection.is_equivocating()
            ),
            (2, 0, false)
        );
        assert!(!selection.is_proven_latest());
        // A host that omits sequence 2 leaves sequence 1 as the best known, still not proven latest.
        let older = select_latest(
            me,
            Kind::Page,
            std::slice::from_ref(&one),
            Bounds::default(),
        )
        .unwrap();
        assert_eq!(older.best, Some(one));
        assert!(!older.is_proven_latest());
        let none = select_latest(me, Kind::Page, &[], Bounds::default()).unwrap();
        assert_eq!((none.best, none.verified), (None, 0));
    }

    // HP-10: two different verified publications at one sequence are equivocation; the reader keeps the latest unconflicted sequence below
    // the conflict and reports it, while a conflict below the best version is reported but does not change the choice.
    #[test]
    fn equivocation_is_reported_and_never_chosen() {
        let publisher = VectorPublisher::new();
        let me = publisher.identity();
        let base = page(&publisher, 1, 1, Kind::Page);
        let (left, right) = (
            page(&publisher, 2, 2, Kind::Page),
            page(&publisher, 3, 2, Kind::Page),
        );
        let selection = select_latest(
            me,
            Kind::Page,
            &[base.clone(), left.clone(), right.clone()],
            Bounds::default(),
        )
        .unwrap();
        assert_eq!(selection.best, Some(base.clone()));
        assert_eq!(selection.equivocating, vec![2]);
        assert!(selection.is_equivocating());
        let above = page(&publisher, 4, 3, Kind::Page);
        let selection = select_latest(
            me,
            Kind::Page,
            &[left, right, above.clone(), base],
            Bounds::default(),
        )
        .unwrap();
        assert_eq!(selection.best, Some(above));
        assert_eq!(selection.equivocating, vec![2]);
        // Only a conflict and nothing else leaves nothing to show.
        let only = select_latest(
            me,
            Kind::Page,
            &[
                page(&publisher, 5, 7, Kind::Page),
                page(&publisher, 6, 7, Kind::Page),
            ],
            Bounds::default(),
        )
        .unwrap();
        assert_eq!((only.best, only.equivocating), (None, vec![7]));
        // The same publication ID signed with different content is also a conflict.
        let a = page(&publisher, 8, 9, Kind::Page);
        let resigned = sign_publication(
            &publisher,
            Draft {
                publication_id: PublicationId([8; 32]),
                kind: Kind::Page,
                sequence: 9,
                published_at: 1000,
                visibility: Visibility::Public,
                audience: vec![],
            },
            vec![9],
        )
        .unwrap();
        let selection = select_latest(me, Kind::Page, &[a, resigned], Bounds::default()).unwrap();
        assert_eq!(selection.equivocating, vec![9]);
    }

    // Candidates that are forged, another publisher's or another kind are ignored and counted; resources have no stream.
    #[test]
    fn unusable_candidates_are_ignored() {
        let publisher = VectorPublisher::new();
        let me = publisher.identity();
        let good = page(&publisher, 1, 1, Kind::Page);
        let mut forged = page(&publisher, 2, 5, Kind::Page);
        forged.content[0] ^= 1;
        let other_kind = page(&publisher, 3, 6, Kind::FeedPost);
        let stranger = page(&VectorPublisher::from_seed([7; 32]), 4, 8, Kind::Page);
        let selection = select_latest(
            me,
            Kind::Page,
            &[good.clone(), forged, other_kind, stranger],
            Bounds::default(),
        )
        .unwrap();
        assert_eq!(
            (selection.best, selection.verified, selection.ignored),
            (Some(good), 1, 3)
        );
        assert_eq!(
            select_latest(me, Kind::Resource, &[], Bounds::default()).err(),
            Some(Error::Invalid)
        );
    }
}
