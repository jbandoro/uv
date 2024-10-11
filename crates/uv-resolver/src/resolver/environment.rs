#![allow(warnings)]

use std::sync::Arc;

use uv_pep508::{MarkerEnvironment, MarkerTree};
use uv_pypi_types::ResolverMarkerEnvironment;

use crate::requires_python::RequiresPythonRange;
use crate::resolver::ForkState;
use crate::PythonRequirement;
use crate::ResolveError;

/// Represents the environment that dependencies must satisfy in a resolution.
///
/// Callers must provide this to the resolver to indicate, broadly, what kind
/// of resolution it will produce. Generally speaking, callers should provide
/// a specific marker environment for `uv pip`-style resolutions and ask for a
/// universal resolution for uv's project based commands like `uv lock`.
///
/// Callers can rely on this type being reasonably cheap to clone.
///
/// # Internals
///
/// Inside the resolver, when doing a universal resolution, it may create
/// many "forking" states to deal with the fact that there may be multiple
/// incompatible dependency specifications. Specifically, in the Python world,
/// the main constraint is that for any one *specific* marker environment,
/// there must be only one version of a package in a corresponding resolution.
/// But when doing a universal resolution, we want to support many marker
/// environments, and in this context, the "universal" resolution may contain
/// multiple versions of the same package. This is allowed so long as, for
/// any marker environment supported by this resolution, an installation will
/// select at most one version of any given package.
///
/// During resolution, a `ResolverEnvironment` is attached to each internal
/// fork. For non-universal or "specific" resolution, there is only ever one
/// fork because a `ResolverEnvironment` corresponds to one and exactly one
/// marker environment. For universal resolution, the resolver may choose
/// to split its execution into multiple branches. Each of those branches
/// (also called "forks" or "splits") will get its own marker expression that
/// represents a set of marker environments that is guaranteed to be disjoint
/// with the marker environments described by the marker expressions of all
/// other branches.
///
/// Whether it's universal resolution or not, and whether it's one of many
/// forks or one fork, this type represents the set of possible dependency
/// specifications allowed in the resolution produced by a single fork.
///
/// An exception to this is `requires-python`. That is handled separately and
/// explicitly by the resolver. (Perhaps a future refactor can incorporate
/// `requires-python` into this type as well, but it's not totally clear at
/// time of writing if that's a good idea or not.)
#[derive(Clone, Debug)]
pub struct ResolverEnvironment {
    kind: Kind,
}

/// The specific kind of resolver environment.
///
/// Note that it is explicitly intended that this type remain unexported from
/// this module. The motivation for this design is to discourage repeated case
/// analysis on this type, and instead try to encapsulate the case analysis via
/// higher level routines on `ResolverEnvironment` itself. (This goal may prove
/// intractable, so don't treat it like gospel.)
#[derive(Clone, Debug)]
enum Kind {
    /// We're solving for one specific marker environment only.
    ///
    /// Generally, this is what's done for `uv pip`. For the project based
    /// commands, like `uv lock`, we do universal resolution.
    Specific {
        /// The marker environment being resolved for.
        ///
        /// Any dependency specification that isn't satisfied by this marker
        /// environment is ignored.
        marker_env: ResolverMarkerEnvironment,
    },
    /// We're solving for all possible marker environments.
    Universal {
        /// The initial set of "fork preferences." These will come from the
        /// lock file when available, or the list of supported environments
        /// explicitly written into the `pyproject.toml`.
        ///
        /// Note that this may be empty, which means resolution should begin
        /// with no forks. Or equivalently, a single fork whose marker
        /// expression matches all marker environments.
        initial_forks: Arc<[MarkerTree]>,
        /// The markers associated with this resolver fork.
        markers: MarkerTree,
    },
}

impl ResolverEnvironment {
    /// Create a resolver environment that is fixed to one and only one marker
    /// environment.
    ///
    /// This enables `uv pip`-style resolutions. That is, the resolution
    /// returned is only guaranteed to be installable for this specific marker
    /// environment.
    pub fn specific(marker_env: ResolverMarkerEnvironment) -> ResolverEnvironment {
        let kind = Kind::Specific { marker_env };
        ResolverEnvironment { kind }
    }

    /// Create a resolver environment for producing a multi-platform
    /// resolution.
    ///
    /// The set of marker expressions given corresponds to an initial
    /// seeded set of resolver branches. This might come from a lock file
    /// corresponding to the set of forks produced by a previous resolution, or
    /// it might come from a human crafted set of marker expressions.
    ///
    /// The "normal" case is that the initial forks are empty. When empty,
    /// resolution will create forks as needed to deal with potentially
    /// conflicting dependency specifications across distinct marker
    /// environments.
    ///
    /// The order of the initial forks is significant. (It is unclear to AG
    /// at time of writing what precisely is significant about it. Is this
    /// just a matter of it being order dependent but otherwise no specific
    /// guarantees are provided, or is precedence given to earlier forks over
    /// later forks?)
    pub fn universal(initial_forks: Vec<MarkerTree>) -> ResolverEnvironment {
        let kind = Kind::Universal {
            initial_forks: initial_forks.into(),
            markers: MarkerTree::TRUE,
        };
        ResolverEnvironment { kind }
    }

    pub(crate) fn included(&self, marker: &MarkerTree) -> bool {
        match self.kind {
            Kind::Specific { ref marker_env } => marker.evaluate(marker_env, &[]),
            Kind::Universal { ref markers, .. } => !markers.is_disjoint(marker),
        }
    }

    pub(crate) fn in_fork(&self, marker: &MarkerTree) -> bool {
        match self.kind {
            Kind::Specific { .. } => true,
            Kind::Universal { ref markers, .. } => !markers.is_disjoint(marker),
        }
    }

    pub fn marker_environment(&self) -> Option<&MarkerEnvironment> {
        match self.kind {
            Kind::Specific { ref marker_env } => Some(marker_env),
            Kind::Universal { .. } => None,
        }
    }

    pub(crate) fn narrow_markers(&self, rhs: &MarkerTree) -> ResolverEnvironment {
        match self.kind {
            Kind::Specific { .. } => self.clone(),
            Kind::Universal {
                ref initial_forks,
                markers: ref lhs,
            } => {
                let mut lhs = lhs.clone();
                lhs.and(rhs.clone());
                let kind = Kind::Universal {
                    initial_forks: initial_forks.clone(),
                    markers: lhs,
                };
                ResolverEnvironment { kind }
            }
        }
    }

    pub(crate) fn forked_states(&self, init: ForkState) -> Vec<ForkState> {
        let Kind::Universal {
            ref initial_forks, ..
        } = self.kind
        else {
            return vec![init];
        };
        if initial_forks.is_empty() {
            return vec![init];
        }
        initial_forks
            .iter()
            .rev()
            .map(|initial_fork| init.clone().with_env(&initial_fork))
            .collect()
    }

    pub(crate) fn narrow_python_requirement(
        &self,
        python_requirement: &PythonRequirement,
    ) -> Option<PythonRequirement> {
        Some(python_requirement.narrow(&self.requires_python_range()?)?)
    }

    pub(crate) fn end_user_fork_display(&self) -> Option<impl std::fmt::Display + '_> {
        match self.kind {
            Kind::Specific { .. } => None,
            Kind::Universal { ref markers, .. } => {
                if markers.is_true() {
                    None
                } else {
                    Some(format!("split ({markers:?})"))
                }
            }
        }
    }

    fn requires_python_range(&self) -> Option<RequiresPythonRange> {
        crate::marker::requires_python(self.markers())
    }

    // TODO: Unexport this. It ought to be encapsulated.
    pub(crate) fn markers(&self) -> &MarkerTree {
        match self.kind {
            Kind::Specific { .. } => &MarkerTree::TRUE,
            Kind::Universal { ref markers, .. } => markers,
        }
    }

    // TODO: Unexport this. It ought to be encapsulated.
    pub(crate) fn try_markers(&self) -> Option<&MarkerTree> {
        match self.kind {
            Kind::Specific { .. } => None,
            Kind::Universal { ref markers, .. } => {
                if markers.is_true() {
                    None
                } else {
                    Some(markers)
                }
            }
        }
    }
}

impl std::fmt::Display for ResolverEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.kind {
            Kind::Specific { .. } => write!(f, "Marker Environment"),
            Kind::Universal { ref markers, .. } => {
                if markers.is_true() {
                    write!(f, "All Marker Environments")
                } else {
                    write!(f, "Split `{markers:?}`")
                }
            }
        }
    }
}
