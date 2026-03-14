pub mod text;
pub mod json;
pub mod path_alias;

use std::convert::Infallible;
use std::str::FromStr;

/// Output format selection.
pub enum OutputFormat {
    Text,
    Json,
}

impl FromStr for OutputFormat {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(OutputFormat::Json),
            _ => Ok(OutputFormat::Text),
        }
    }
}
