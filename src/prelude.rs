use crate::errors::SplitterError;
pub(crate) use std::result::Result as StdResult;
pub(crate) use twba_common::prelude::*;
pub type Result<T> = StdResult<T, SplitterError>;

pub(crate) use tracing::{debug, error, info, trace, warn};
