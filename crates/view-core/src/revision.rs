use crate::CoreError;

/// A monotonic version within one explicitly owned revision stream.
///
/// Zero is the initial version. Compare only values from the same stream;
/// document, request, committed and presented versions are separate streams.
/// This value alone does not publish a commit or prove that it was presented.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(u64);

impl Revision {
    /// Initial version before any change in this stream.
    pub const INITIAL: Self = Self(0);

    /// Reconstruct a version from an observation of the same stream.
    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    /// Return the version number, without its owner/stream context.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Produce the next version, failing rather than wrapping or saturating.
    ///
    /// The caller assigns the result only when its corresponding change succeeds.
    pub fn checked_next(self) -> Result<Self, CoreError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(CoreError::RevisionExhausted)
    }
}
