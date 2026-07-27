//! The extracted feature system — `design/feature-system.md` §3, one module per category.
//!
//! These are *measurements*, not decisions. Nothing here interprets: no thresholds, no
//! classification, no aesthetic judgement. `N7` requires every visual property derive from
//! primitives, and that rule only means something if the primitives themselves are
//! measurements a second implementation would agree with.
//!
//! # Why structured records rather than scalars
//!
//! `design/feature-system.md` §2 is explicit that flattening loses the information later
//! stages need: a `public` folder full of static assets means something different from one
//! full of binaries, and a single `is_public: bool` cannot tell them apart. So folder
//! signals carry their modulation, balance carries three axes rather than one number, and
//! branching carries a histogram rather than a mean.
//!
//! # Numeric conventions
//!
//! * Counts are `u32`, sizes `u64`, timestamps `i64` seconds since the Unix epoch.
//! * Everything computed — ratios, scores, rates — is [`Fx`](treepo_det::Fx), never a
//!   float. A ratio in a manifest that came from a platform `libm` would make the manifest
//!   differ by machine and lose `AC-MAN-1`.
//! * Ratios that are proportions of a whole are in `0..=1`. Scores with a natural centre
//!   say so where they are declared.

pub mod derived;
pub mod folder_signal;
pub mod ownership;
pub mod size;
pub mod structural;
pub mod temporal;

pub use derived::DerivedSignals;
pub use folder_signal::{ContentModulation, FolderSignal, HierarchyPosition};
pub use ownership::{AuthorShare, OwnershipPrimitives};
pub use size::{LineCounts, SizePrimitives};
pub use structural::{BalanceScore, BranchingHistogram, DepthProfile, StructuralPrimitives};
pub use temporal::{ChurnWindows, TemporalPrimitives};
