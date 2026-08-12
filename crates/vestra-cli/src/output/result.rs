use serde::Serialize;

use vestra::Diagnostic;

#[derive(Serialize)]
pub struct ResultEnvelope<T: Serialize> {
    pub result_schema_version: u8,
    pub status: &'static str,
    pub command: &'static str,
    #[serde(flatten)]
    pub data: T,
}

#[derive(Serialize)]
pub struct FailureEnvelope {
    pub errors: Vec<Diagnostic>,
    pub warnings: Vec<Diagnostic>,
}
