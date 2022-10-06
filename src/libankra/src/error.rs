use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnkraError {
    #[error("`{0}`")]
    IoError(#[from] std::io::Error),
    #[error("`{0}`")]
    CsvParseError(#[from] csv::Error),
    #[error("`parsing error {0}`")]
    ZmeraldError(#[from] zmerald::error::SpannedError),
    #[error("kb parse error")]
    KbParseError
}