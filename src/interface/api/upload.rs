// this api is responsible for collecting the users uploaded pdfs
// then checks it, if there are encrypted pdfs it passes a state and the data
// it can handle processing
use crate::application::ingestion::StatementProcessor;
use crate::domain::entities::{CorruptedPdfInfo, LockedPdfInfo, PdfFile, PdfStates};
use actix_multipart::form::{tempfile::TempFile, MultipartForm};
use actix_web::{post, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use uuid::*;
use tokio::fs;

pub struct CheckPdfs{
    processor: StatmentProcessor,
}

// okay now how do i even recieve pdfs? i use rusts actic-multipart crate to do that
// and the first order of business after adding it to cargo and importing it is to define a special struct
#[derive(MultiPartForm)]
pub struct MediaForm {
    // this is the actual thing that comes from a client device, web, mobile, tablet
    // so alot of things can come here, even json from the device
    #[MultiPart(limit = "100MB")] // setting size limit
    files: Vec<TempFile>, // whether its one thing or more its type is tempFile and always put it in a vec

    // i also want to get the year the user is uploading too and their tax type
    #[derive(deserialize)] // converts json to rust code
    #[MultiPart(rename = "metadata")]
    user_data: Json<UserDatas>, // Reads the JSON string from the multipart field then Deserializes it into `UserDatas` struct using serde
}

// the particular data will come as json and this is where i store it
pub struct UserDatas {
    year_of_upload: u32,
    tax_entity: String,
}

// there are 2 types of flows
// the first one is the checking flow
// two things can be returned here, either all pdfs are not locked or corrupted and it complete
// or some pdfs are locked and it responds with the fact that some pdfs are locked and returns details
//
// theres a second flow which is after_password flow
// two things, one is that the passwords inputted was correct then it takes everything and responed as complete and processes
// second is that some or one password is wrong and it shows, incorrect password with details
// i need two structs for this different flows, withs similar responses
#[derive(serialize)] // this converts my struct to a json
#[serde(tag = "status")] // i need tagging
                         // first senario
pub enum UploadCheckBatch {
    #[serde(rename = "completed")]
    Completed {
        // all pdfs were unlocked and none were corrupt
        batch_id: String,
        processed_pdf: BatchProcessed,
    },

    #[serde(tag = "need_password")]
    NeedPassword {
        batch_id: String,
        locked_pdfs: Vec<LockedPdfInfo>,
        corrupted_pdfs: Vec<CorruptedPdfInfo>,
    },
}

pub enum DecryptBatch {
    #[serde(tag = "completed")]
    Completed {
        batch_id: String,
        processed_pdfs: BatchProcessed,
    },

    #[serde(tag = "incorrect_password")]
    IncorrectPassword {
        batch_id: String,
        wrong_password: Vec<PdfFile>,
    },
}

// with the enums down i need the actual functions to relate with the application layer

impl CheckPdfs{

pub fn new(processor: StatementProcessor) -> Self{
    Self{
        processor
}
}
// this is where the actual pdfs fall in
// notice the input, thats how you pass files that come from devices, into a function
pub async fn check_batch(data(form): MultipartForm<MediaForm>) -> impl Responder {
    // i have to first assign the tax_year and entity
    let tax_year = form.user_data.year_of_upload;
    let user_type = form.user_data.tax_entity;
    let mut pdfs_container = Vec::new();

    for pdf in form.files{
        let converter = tempfile_to_pdffile(pdf);

        pdfs_container.push(converter);
    }

    let check_pdfs =
    // i need to work on the pdfs and convert it to vec<u8>'s

}

// how do i then convert the files uploaded to what i need in code vec<u8>
// this function needs to return my entity PdfFile
pub fn tempfile_to_pdffile(temp: TempFile) -> Result <PdfFile , tokio::io::Error>{
    let data = fs::read(temp.files.path()).await?; // this is how you convert provided files to bytes
    // failure can happen so handle that

    Ok(PdfFile{
        id: uuid::Uuid::new_v4().to_string(),
        name: temp.file_name.unwrap_or_else(|| "Pdf has no Name".to_string()),
        data
    })
}

}
