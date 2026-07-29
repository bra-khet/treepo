//! `F-MAT-1` — what a limb is made of, and the table that decides it.
//!
//! > Primary material family is driven by language, binary-vs-text, and asset class. Binary
//! > and asset-heavy regions render as resource-like material rather than living wood.
//!
//! [`MaterialFamily`] and its mapping from a measured
//! [`ContentCategory`](treepo_model::primitives::size::ContentCategory) live in
//! `treepo-model`, because they are a handoff — the renderer matches on them. What lives here
//! is the part that reads a *path* rather than a *file*: a directory holds a mixture of
//! categories, and turning a mixture into something a limb can be made of is a decision with
//! more than one defensible answer.
//!
//! # The rule: dominant sets the family, the runner-up veins it
//!
//! The largest family by bytes is what the limb reads as. Where a second family holds more
//! than [`Table::blend_floor`] of the bytes, the limb is
//! [`Blended`](treepo_model::Composition::Blended) and the renderer interpolates toward it.
//!
//! The alternative considered and rejected was a threshold ladder — each family declaring the
//! share at which it claims a limb outright, so that a directory of 55% code and 45% images
//! would read as pure `Ore` on the strength of being "asset-heavy". It is closer to `F-MAT-1`'s
//! wording and it produces a worse picture: a limb that is nearly half source would be drawn
//! as though it held none, and the honest answer — that it is both — was available.
//!
//! Blending also disposes of the tie problem for free, which a winner-takes-all rule cannot.
//! [`SizePrimitives::dominant_language`](treepo_model::SizePrimitives::dominant_language)
//! returns `None` on an exact tie precisely because breaking one "would flip on the next
//! commit", and a family cannot return `None` — every limb needs a material. Under blending
//! the flip is invisible: at an exact tie the weight is 1.0, so swapping which family is
//! primary and which is secondary produces very nearly the same limb.
//!
//! # Made of, against holds
//!
//! Which reading applies is chosen by the node's [`NodeRole`], not by its content. A limb or a
//! group *is* its mixture; an [`Aggregate`](NodeRole::Aggregate) *holds* one. See
//! [`treepo_model::material`] for why that distinction is worth a type.

use crate::normalize::{Normalize, NormalizeError};
use crate::params::per_mille;
use alloc::string::String;
use core::fmt;
use serde::Deserialize;
use treepo_det::Fx;
use treepo_model::material::{Composition, FamilyMix, Material, MaterialFamily};
use treepo_model::primitives::size::{ContentCategory, SizePrimitives};
use treepo_model::segment::NodeRole;

/// The compiled-in table. Same reasoning as [`params`](crate::params): this crate is `no_std`
/// and could not open a file if it wanted to, and a compiled-in copy guarantees a usable
/// table exists when the user has not supplied one.
const BUILT_IN_RON: &str = include_str!("../../../assets/params/materials.ron");

/// The table format this crate understands.
///
/// Independent of [`params::TABLE_VERSION`](crate::params::TABLE_VERSION) and of
/// [`treepo_model::SCHEMA_VERSION`]: the skeleton table, the material table and the manifest
/// version separately, and an edit to one must not invalidate the others.
pub const TABLE_VERSION: u32 = 1;

/// The material table — `F-MAT-1` and `F-MAT-3` as data.
///
/// Small on purpose. `design/l-system-parameterization.md` §6's one-family-at-a-time tuning
/// loop needs knobs that a person can hold in their head, and the material rule has exactly
/// one real decision in it — how much of a second family it takes before a limb is visibly
/// two materials. Everything else is either a measurement or a meaning, and neither of those
/// is tunable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    /// Format version — must equal [`TABLE_VERSION`].
    pub version: u32,
    /// The share of a node's bytes a second family must hold to vein it, per mille.
    ///
    /// Below this the node is [`Pure`](Composition::Pure). The number is answering "how much
    /// of something else can you see", so it is a perceptual threshold rather than a
    /// statistical one — a directory that is 2% configuration is a directory of source, and
    /// drawing a 2% stripe on it would be noise dressed as data.
    ///
    /// It does not apply to a container's [`Subordinate`](Composition::Subordinate) inventory,
    /// which keeps everything: a shelf listing what is on it does not round its contents away.
    pub blend_floor: i32,
    /// `F-MAT-3` — log, soft clamp, floor, and the contributor quota.
    pub normalize: Normalize,
}

impl Table {
    /// The compiled-in table.
    ///
    /// # Panics
    ///
    /// If the compiled-in table is malformed, which a unit test in this module rules out.
    #[must_use]
    pub fn built_in() -> Self {
        Self::from_ron(BUILT_IN_RON).expect("built-in material table must parse and validate")
    }

    /// Parses and validates a table from RON, for a caller supplying its own.
    ///
    /// # Errors
    ///
    /// [`MaterialError::Parse`] if the text is not a well-formed table, or a validation
    /// variant if it is well-formed but describes materials the design does not permit.
    pub fn from_ron(text: &str) -> Result<Self, MaterialError> {
        let table: Self = ron::from_str(text).map_err(|error| MaterialError::Parse {
            detail: alloc::format!("{error}"),
        })?;
        table.validate()?;
        Ok(table)
    }

    /// Checks a parsed table against the rules `F-MAT-1` and `F-MAT-3` state.
    ///
    /// # Errors
    ///
    /// The first violated rule.
    pub fn validate(&self) -> Result<(), MaterialError> {
        if self.version != TABLE_VERSION {
            return Err(MaterialError::Version {
                found: self.version,
                expected: TABLE_VERSION,
            });
        }

        // Above half, no second family could ever clear it — the runner-up is by definition
        // no larger than the winner — so the table would have silently disabled blending
        // while appearing to configure it. That is the failure `deny_unknown_fields` is
        // elsewhere written to prevent, arriving through a legal value instead of a typo.
        if !(1..=500).contains(&self.blend_floor) {
            return Err(MaterialError::Decision {
                row: "blend_floor",
                detail: "the blend floor is a share in 1..=500 per mille — above half nothing \
                         can clear it, and blending would be off while looking configured",
            });
        }

        self.normalize.validate().map_err(MaterialError::Normalize)
    }

    /// The material for one node.
    ///
    /// `size` is the node's own size primitives; `role` decides whether the mixture inside it
    /// is read as what the node is made of or as what it holds.
    ///
    /// `bytes` is passed separately rather than taken from `size`, because an
    /// [`Aggregate`](NodeRole::Aggregate) knows its own byte total
    /// ([`AggregateNode::bytes`](treepo_model::AggregateNode::bytes)) while standing for
    /// several paths whose primitives have been rolled up — and `F-MAT-3`'s budget must be
    /// the container's, not one member's.
    #[must_use]
    pub fn material_of(&self, size: &SizePrimitives, bytes: u64, role: &NodeRole) -> Material {
        let mix = self.mix_of(size);
        let family = dominant(&mix);

        let composition = match role {
            // F-SKEL-7's container stands for content it does not draw. What is inside is an
            // inventory, and F-INSP-3 requires it to report what it represents — so the whole
            // mix survives rather than its largest two.
            NodeRole::Aggregate(_) => Composition::Subordinate(mix),
            // A limb is one path; a group draws its members as limbs of their own and its stem
            // carries all of them; a root mass stands for the repository. All three are made
            // of what they account for.
            NodeRole::Limb { .. } | NodeRole::Group { .. } | NodeRole::RootMass { .. } => {
                self.blend(&mix, family)
            }
        };

        Material {
            family,
            composition,
            budget: self.normalize.budget(bytes),
        }
    }

    /// Every family's share of a node's bytes.
    #[must_use]
    pub fn mix_of(&self, size: &SizePrimitives) -> FamilyMix {
        let mut shares = [Fx::ZERO; MaterialFamily::ALL.len()];
        for category in ContentCategory::ALL {
            let slot = MaterialFamily::of_category(category).position();
            // `+=` rather than `=`: Asset and Binary are two categories and one family, so
            // the slot has to accumulate or the second would overwrite the first and half of
            // every mixed-binary directory would vanish.
            shares[slot] = shares[slot].add(size.category_ratio(category));
        }
        FamilyMix::new(shares)
    }

    /// The runner-up as a vein, where there is enough of it to see.
    fn blend(&self, mix: &FamilyMix, primary: MaterialFamily) -> Composition {
        let floor = per_mille(self.blend_floor);
        let mut best: Option<(MaterialFamily, Fx)> = None;

        // `ALL` order — the same fixed order everywhere, so a tie between two runners-up
        // breaks the same way on every machine (`N3`).
        for (family, share) in mix.present() {
            if family == primary || share < floor {
                continue;
            }
            match best {
                Some((_, high)) if share <= high => {}
                _ => best = Some((family, share)),
            }
        }

        match best {
            Some((secondary, weight)) => Composition::Blended { secondary, weight },
            None => Composition::Pure,
        }
    }
}

/// The largest family in a mix, in [`MaterialFamily::ALL`] order on a tie.
///
/// A tie has to resolve to something — every limb needs a material, so the `None` that
/// [`OwnershipPrimitives::dominant_author`](treepo_model::OwnershipPrimitives::dominant_author)
/// and [`SizePrimitives::dominant_language`](treepo_model::SizePrimitives::dominant_language)
/// return is not available here. Declaration order is the tie-break, and blending is what
/// makes it harmless: at an exact tie the loser is carried at full weight as the vein, so the
/// limb looks very nearly the same either way.
///
/// An empty mix — a node with no bytes at all — yields [`Stone`](MaterialFamily::Stone).
/// Nothing is known about it, which is exactly what that family means.
fn dominant(mix: &FamilyMix) -> MaterialFamily {
    let mut best: Option<(MaterialFamily, Fx)> = None;
    for (family, share) in mix.present() {
        match best {
            Some((_, high)) if share <= high => {}
            _ => best = Some((family, share)),
        }
    }
    best.map_or(MaterialFamily::Stone, |(family, _)| family)
}

/// Why a material table was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterialError {
    /// The text is not a well-formed table.
    Parse {
        /// The RON parser's message, including its position.
        detail: String,
    },
    /// The table was written for a different format version.
    Version {
        /// The version the file declares.
        found: u32,
        /// The version this build understands.
        expected: u32,
    },
    /// A row contradicts a choice the design has already made.
    Decision {
        /// The row that failed.
        row: &'static str,
        /// The rule it broke.
        detail: &'static str,
    },
    /// The `F-MAT-3` section is invalid.
    Normalize(NormalizeError),
}

impl fmt::Display for MaterialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { detail } => write!(f, "materials.ron is not well-formed: {detail}"),
            Self::Version { found, expected } => write!(
                f,
                "materials.ron declares version {found}; this build understands {expected}"
            ),
            Self::Decision { row, detail } => write!(f, "`{row}`: {detail}"),
            Self::Normalize(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for MaterialError {}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use treepo_model::AggregateNode;
    use treepo_model::path::RepoPath;

    fn path(text: &str) -> RepoPath {
        RepoPath::new(text.as_bytes()).unwrap()
    }

    fn limb() -> NodeRole {
        NodeRole::Limb { path: path("src") }
    }

    fn container() -> NodeRole {
        NodeRole::Aggregate(AggregateNode {
            anchor: path("src"),
            index: 0,
            members: vec![path("src/deep")],
            bytes: 4096,
            file_count: 12,
            dir_count: 2,
        })
    }

    fn sized(entries: &[(ContentCategory, u64)]) -> SizePrimitives {
        SizePrimitives {
            category_bytes: entries.iter().copied().collect(),
            ..SizePrimitives::default()
        }
    }

    #[test]
    fn the_built_in_table_parses_and_validates() {
        let table = Table::built_in();
        assert_eq!(table.version, TABLE_VERSION);
        assert_eq!(table.validate(), Ok(()));
    }

    /// `F-MAT-1`'s named requirement: a repository of source is a tree, and binary content is
    /// not living wood.
    #[test]
    fn language_and_asset_class_drive_the_primary_family() {
        let table = Table::built_in();
        let cases = [
            (ContentCategory::Code, MaterialFamily::Heartwood),
            (ContentCategory::Asset, MaterialFamily::Ore),
            (ContentCategory::Binary, MaterialFamily::Ore),
            (ContentCategory::Generated, MaterialFamily::Machined),
            (ContentCategory::Docs, MaterialFamily::Parchment),
            (ContentCategory::Config, MaterialFamily::Resin),
            (ContentCategory::Unknown, MaterialFamily::Stone),
        ];
        for (category, family) in cases {
            let material = table.material_of(&sized(&[(category, 1000)]), 1000, &limb());
            assert_eq!(material.family, family, "{category:?}");
            assert_eq!(
                material.composition,
                Composition::Pure,
                "{category:?} is one category and therefore one material"
            );
            assert!(!material.budget.is_zero());
        }
    }

    /// The Asset/Binary pair is two categories and one family, so their shares must add
    /// rather than one overwriting the other.
    #[test]
    fn asset_and_binary_accumulate_into_one_family() {
        let table = Table::built_in();
        let size = sized(&[
            (ContentCategory::Asset, 300),
            (ContentCategory::Binary, 300),
            (ContentCategory::Code, 400),
        ]);
        let material = table.material_of(&size, 1000, &limb());
        // 60% together beats 40% of code; separately, 30% each would have lost.
        assert_eq!(material.family, MaterialFamily::Ore);
        assert_eq!(
            material.composition.secondary(),
            Some(MaterialFamily::Heartwood)
        );
    }

    /// The chosen rule, on the case that separates it from a threshold ladder: a limb that is
    /// mostly source and substantially assets is *both*, not one of them.
    #[test]
    fn a_mixed_limb_is_veined_rather_than_reassigned() {
        let table = Table::built_in();
        let size = sized(&[(ContentCategory::Code, 550), (ContentCategory::Asset, 450)]);
        let material = table.material_of(&size, 1000, &limb());

        assert_eq!(material.family, MaterialFamily::Heartwood);
        let Composition::Blended { secondary, weight } = material.composition else {
            panic!(
                "a 45% second family must be visible: {:?}",
                material.composition
            );
        };
        assert_eq!(secondary, MaterialFamily::Ore);
        assert_eq!(weight, Fx::from_ratio(45, 100));
    }

    /// The one knob: a trace of something else is not a stripe.
    #[test]
    fn a_trace_of_a_second_family_leaves_the_limb_pure() {
        let table = Table::built_in();
        let below = Fx::from_ratio(i64::from(table.blend_floor) - 1, 1000);
        let size = sized(&[
            (ContentCategory::Code, 1000),
            (
                ContentCategory::Config,
                (below.mul(Fx::from_int(1000))).round() as u64,
            ),
        ]);
        assert_eq!(
            table.material_of(&size, 2000, &limb()).composition,
            Composition::Pure
        );
    }

    /// A tie must resolve, and blending is what makes resolving it harmless — the limb looks
    /// nearly the same whichever way the tie broke.
    #[test]
    fn an_exact_tie_resolves_and_carries_the_loser_at_full_weight() {
        let table = Table::built_in();
        let size = sized(&[(ContentCategory::Code, 500), (ContentCategory::Docs, 500)]);
        let material = table.material_of(&size, 1000, &limb());
        assert_eq!(
            material.family,
            MaterialFamily::Heartwood,
            "ALL order breaks it"
        );
        assert_eq!(
            material.composition,
            Composition::Blended {
                secondary: MaterialFamily::Parchment,
                weight: Fx::HALF,
            }
        );
    }

    /// The distinction the answer to Phase 4's family question turned on: a container holds
    /// materials it is not made of, and its inventory keeps its tail.
    #[test]
    fn a_container_holds_its_materials_rather_than_being_made_of_them() {
        let table = Table::built_in();
        let size = sized(&[
            (ContentCategory::Docs, 600),
            (ContentCategory::Asset, 300),
            (ContentCategory::Config, 100),
        ]);

        let held = table.material_of(&size, 4096, &container());
        assert_eq!(held.family, MaterialFamily::Parchment);
        let contents = held
            .composition
            .contents()
            .expect("a container reports what it represents — F-INSP-3");
        // Three families, including the 10% tail a blend would have dropped.
        assert_eq!(contents.count(), 3);
        assert_eq!(
            contents.share_of(MaterialFamily::Resin),
            Fx::from_ratio(1, 10)
        );
        assert_eq!(held.composition.secondary(), None);

        // The same content drawn as a limb is a different reading of the same fact.
        let made_of = table.material_of(&size, 4096, &limb());
        assert_eq!(made_of.family, held.family);
        assert_eq!(made_of.composition.secondary(), Some(MaterialFamily::Ore));
    }

    /// PRD §6, "Empty repository" and "Single file": a node with nothing in it is an ordinary
    /// node, and `Stone` is what "treepo knows nothing about this" looks like.
    #[test]
    fn a_node_with_no_content_is_stone_at_the_floor() {
        let table = Table::built_in();
        let material = table.material_of(&SizePrimitives::default(), 0, &limb());
        assert_eq!(material.family, MaterialFamily::Stone);
        assert_eq!(material.composition, Composition::Pure);
        assert_eq!(material.budget, per_mille(table.normalize.floor));
        assert!(
            material.budget > Fx::ZERO,
            "P7: no path is drawn with no pixels"
        );
    }

    /// A container's budget is its own, not one member's — the reason `bytes` is a separate
    /// argument.
    #[test]
    fn a_container_is_budgeted_for_what_it_stands_for() {
        let table = Table::built_in();
        let size = sized(&[(ContentCategory::Code, 100)]);
        let small = table.material_of(&size, 100, &container());
        let large = table.material_of(&size, 100_000_000, &container());
        assert!(large.budget > small.budget);
    }

    #[test]
    fn a_table_that_breaks_a_stated_rule_is_refused() {
        let base = Table::built_in();

        assert_eq!(
            Table {
                version: TABLE_VERSION + 1,
                ..base
            }
            .validate(),
            Err(MaterialError::Version {
                found: TABLE_VERSION + 1,
                expected: TABLE_VERSION,
            })
        );

        // Above half nothing can clear it: blending would be off while looking configured.
        for floor in [0, 501, 1000] {
            let error = Table {
                blend_floor: floor,
                ..base
            }
            .validate()
            .expect_err("should have been refused");
            assert!(matches!(
                error,
                MaterialError::Decision {
                    row: "blend_floor",
                    ..
                }
            ));
        }

        // The F-MAT-3 section's own rules surface through this table rather than being
        // checked twice.
        let error = Table {
            normalize: Normalize {
                quota_cells: 0,
                ..base.normalize
            },
            ..base
        }
        .validate()
        .expect_err("should have been refused");
        assert!(matches!(error, MaterialError::Normalize(_)));
    }

    #[test]
    fn a_malformed_table_names_its_problem() {
        assert!(matches!(
            Table::from_ron("(version: 1"),
            Err(MaterialError::Parse { .. })
        ));
        // `deny_unknown_fields`: a misspelled row must not parse as an unremarkable success,
        // or the user edits the file, reloads, and sees no change.
        assert!(matches!(
            Table::from_ron("(version: 1, blend_flor: 80, normalize: ())"),
            Err(MaterialError::Parse { .. })
        ));
    }
}
