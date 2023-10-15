use crate::errors::SplitterError;
pub(crate) use std::result::Result as StdResult;

pub type Result<T> = StdResult<T, SplitterError>;

pub(crate) use tracing::{debug, error, info, trace, warn};
