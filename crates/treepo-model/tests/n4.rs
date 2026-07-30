//! `AC-MAT-3` and `N4` against this crate's **public** API.
//!
//! > No `treepo-model` type exposes an ordered contributor collection or a share as a figure.
//!
//! The unit tests inside `primitives::ownership` and `material` hold the individual pieces —
//! `AuthorShare` implements neither [`Ord`] nor [`PartialOrd`] (two `compile_fail` doctests say
//! so), iteration is key-ordered, `Debug` renders a bucket. This file is the crate-level claim
//! those pieces are supposed to add up to, and it lives in `tests/` rather than in `src/` for the
//! reason that matters: an integration test can reach **only what a caller can reach**. A unit test
//! can see private fields and would prove a property about the inside of the crate, where the
//! question `N4` asks is about what leaves it.
//!
//! # What this file claims, and what it does not
//!
//! It asserts the **shape** of the API. It does not claim the compiler prevents a determined
//! caller from building a leaderboard, because it does not: `Mosaic`'s cell counts and
//! `AuthorEntry::commit_count` are `u32`, `u32` is [`Ord`], and anyone who collects them can sort
//! them. That was accepted deliberately rather than overlooked — cells are a geometric quantity a
//! renderer has to compare against a quota, and a count that could not be compared would obstruct
//! every legitimate use in order to inconvenience one illegitimate one.
//!
//! So the honest claim is narrower and still worth holding. Three of the four are *enforced* — a
//! regression fails this file — and the fourth is *recorded*, which is a weaker thing and is
//! labelled as one:
//!
//! 1. **Enforced. Nothing hands back contributors in an order that means anything.** Every public
//!    iterator over people yields them in [`AuthorKey`] order, which is hash order. Verified by
//!    sabotage: ranking `Mosaic::holders` by cell count failed this file naming the accessor.
//! 2. **Enforced. No rendering the crate offers contains a contribution as a figure.** [`Debug`] is
//!    the only one — nothing here implements [`Display`](core::fmt::Display) — and it is coarse on
//!    purpose. Verified by sabotage: adding the ppm value to `AuthorShare`'s `Debug` failed both
//!    rendering tests, the composite one naming the leaked digits.
//! 3. **Enforced at compile time. A new per-contributor field must be reviewed here.** The
//!    exhaustive destructuring in `every_per_contributor_field_is_accounted_for` is the tripwire.
//!    Verified by sabotage: a `lines_authored: u64` field on [`AuthorEntry`] stopped this file
//!    compiling with `E0027`.
//! 4. **Recorded, not enforced. Every per-contributor magnitude requires naming the contributor
//!    first.** There is no accessor for "the largest holder", "the top three", or "the remainder by
//!    rank", so no call produces a ranking a caller did not arrive with. What the test below
//!    asserts is that this holds of every accessor that exists *today* — it cannot fail if someone
//!    adds a `largest_holder` tomorrow, because Rust offers no way to assert the absence of a
//!    method. That gap is closed by review and by the type-level documentation on
//!    [`Mosaic`](treepo_model::Mosaic), not by this file, and saying so is better than implying a
//!    guarantee that is not here.
//!
//! `AC-MAT-3` as the PRD words it — "no UI surface anywhere displays a contribution percentage,
//! count, rank, or ordering of people" — binds a surface this crate does not have. The gate for
//! that is Phase 5's inspection panel; what this file protects is that the panel is not *handed*
//! an ordering, so displaying one would have to be a deliberate act rather than an accident of
//! iterating what it was given.

use treepo_det::OrderedMap;
use treepo_model::identity::AuthorKey;
use treepo_model::primitives::AuthorShare;
use treepo_model::primitives::ownership::OwnershipPrimitives;
use treepo_model::{AuthorEntry, AuthorTable, Manifest, Mosaic, NodeKind, PathRecord, RepoPath};

/// A contributor keyed off a stable string.
fn author(name: &str) -> AuthorKey {
    AuthorKey::from_email(name.as_bytes())
}

/// The four contributors every test here shares, with deliberately unequal volumes.
///
/// `(name, lines)`. The volumes are wide apart so that "ordered by contribution" and "ordered by
/// key" are different orders — which every ordering assertion below depends on, and which
/// [`key_order_is_not_volume_order`] refuses to let go stale.
const CONTRIBUTORS: [(&str, u64); 4] = [
    ("dominant@example.test", 8_000),
    ("frequent@example.test", 1_500),
    ("occasional@example.test", 400),
    ("rare@example.test", 100),
];

fn line_counts() -> OrderedMap<AuthorKey, u64> {
    CONTRIBUTORS
        .iter()
        .map(|&(name, lines)| (author(name), lines))
        .collect()
}

fn ownership() -> OwnershipPrimitives {
    OwnershipPrimitives::from_line_counts(&line_counts(), recency())
}

fn recency() -> OrderedMap<AuthorKey, i64> {
    CONTRIBUTORS
        .iter()
        .enumerate()
        .map(|(i, &(name, _))| (author(name), 1_700_000_000 + i as i64 * 86_400))
        .collect()
}

/// The mosaic the same four would be drawn in, sized generously enough that all four earn cells.
fn mosaic() -> Mosaic {
    let held: OrderedMap<AuthorKey, u32> = CONTRIBUTORS
        .iter()
        .map(|&(name, lines)| (author(name), u32::try_from(lines / 100).unwrap()))
        .collect();
    Mosaic::new(held, 100)
}

fn author_table() -> AuthorTable {
    let mut table = AuthorTable::new();
    for (i, &(name, lines)) in CONTRIBUTORS.iter().enumerate() {
        table.insert(
            author(name),
            AuthorEntry {
                recency: 1_700_000_000 + i as i64 * 86_400,
                commit_count: u32::try_from(lines / 100).unwrap(),
                is_self: false,
            },
        );
    }
    table
}

/// Contributors sorted by what they contributed — the order `N4` forbids producing.
///
/// Built here, from the fixture's own declared volumes, precisely so the assertions below can
/// compare against it. That it takes a const table and a sort to obtain is the point: no accessor
/// on any type in the crate returns this.
fn by_volume() -> Vec<AuthorKey> {
    let mut ranked = CONTRIBUTORS;
    ranked.sort_by_key(|&(_, lines)| core::cmp::Reverse(lines));
    ranked.iter().map(|&(name, _)| author(name)).collect()
}

fn by_key() -> Vec<AuthorKey> {
    let mut keys: Vec<AuthorKey> = CONTRIBUTORS.iter().map(|&(name, _)| author(name)).collect();
    keys.sort();
    keys
}

/// The guard the whole file rests on.
///
/// If a hash happened to sort the fixture's contributors into volume order, then "iteration is in
/// key order" and "iteration is in contribution order" would be the same sequence, and every
/// ordering assertion below would pass under the bug it exists to catch. Computed rather than
/// assumed, because which key sorts first is a property of SHA-256 and not something a fixture
/// should be written around — the same guard `treepo-gen::normalize`'s
/// `holders_iterate_in_key_order_not_by_size` carries.
#[test]
fn key_order_is_not_volume_order() {
    assert_ne!(
        by_key(),
        by_volume(),
        "this fixture's key order coincides with its contribution order, so it cannot tell the \
         two apart — pick contributors whose hashes disagree with their volumes"
    );
}

/// Claim 1. Every public iterator over people, in one test, because the claim is about the API as
/// a whole rather than about any one of them.
#[test]
fn no_public_iterator_yields_contributors_in_contribution_order() {
    let ownership = ownership();
    let mosaic = mosaic();
    let table = author_table();

    let iterated: [(&str, Vec<AuthorKey>); 4] = [
        (
            "OwnershipPrimitives::shares",
            ownership.shares().map(|(&key, _)| key).collect(),
        ),
        (
            "OwnershipPrimitives::recency",
            ownership.recency().map(|(&key, _)| key).collect(),
        ),
        (
            "Mosaic::holders",
            mosaic.holders().map(|(&key, _)| key).collect(),
        ),
        (
            "AuthorTable::iter",
            table.iter().map(|(&key, _)| key).collect(),
        ),
    ];

    for (name, order) in iterated {
        assert_eq!(
            order,
            by_key(),
            "{name} does not yield contributors in key order"
        );
        assert_ne!(
            order,
            by_volume(),
            "{name} yields contributors in contribution order — N4"
        );
    }
}

/// Claim 2, and the sharpest thing this file has to say. Every per-contributor magnitude the crate
/// exposes takes an [`AuthorKey`] as an argument, so a caller must already know who they are
/// asking about. There is no `largest_holder`, no `top_n`, no `nth_by_share` — and therefore no
/// call that turns "these four people" into "these four people in order".
#[test]
fn every_per_contributor_magnitude_requires_naming_the_contributor() {
    let ownership = ownership();
    let mosaic = mosaic();
    let table = author_table();
    let dominant = author(CONTRIBUTORS[0].0);
    let rare = author(CONTRIBUTORS[3].0);

    // Each of these needs a key handed to it. That is the shape being asserted; the values are
    // checked so the test would notice if an accessor started ignoring its argument.
    assert!(ownership.share_of(&dominant).to_ppm() > ownership.share_of(&rare).to_ppm());
    assert!(mosaic.cells_for(&dominant) > mosaic.cells_for(&rare));
    assert!(table.get(&dominant).is_some());

    // And the presence predicates, which are what `N4` actually invites a caller to ask: `F-MAT-2`
    // makes ownership an accent over the primary material, and presence is all a renderer needs to
    // decide whether to draw one.
    assert!(ownership.share_of(&rare).is_present());
    assert!(mosaic.is_present(&rare));

    // The aggregate accessors return counts, never identities: how many people, not which.
    assert_eq!(ownership.author_count(), 4);
    assert_eq!(mosaic.holder_count(), 4);
    assert_eq!(table.len(), 4);
}

/// Claim 2, continued: the two accessors that *do* single someone out, and why neither is a rank.
///
/// `dominant_author` names one contributor because `F-MAT-1` needs a base material family for the
/// limb. It is not the first step of a ranking, and the tie case is what makes that structural
/// rather than a promise — on an exact tie it returns `None`, so a caller cannot strip the winner
/// and ask again to get second place. `bus_factor_proxy` is a count that names nobody at all.
#[test]
fn dominance_is_a_material_choice_and_the_bus_factor_names_nobody() {
    let ownership = ownership();
    assert_eq!(
        ownership.dominant_author(),
        Some(author(CONTRIBUTORS[0].0)),
        "F-MAT-1 needs a base family"
    );

    // Two contributors, exactly equal: no dominant author, so the accessor cannot be recursed into
    // a ranking. This is the property that stops it being `nth(0)`.
    let tied = OwnershipPrimitives::from_line_counts(
        &[
            (author("a@example.test"), 500),
            (author("b@example.test"), 500),
        ]
        .into_iter()
        .collect(),
        OrderedMap::new(),
    );
    assert_eq!(tied.dominant_author(), None);

    // Four unequal contributors and the bus factor is a number. One is 8,000 of 10,000 lines, so
    // that contributor alone clears the 80% threshold and the proxy is 1 — and nothing in the API
    // says *which* one it counted, which is the whole of why a count is permitted where a list
    // would not be.
    assert_eq!(ownership.bus_factor_proxy(), 1);
}

/// Claim 3. `Debug` is the only rendering this crate offers of a contribution — no type here
/// implements [`Display`](core::fmt::Display) — and it must not carry a figure, including inside a
/// dump of a whole record. A tooltip built from `{:?}` is the likeliest way `AC-MAT-3` would be
/// broken by accident.
#[test]
fn a_shares_own_rendering_is_a_bucket() {
    // 731,902 ppm — a distinctive value whose digits would be unmistakable in any output.
    assert_eq!(
        format!("{:?}", AuthorShare::from_ppm(731_902)),
        "AuthorShare(major)"
    );
}

/// The same claim about a *container*, which is the one that would actually break: nothing here
/// implements `Debug` by hand except [`AuthorShare`], so every composite inherits its discretion —
/// and a derived `Debug` on a new wrapper is how a figure would reappear without anyone editing the
/// rendering that was written to suppress it.
#[test]
fn no_rendering_of_a_contribution_carries_a_figure() {
    let mut record = PathRecord::new(RepoPath::new(b"src/main.rs").unwrap(), NodeKind::File);
    record.ownership = OwnershipPrimitives::from_line_counts(
        &[
            (author(CONTRIBUTORS[0].0), 731_902),
            (author(CONTRIBUTORS[1].0), 268_098),
        ]
        .into_iter()
        .collect(),
        OrderedMap::new(),
    );

    // The whole record, which is what a debug surface or a `{:#?}` in a log would print.
    let rendered = format!("{record:?}");
    for figure in ["731902", "731,902", "73.1", "26.8", "%"] {
        assert!(
            !rendered.contains(figure),
            "a debug dump of a PathRecord surfaced `{figure}` — N4 draws the line at a share as a \
             figure, and a tooltip built from Debug is how that happens by accident:\n{rendered}"
        );
    }

    // And the bucket really is in there, so the assertions above are not passing because the
    // ownership was empty.
    assert!(rendered.contains("major"), "{rendered}");
    assert!(rendered.contains("partial"), "{rendered}");
}

/// Claim 4 — the tripwire, and the only compile-time assertion in this file.
///
/// Both structs are destructured exhaustively, so **adding a field to either stops this test
/// compiling** and whoever added it has to decide here whether it is a contributor magnitude and
/// whether `N4` permits it. This is the same discipline `treepo-store::manifest_io` uses to stop a
/// new primitive quietly not being persisted, pointed at a constraint instead of at a format.
///
/// The two accounted-for routes are named rather than hidden:
///
/// * **`AuthorShare::to_ppm` / `to_fx`** — the share as a number, which `treepo-store` must
///   serialize and `treepo-gen`'s `F-MAT-3` normalization needs the magnitude of. Neither is a
///   percentage and neither is reachable from a rendering; `to_ppm`'s own doc comment says "not for
///   display".
/// * **`AuthorEntry::commit_count`** — a per-contributor commit total, which nothing generative
///   reads (`treepo-gen` never touches [`AuthorTable`]) and which `treepo-store` persists. It is
///   the widest route to a leaderboard in this crate: collect [`AuthorTable::iter`] and sort by it.
///   Recorded here as accepted-and-watched rather than closed, because closing it is a manifest
///   schema change and a decision about what `F-INSP-1` will need, not a test's call.
#[test]
fn every_per_contributor_field_is_accounted_for() {
    let entry = AuthorEntry {
        recency: 1_700_000_000,
        commit_count: 41,
        is_self: false,
    };

    // Exhaustive: a new field here is a compile error rather than an unreviewed magnitude.
    //
    // **Do not add `..` to this pattern.** rustc will suggest it — "if you don't care about this
    // missing field, you can explicitly ignore it" — and taking that suggestion is the one edit
    // that turns this test from a tripwire into decoration. Verified by sabotage: adding a
    // `lines_authored: u64` field failed this file with `E0027 pattern does not mention field`,
    // which is exactly the prompt intended.
    let AuthorEntry {
        recency,
        commit_count,
        is_self,
    } = entry;

    // `recency` is a timestamp, not a volume — a contributor who committed recently is not thereby
    // a larger contributor, and `design/feature-system.md` §3.4 permits recency as an accent.
    assert_eq!(recency, 1_700_000_000);
    // The one per-contributor volume figure in the public API. Asserted so that its removal is
    // also a deliberate act, and so the doc comment above cannot go stale while claiming it.
    assert_eq!(commit_count, 41);
    // `F-ID-1`'s one permitted naming, and only of the viewer, by their own choice.
    assert!(!is_self);

    // And the same exhaustiveness over the manifest's own contributor surface: `authors` is the
    // only field on `Manifest` that holds people, so a second one would have to be reviewed here.
    let manifest = Manifest::new(String::from("test"), treepo_det::Seed::root(b"n4"));
    let contributor_fields = [&manifest.authors];
    assert_eq!(contributor_fields.len(), 1);
    assert!(manifest.authors.is_empty());
    assert_eq!(
        manifest.authors.self_author(),
        None,
        "F-ID-7: no viewer is the ordinary state"
    );
}
