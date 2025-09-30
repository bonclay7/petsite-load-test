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
    verbose: bool,
    client: Client,
}

impl LoadTester {
    pub fn new(
        user_count: usize,
        concurrent_requests: usize,
        endpoints: Endpoints,
        dry_run: bool,
        verbose: bool,
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
            verbose,
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

        // Step 1: List all pets via petsearch
        let list_all_pets_result = self
            .make_request(
                "GET",
                &self.endpoints.petsearch,
                &user_id,
                None::<()>,
            )
            .await;
        requests.push(list_all_pets_result.clone());

        // Step 2: Filter by color (random selection)
        let colors = ["black", "brown", "white", "red", "blue"]; // Include some invalid colors
        let mut rng = rand::thread_rng();
        let random_color = colors[rng.gen_range(0..colors.len())];
        let color_search_url = if self.endpoints.petsearch.ends_with('?') {
            format!("{}petcolor={}", self.endpoints.petsearch, random_color)
        } else {
            format!("{}?petcolor={}", self.endpoints.petsearch, random_color)
        };
        let color_search_result = self
            .make_request(
                "GET",
                &color_search_url,
                &user_id,
                None::<()>,
            )
            .await;
        requests.push(color_search_result);

        // Step 3: Filter by pet type (random selection)
        let pet_types = ["puppy", "kitten", "bunny"];
        let random_pet_type = pet_types[rng.gen_range(0..pet_types.len())];
        let type_search_url = if self.endpoints.petsearch.ends_with('?') {
            format!("{}pettype={}", self.endpoints.petsearch, random_pet_type)
        } else {
            format!("{}?pettype={}", self.endpoints.petsearch, random_pet_type)
        };
        let type_search_result = self
            .make_request(
                "GET",
                &type_search_url,
                &user_id,
                None::<()>,
            )
            .await;
        requests.push(type_search_result);

        // Step 4: Parse pets from the first successful search and select one for adoption
        let (selected_pet_id, selected_pet_type) = if list_all_pets_result.success {
            // Try to parse the response to get pets
            if let Ok(response) = self.client.get(&self.endpoints.petsearch).send().await {
                if let Ok(text) = response.text().await {
                    if let Ok(pets) = serde_json::from_str::<PetListResponse>(&text) {
                        if !pets.is_empty() {
                            let random_pet = &pets[rng.gen_range(0..pets.len())];
                            (
                                random_pet.petid.clone(),
                                random_pet.pettype.clone().unwrap_or_else(|| random_pet_type.to_string())
                            )
                        } else {
                            (format!("pet{:03}", rng.gen_range(1..1000)), random_pet_type.to_string())
                        }
                    } else {
                        (format!("pet{:03}", rng.gen_range(1..1000)), random_pet_type.to_string())
                    }
                } else {
                    (format!("pet{:03}", rng.gen_range(1..1000)), random_pet_type.to_string())
                }
            } else {
                (format!("pet{:03}", rng.gen_range(1..1000)), random_pet_type.to_string())
            }
        } else {
            success = false;
            (format!("pet{:03}", rng.gen_range(1..1000)), random_pet_type.to_string())
        };

        // Step 5: Pay for adoption using query parameters
        let adoption_url = format!(
            "{}?petId={}&petType={}&userId={}",
            self.endpoints.payforadoption,
            selected_pet_id,
            selected_pet_type,
            user_id
        );
        let adoption_result = self
            .make_request(
                "POST",
                &adoption_url,
                &user_id,
                None::<()>,
            )
            .await;
        requests.push(adoption_result);

        // Step 6: Pet food operations
        // List all food
        let food_list_result = self
            .make_request(
                "GET",
                &self.endpoints.petfood,
                &user_id,
                None::<()>,
            )
            .await;
        requests.push(food_list_result);

        // Add food to cart - use base petfood URL with /cart path
        let add_food_payload = FoodCartPayload {
            user_id: user_id.clone(),
            food_id: "food001".to_string(),
            quantity: 2,
        };
        let cart_url = self.endpoints.petfood.replace("/api/foods", "/api/cart");
        let add_food_result = self
            .make_request(
                "POST",
                &cart_url,
                &user_id,
                Some(add_food_payload),
            )
            .await;
        requests.push(add_food_result);

        // Pay for food - use base petfood URL with /pay path
        let pay_food_payload = PayFoodPayload {
            user_id: user_id.clone(),
            cart_total: 25.99,
        };
        let pay_url = self.endpoints.petfood.replace("/api/foods", "/api/pay");
        let pay_food_result = self
            .make_request(
                "POST",
                &pay_url,
                &user_id,
                Some(pay_food_payload),
            )
            .await;
        requests.push(pay_food_result);

        // Step 7: Cleanup (optional DELETE operations)
        let cleanup_url = self.endpoints.payforadoption.replace("/api/completeadoption", &format!("/api/adoption/{}", selected_pet_id));
        let cleanup_result = self
            .make_request(
                "DELETE",
                &cleanup_url,
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

        let result = match timeout(Duration::from_secs(10), request_future).await {
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
        };

        // Verbose logging
        if self.verbose {
            if result.success {
                println!("{}", format!("[{}] {} {} - {} ({}ms)", 
                    user_id, method, url, result.status, result.response_time.as_millis()).green());
            } else {
                println!("{}", format!("[{}] {} {} - FAILED: {} ({}ms)", 
                    user_id, method, url, 
                    result.error.as_deref().unwrap_or("Unknown error"), 
                    result.response_time.as_millis()).red());
            }
        }

        result
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

        // Show detailed failed request information
        let failed_requests: Vec<&RequestResult> = all_requests.iter().filter(|r| !r.success).copied().collect();
        if !failed_requests.is_empty() {
            println!("{}", "\n❌ Failed Requests Details:".red().bold());
            println!("{}", "─".repeat(80).bright_black());
            
            for (i, request) in failed_requests.iter().enumerate() {
                println!("{}", format!("{}. {} {} ({})", 
                    i + 1, 
                    request.method, 
                    request.url, 
                    request.user_id
                ).red());
                
                if request.status > 0 {
                    println!("{}", format!("   Status: {}", request.status).yellow());
                }
                
                if let Some(error) = &request.error {
                    println!("{}", format!("   Error: {}", error).red());
                }
                
                println!("{}", format!("   Response Time: {}ms", request.response_time.as_millis()).cyan());
                println!();
            }
        }

        // Show failed scenarios summary
        let failed_scenarios: Vec<&UserScenarioResult> = results.iter().filter(|r| !r.success).collect();
        if !failed_scenarios.is_empty() {
            println!("{}", "📋 Failed Scenarios Summary:".red().bold());
            for scenario in failed_scenarios {
                let failed_count = scenario.requests.iter().filter(|r| !r.success).count();
                let total_count = scenario.requests.len();
                println!("{}", format!("  {}: {}/{} requests failed", 
                    scenario.user_id, 
                    failed_count, 
                    total_count
                ).red());
            }
        }

        // Show request breakdown by endpoint
        println!("{}", "\n📈 Request Breakdown by Endpoint:".blue().bold());
        println!("{}", "─".repeat(80).bright_black());
        
        let mut endpoint_stats = std::collections::HashMap::new();
        for request in &all_requests {
            let endpoint = request.url.split('?').next().unwrap_or(&request.url);
            let stats = endpoint_stats.entry(endpoint.to_string()).or_insert((0, 0));
            if request.success {
                stats.0 += 1;
            } else {
                stats.1 += 1;
            }
        }
        
        for (endpoint, (success, failed)) in endpoint_stats {
            let total = success + failed;
            let success_rate = if total > 0 { (success as f64 / total as f64) * 100.0 } else { 0.0 };
            let status_color = if success_rate >= 90.0 { "green" } else if success_rate >= 70.0 { "yellow" } else { "red" };
            
            match status_color {
                "green" => println!("{}", format!("  ✓ {}: {}/{} ({:.1}%)", endpoint, success, total, success_rate).green()),
                "yellow" => println!("{}", format!("  ⚠ {}: {}/{} ({:.1}%)", endpoint, success, total, success_rate).yellow()),
                "red" => println!("{}", format!("  ✗ {}: {}/{} ({:.1}%)", endpoint, success, total, success_rate).red()),
                _ => println!("{}", format!("  • {}: {}/{} ({:.1}%)", endpoint, success, total, success_rate)),
            }
        }

        println!("{}", format!("\n{}", "═".repeat(50)).bright_black());
    }
}