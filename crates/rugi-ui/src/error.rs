//! API misuse reporting.

/// API misuse detected by the context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiError {
    /// `begin_frame` called while a frame is in progress.
    FrameInProgress,
    /// A frame-only call was made outside `begin_frame`/`end_frame`.
    NoFrame,
    /// `end` was called with no open container.
    NothingToEnd,
    /// `end_frame` found containers that were never ended (auto-closed).
    UnclosedContainers(usize),
    /// A root-only container (panel) was opened inside another container.
    PanelNotAtRoot,
    /// `push_style`/`push_font` without a matching `pop_style` (auto-restored).
    UnbalancedStyle,
    /// Invalid argument.
    InvalidArgument(&'static str),
}

impl std::fmt::Display for UiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiError::FrameInProgress => write!(f, "begin_frame called while a frame is in progress"),
            UiError::NoFrame => write!(f, "call made outside begin_frame/end_frame"),
            UiError::NothingToEnd => write!(f, "end called with no open container"),
            UiError::UnclosedContainers(n) => write!(f, "{n} container(s) were not ended; auto-closed"),
            UiError::PanelNotAtRoot => write!(f, "panels can only be opened at the root level"),
            UiError::UnbalancedStyle => write!(f, "push_style without matching pop_style; theme restored"),
            UiError::InvalidArgument(a) => write!(f, "invalid argument: {a}"),
        }
    }
}

impl std::error::Error for UiError {}
