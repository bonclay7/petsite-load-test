use colored::*;
use futures::future::join_all;
use rand::Rng;
use reqwest::Client;
use std::time::{Duration, Instant};
use tokio::time::timeout;

use crate::types::*;

pub struct LoadTester {
    user_count: usize,
    concurrent_requests: usize,
    endpoints: Endpoints,
    dry_run: bool,
    client: Client,
}

impl LoadTester {
    pub fn new(
        user_count: usize,
        concurrent_requests: usize,
        endpoints: Endpoints,
        dry_run: bool,
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            user_count,
            concurrent_requests,
            endpoints,
            dry_run,
            client,
        }
    }

    fn generate_users(&self) -> Vec<String> {
        (1..=self.user_count)
            .map(|i| format!("user{:04}", i))
            .collect()
    }

    pub async fn run_load_test(&self) -> anyhow::Result<Vec<UserScenarioResult>> {
        println!("{}", "\n🎯 Starting load test...".blue());

        let users = self.generate_users();
        let mut all_futures = Vec::new();

        // Create concurrent futures for all user scenarios
        for _ in 0..self.concurrent_requests {
            for user_id in &users {
                let future = self.run_scenario_for_user(user_id.clone());
                all_futures.push(future);
            }
        }

        println!(
            "{}",
            format!("⚡ Running {} concurrent scenarios...", all_futures.len()).yellow()
        );

        let results = join_all(all_futures).await;
        Ok(results)
    }

    async fn run_scenario_for_user(&self, user_id: String) -> UserScenarioResult {
        let start_time = Instant::now();
        let mut requests = Vec::new();
        let mut success = true;
        let mut error = None;

        // Step 1: List all available pets
        let list_pets_result = self
            .make_request(
                "GET",
                &format!("{}/api/petlistadoptions", self.endpoints.petlistadoptions),
                &user_id,
                None::<()>,
            )
            .await;

        let pets_data = if list_pets_result.success {
            // Try to parse the response as JSON to get pets
            if let Ok(response_text) = reqwest::get(&format!("{}/api/petlistadoptions", self.endpoints.petlistadoptions)).await {
                if let Ok(text) = response_text.text().await {
                    serde_json::from_str::<PetListResponse>(&text).ok()
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            success = false;
            None
        };

        requests.push(list_pets_result);

        if !success {
            return UserScenarioResult {
                user_id,
                requests,
                total_time: start_time.elapsed(),
                success: false,
                error: Some("Failed to list pets".to_string()),
            };
        }

        // Step 2: Get random petID from results
        let pet_id = if let Some(pets_response) = pets_data {
            if !pets_response.pets.is_empty() {
                let mut rng = rand::thread_rng();
                let random_pet = &pets_response.pets[rng.gen_range(0..pets_response.pets.len())];
                random_pet.pet_id.clone()
            } else {
                format!("pet{}", rand::thread_rng().gen_range(1..1000))
            }
        } else {
            format!("pet{}", rand::thread_rng().gen_range(1..1000))
        };

        // Step 3: Search for specific pet
        let search_result = self
            .make_request(
                "GET",
                &format!("{}/api/petsearch?petid={}", self.endpoints.petsearch, pet_id),
                &user_id,
                None::<()>,
            )
            .await;
        requests.push(search_result);

        // Step 4: Pay for adoption
        let adoption_payload = AdoptionPayload {
            pet_id: pet_id.clone(),
            user_id: user_id.clone(),
            adoption_fee: 50.0,
        };
        let adoption_result = self
            .make_request(
                "POST",
                &format!("{}/api/payforadoption", self.endpoints.payforadoption),
                &user_id,
                Some(adoption_payload),
            )
            .await;
        requests.push(adoption_result);

        // Step 5: Pet food operations
        // List all food
        let food_list_result = self
            .make_request(
                "GET",
                &format!("{}/api/petfood", self.endpoints.petfood),
                &user_id,
                None::<()>,
            )
            .await;
        requests.push(food_list_result);

        // Add food to cart
        let add_food_payload = FoodCartPayload {
            user_id: user_id.clone(),
            food_id: "food001".to_string(),
            quantity: 2,
        };
        let add_food_result = self
            .make_request(
                "POST",
                &format!("{}/api/petfood/cart", self.endpoints.petfood),
                &user_id,
                Some(add_food_payload),
            )
            .await;
        requests.push(add_food_result);

        // Pay for food
        let pay_food_payload = PayFoodPayload {
            user_id: user_id.clone(),
            cart_total: 25.99,
        };
        let pay_food_result = self
            .make_request(
                "POST",
                &format!("{}/api/petfood/pay", self.endpoints.petfood),
                &user_id,
                Some(pay_food_payload),
            )
            .await;
        requests.push(pay_food_result);

        // Step 6: Cleanup (optional DELETE operations)
        let cleanup_result = self
            .make_request(
                "DELETE",
                &format!("{}/api/adoption/{}", self.endpoints.payforadoption, pet_id),
                &user_id,
                None::<()>,
            )
            .await;
        requests.push(cleanup_result);

        // Check if any request failed
        success = requests.iter().all(|r| r.success);
        if !success {
            error = Some("One or more requests failed".to_string());
        }

        UserScenarioResult {
            user_id,
            requests,
            total_time: start_time.elapsed(),
            success,
            error,
        }
    }

    async fn make_request<T: serde::Serialize>(
        &self,
        method: &str,
        url: &str,
        user_id: &str,
        data: Option<T>,
    ) -> RequestResult {
        let start_time = Instant::now();

        if self.dry_run {
            println!("{}", format!("[DRY RUN] {} {} ({})", method, url, user_id).bright_black());
            return RequestResult {
                method: method.to_string(),
                url: url.to_string(),
                user_id: user_id.to_string(),
                success: true,
                response_time: Duration::from_millis(0),
                status: 200,
                error: None,
            };
        }

        let request_future = async {
            let mut request_builder = match method {
                "GET" => self.client.get(url),
                "POST" => {
                    let mut builder = self.client.post(url);
                    if let Some(payload) = data {
                        builder = builder.json(&payload);
                    }
                    builder
                }
                "PUT" => {
                    let mut builder = self.client.put(url);
                    if let Some(payload) = data {
                        builder = builder.json(&payload);
                    }
                    builder
                }
                "DELETE" => self.client.delete(url),
                _ => return Err(anyhow::anyhow!("Unsupported HTTP method: {}", method)),
            };

            request_builder = request_builder.header("User-Agent", format!("LoadTester-{}", user_id));

            let response = request_builder.send().await?;
            let status = response.status().as_u16();
            
            // Consume the response body to complete the request
            let _body = response.text().await?;
            
            Ok(status)
        };

        match timeout(Duration::from_secs(10), request_future).await {
            Ok(Ok(status)) => RequestResult {
                method: method.to_string(),
                url: url.to_string(),
                user_id: user_id.to_string(),
                success: status >= 200 && status < 400,
                response_time: start_time.elapsed(),
                status,
                error: None,
            },
            Ok(Err(err)) => RequestResult {
                method: method.to_string(),
                url: url.to_string(),
                user_id: user_id.to_string(),
                success: false,
                response_time: start_time.elapsed(),
                status: 0,
                error: Some(err.to_string()),
            },
            Err(_) => RequestResult {
                method: method.to_string(),
                url: url.to_string(),
                user_id: user_id.to_string(),
                success: false,
                response_time: start_time.elapsed(),
                status: 0,
                error: Some("Request timeout".to_string()),
            },
        }
    }

    pub fn display_results(&self, results: &[UserScenarioResult], total_time: Duration) {
        let all_requests: Vec<&RequestResult> = results.iter().flat_map(|r| &r.requests).collect();

        let total_requests = all_requests.len();
        let successful_requests = all_requests.iter().filter(|r| r.success).count();
        let failed_requests = total_requests - successful_requests;

        let successful_response_times: Vec<Duration> = all_requests
            .iter()
            .filter(|r| r.success)
            .map(|r| r.response_time)
            .collect();

        let average_response_time = if !successful_response_times.is_empty() {
            successful_response_times.iter().sum::<Duration>() / successful_response_times.len() as u32
        } else {
            Duration::from_millis(0)
        };

        let requests_per_second = if total_time.as_secs_f64() > 0.0 {
            total_requests as f64 / total_time.as_secs_f64()
        } else {
            0.0
        };

        let success_rate = if total_requests > 0 {
            (successful_requests as f64 / total_requests as f64) * 100.0
        } else {
            0.0
        };

        println!("{}", "\n📊 Load Test Results".green().bold());
        println!("{}", "═".repeat(50).bright_black());

        println!("{}", format!("Total Scenarios: {}", results.len()).blue());
        println!("{}", format!("Total Requests: {}", total_requests).blue());
        println!("{}", format!("✓ Successful: {}", successful_requests).green());
        println!("{}", format!("✗ Failed: {}", failed_requests).red());
        println!("{}", format!("Success Rate: {:.1}%", success_rate).yellow());
        println!("{}", format!("Average Response Time: {}ms", average_response_time.as_millis()).cyan());
        println!("{}", format!("Requests/Second: {:.1}", requests_per_second).magenta());
        println!("{}", format!("Total Test Time: {}ms", total_time.as_millis()).bright_black());

        // Show failed scenarios
        let failed_scenarios: Vec<&UserScenarioResult> = results.iter().filter(|r| !r.success).collect();
        if !failed_scenarios.is_empty() {
            println!("{}", "\n❌ Failed Scenarios:".red().bold());
            for scenario in failed_scenarios {
                let error_msg = scenario.error.as_deref().unwrap_or("Unknown error");
                println!("{}", format!("  {}: {}", scenario.user_id, error_msg).red());
            }
        }

        println!("{}", format!("\n{}", "═".repeat(50)).bright_black());
    }
}