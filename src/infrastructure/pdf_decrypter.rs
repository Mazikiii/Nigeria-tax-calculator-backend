use crate::domain::entities::{CheckError, CheckerResult};
use lopdf::Document;

// the old idea was to let the parser deal with encrypted pdf behavior too
// that was too much responsibility in one layer, and it makes the parser harder to reason about
// this keeps pdf byte handling separate from parsing so the parser only sees readable bytes
pub struct PdfEncryptionChecker;

impl PdfEncryptionChecker {
    pub fn new() -> Self {
        Self
    }

    // the previous approach tried to do file checks and parsing in one place
    // the better approach is to stop at the byte boundary here and only say whether the pdf is usable
    pub fn check_pdf(&self, pdf_upload: &[u8]) -> Result<CheckerResult, CheckError> {
        let pdf =
            Document::load_mem(pdf_upload).map_err(|e| CheckError::CorruptedPdf(e.to_string()))?;

        if pdf.is_encrypted() {
            return Ok(CheckerResult::LockedPdf(pdf_upload.to_vec()));
        }

        Ok(CheckerResult::UnlockedPdf(pdf_upload.to_vec()))
    }

    // only unlock the document and returns bytes the parser can consume directly
    pub fn decrypt_pdf(&self, pdf_bytes: &[u8], pass: &str) -> Result<Vec<u8>, CheckError> {
        let mut pdf =
            Document::load_mem(pdf_bytes).map_err(|e| CheckError::CorruptedPdf(e.to_string()))?;

        if pdf.is_encrypted() {
            pdf.authenticate_password(pass)
                .map_err(|_| CheckError::WrongPassword)?;
        }

        let mut decrypted_pdf = Vec::new();
        pdf.save_to(&mut decrypted_pdf)
            .map_err(|e| CheckError::CorruptedPdf(e.to_string()))?;

        Ok(decrypted_pdf)
    }
}
