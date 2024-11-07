use std::{
	fmt::{Display, Formatter},
	ops::Range,
};

use warpforge_api::formula::FormulaAndContext;
use warpforge_terminal::{debug, warn};

const MAX_TRAILING_COMMA: usize = 20;

pub fn validate_formula(formula: &str) -> Result<ValidatedFormula> {
	// Documentation from serde_json::from_reader about performance:
	// "Note that counter to intuition, this function (from_reader) is usually
	// slower than reading a file completely into memory and then applying
	// `from_str` or `from_slice` on it. See [issue #160]."
	// [issue #160]: https://github.com/serde-rs/json/issues/160

	let mut modified_formula = None;

	// We parse to `serde_json::Value` because we want to be able to generate
	// multiple erros if present: When deserializing to a struct, serde_json
	// fails fast and only reports the first error. For users this can lead to
	// a tedious bug chasing, where they 1st fix one thing, 2nd rerun, 3rd get
	// the next error. Instead we want to show all errors we can find at once.
	let parsed = serde_json::from_str::<serde_json::Value>(formula);

	// Handle json syntax errors.
	let (parsed, mut errors) = match parsed {
		Ok(parsed) => (parsed, Vec::with_capacity(0)),
		Err(mut err) => {
			// Replacing trailing commas with white space is an easy fix,
			// which always works. We do this to be able to continue parsing
			// and find as many errors as possible.
			let mut errors = Vec::new();
			loop {
				if !err_is_trailing_comma(&err) {
					errors.push(ValidationError::Serde(err));
					return Err(Error::Invalid { errors });
				} else {
					let (line, column) = (err.line(), err.column());
					errors.push(ValidationError::Serde(err));

					let Some(mut offset) = find_byte_offset(formula.as_bytes(), line, column)
					else {
						warn!("trailing comma error, but could not find comma");
						return Err(Error::Invalid { errors });
					};

					// We only create the vector on the error path to avoid allocations on the hot path.
					modified_formula =
						Some(modified_formula.unwrap_or_else(|| formula.as_bytes().to_owned()));
					let modified_formula = modified_formula.as_mut().unwrap();

					// Find trailing comma, since serde_json points to closing braces instead of comma.
					// `serde_json::Error` does not allow us to match the concrete error kind,
					// so we look at the emitted error message.
					while offset > 0 {
						offset -= 1;
						if modified_formula[offset] == b',' {
							break;
						} else if !modified_formula[offset].is_ascii_whitespace() {
							warn!("trailing comma error, but could not find comma");
							return Err(Error::Invalid { errors });
						}
					}
					modified_formula[offset] = b' ';

					let Some(ValidationError::Serde(serde_error)) = errors.pop() else {
						debug!("failed to pop value, we just pushed");
						return Err(Error::Invalid { errors });
					};
					errors.push(ValidationError::TrailingComma(TrailingComma {
						span: offset..(offset + 1),
						serde_error,
					}));

					if errors.len() >= MAX_TRAILING_COMMA {
						return Err(Error::Invalid { errors });
					}

					match serde_json::from_slice::<serde_json::Value>(modified_formula) {
						// We only encountered trailing comma errors, we
						// continue validation to potentially find other errors.
						Ok(parsed) => break (parsed, errors),

						Err(next_err) => err = next_err,
					}
				}
			}
		}
	};

	let deserialize_err = if errors.is_empty() {
		match serde_json::from_value(parsed) {
			Ok(validated) => return Ok(ValidatedFormula { formula: validated }),
			Err(err) => Some(err),
		}
	} else {
		None
	};

	// Parse again with serde_json::from_slice to get line and column in error.
	// serde_json::from_value populates line and column with 0.
	let source = modified_formula.as_deref().unwrap_or(formula.as_bytes());
	let parse_result = serde_json::from_slice::<FormulaAndContext>(source);
	match (parse_result, deserialize_err) {
		(Err(err), _) => {
			errors.push(ValidationError::Serde(err));
		}
		(Ok(_), None) => {}
		(Ok(_), Some(err)) => {
			debug!("serde_json::from_value found error that serde_json::from_slice did not");
			errors.push(ValidationError::Serde(err));
		}
	}

	Err(Error::Invalid { errors })
}

pub struct ValidatedFormula {
	pub formula: FormulaAndContext,
}

fn find_byte_offset(src: &[u8], line: usize, column: usize) -> Option<usize> {
	let mut walk_line = 1;
	let mut walk_column = 1;
	let mut offset = 0;
	while offset < src.len() && (walk_line < line || (walk_line == line && walk_column < column)) {
		if src[offset] == b'\n' {
			walk_line += 1;
			walk_column = 1;
		} else {
			walk_column += 1;
		}
		offset += 1;
	}

	if offset >= src.len() || walk_line != line || walk_column != column {
		None
	} else {
		Some(offset)
	}
}

fn err_is_trailing_comma(err: &serde_json::Error) -> bool {
	// serde_json provides no better way to branch on a concrete error type.
	err.is_syntax() && format!("{err}").starts_with("trailing comma")
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("failed validation")]
	Invalid { errors: Vec<ValidationError> },
}

#[derive(Debug)]
pub enum ValidationError {
	Serde(serde_json::Error),

	TrailingComma(TrailingComma),

	Custom(CustomError),
}

#[derive(Debug)]
pub struct TrailingComma {
	pub span: Range<usize>,
	pub serde_error: serde_json::Error,
}

#[derive(Debug)]
pub struct CustomError {
	pub span: Range<usize>,
	pub message: String,
}

impl ValidationError {
	pub fn is_trailing_comma(&self) -> bool {
		matches!(self, ValidationError::TrailingComma(_))
	}

	pub fn span(&self, source: &str) -> Option<Range<usize>> {
		match self {
			ValidationError::Serde(error) => {
				find_byte_offset(source.as_bytes(), error.line(), error.column())
					.map(|offset| offset..offset)
			}
			ValidationError::TrailingComma(error) => Some(error.span.clone()),
			ValidationError::Custom(error) => Some(error.span.clone()),
		}
	}
}

impl Display for ValidationError {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			ValidationError::Serde(err) => write!(fmt, "{err}"),
			ValidationError::TrailingComma(trailing_comma) => {
				write!(fmt, "{}", trailing_comma.serde_error)
			}
			ValidationError::Custom(custom_error) => write!(fmt, "{}", custom_error.message),
		}
	}
}
