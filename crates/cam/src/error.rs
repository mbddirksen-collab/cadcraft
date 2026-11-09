use thiserror::Error;

/// CAM assistant failure that is safe to show to a user.
#[derive(Debug, Error)]
pub enum CamError {
    #[error("Eingabe ist zu groß: {0}")]
    InputTooLarge(String),
    #[error("STEP-Datei konnte nicht gelesen werden: {0}")]
    Step(String),
    #[error("Projektdatei ist ungültig: {0}")]
    Project(String),
    #[error("Funktion ist in diesem Backend nicht verfügbar: {0}")]
    NotAvailable(String),
    #[error("E/A-Fehler: {0}")]
    Io(String),
}

/// Result used by the CAM core.
pub type Result<T> = std::result::Result<T, CamError>;
