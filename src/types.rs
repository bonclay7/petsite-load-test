use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Endpoints {
    pub petlistadoptions: String,
    pub petsearch: String,
    pub payforadoption: String,
    pub petfood: String,
}

impl Endpoints {
    pub fn new() -> Self {
        Self {
            petlistadoptions: std::env::var("PETLIST_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            petsearch: std::env::var("PETSEARCH_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:8081".to_string()),
            payforadoption: std::env::var("PAYFORADOPTION_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:8082".to_string()),
            petfood: std::env::var("PETFOOD_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:8083".to_string()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.petlistadoptions.is_empty()
            && self.petsearch.is_empty()
            && self.payforadoption.is_empty()
            && self.petfood.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct RequestResult {
    pub method: String,
    pub url: String,
    pub user_id: String,
    pub success: bool,
    pub response_time: Duration,
    pub status: u16,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct UserScenarioResult {
    pub user_id: String,
    pub requests: Vec<RequestResult>,
    pub total_time: Duration,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct LoadTestResults {
    pub total_requests: usize,
    pub successful_requests: usize,
    pub failed_requests: usize,
    pub average_response_time: Duration,
    pub total_test_time: Duration,
    pub requests_per_second: f64,
    pub success_rate: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Pet {
    pub petid: String,
    pub availability: Option<String>,
    pub cuteness_rate: Option<String>,
    pub petcolor: Option<String>,
    pub pettype: Option<String>,
    pub price: Option<String>,
    pub peturl: Option<String>,
}

// The API returns an array of pets directly, not wrapped in an object
pub type PetListResponse = Vec<Pet>;

// Adoption is now done via query parameters, not JSON payload
#[derive(Debug)]
pub struct AdoptionRequest {
    pub pet_id: String,
    pub pet_type: String,
    pub user_id: String,
}

// Food cart operations now use dynamic JSON payloads via serde_json::json!
