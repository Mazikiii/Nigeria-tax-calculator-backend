use crate::domain::entities::CheckError;
use lopdf::Document;
use std::load::read;
// what is the point of this file?
// i want to check things
// is the pdf encrypted?
// is the pdf already free?
// is the pdf corrupted?
// i want to decrypt passwords, when a user puts password, and send a vec<u8> the bytes
// if the pdf is free, i want to turn it to vec<u8> and send
// i need one function that checks 3 things and acts accordingly
// check if the pdf is free, if it is use the function that converts to vec<u8>
// check if it is encrypted, if it is push that pdf to a storage
// if it is corrupted send an error
//
//what data structure do i need?
// i need a struct for the output of the check function
// it'll be giving an Result<vec<u8>> the err would be corrupted pdf
// for the input for that checker i need the pdf only

pub struct PdfEncryptionChecker;

impl PdfEncryptionChecker {
    pub fn new() -> Self {
        Self
    }

    pub fn check_pdf(&self, pdf_upload: &[u8]) -> Result<CheckerResult, CheckingError> {
        let mut pdf_bytes: Vec<u8> = std::fs::load(pdf)?;

        let pdf = Document::load_mem(&pdf_bytes)
            .map_err(|e| CheckingError::CorruptedPdf(e.to_string()))?;

        // to_vec() justs puts the bytes in a vector
        if pdf.is_encrypted() {
            Ok(CheckerResult::LockedPdf(pdf.to_vec())
                .map_err(|e| CheckingError::CorruptedPdf(e.to_string())))?;
        }

        Ok(CheckerResult::UnlockedPdf(pdf.to_vec()))
    }
    // okay i thought about this one and this should return a option(vec<u8>) and an option(vec<u8>), its either encrypted or decrypted

    // to decrypt a pdf i need to collect the  pdf and the password
    // and return a vec<u8> / wrong password error
    pub fn decrypt_pdf(&self, pdf_bytes: &[u8], pass: &str) -> Result<Vec, CheckingError> {
        // the checker will aleady return a pdf_byte

        // now its converted i need to load the bytes, signaling its a pdf so i can check if its encrypted
        let pdf = Document
            .load_mem(pdf_bytes)
            .map_err(|e| CheckerError::CorruptedPdf(e.to_string()))?;

        if pdf.is_encrypted() {
            pdf.authenticate_password(pass.as_bytes())
                .map_err(|e| CheckerError::WrongPassword)?; // this just decryptes the pdf assuming the pass is correct
                                                            // i still have to save the new decrypted pdf somewhere as its hanging
        }
        // container for the decrypted data
        let mut decrypted_pdf = Vec::new();

        pdf.save_to(&mut decrypted_pdf)?;

        ok(decrypted_pdf);
    }
}
