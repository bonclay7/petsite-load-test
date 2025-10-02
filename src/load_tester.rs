use colored::*;
use futures::future::join_all;
use rand::Rng;
use reqwest::Client;
use serde_json;
use std::time::{Duration, Instant};
use tokio::time::timeout;

use crate::types::*;

pub struct LoadTester {
    user_count: usize,
    concurrent_requests: usize,
    endpoints: Endpoints,
    dry_run: bool,
    verbose: bool,
    rampup_seconds: u64,
    client: Client,
}

impl LoadTester {
    pub fn new(
        user_count: usize,
        concurrent_requests: usize,
        endpoints: Endpoints,
        dry_run: bool,
        verbose: bool,
        rampup_seconds: u64,
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
            rampup_seconds,
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
        let total_scenarios = self.user_count * self.concurrent_requests;

        if self.rampup_seconds > 0 {
            println!(
                "{}",
                format!(
                    "📈 Ramping up {} scenarios over {} seconds...",
                    total_scenarios, self.rampup_seconds
                ).cyan()
            );
            self.run_rampup_test(users).await
        } else {
            println!(
                "{}",
                format!("⚡ Running {} concurrent scenarios...", total_scenarios).yellow()
            );
            self.run_immediate_test(users).await
        }
    }

    async fn run_immediate_test(&self, users: Vec<String>) -> anyhow::Result<Vec<UserScenarioResult>> {
        let mut all_futures = Vec::new();

        // Create concurrent futures for all user scenarios
        for _ in 0..self.concurrent_requests {
            for user_id in &users {
                let future = self.run_scenario_for_user(user_id.clone());
                all_futures.push(future);
            }
        }

        let results = join_all(all_futures).await;
        Ok(results)
    }

    async fn run_rampup_test(&self, users: Vec<String>) -> anyhow::Result<Vec<UserScenarioResult>> {
        use tokio::time::{sleep, Duration, Instant};
        
        let total_scenarios = self.user_count * self.concurrent_requests;
        let rampup_interval = Duration::from_millis((self.rampup_seconds * 1000) / total_scenarios as u64);
        
        let mut all_futures = Vec::new();
        let mut scenario_count = 0;
        let start_time = Instant::now();

        println!(
            "{}",
            format!(
                "⏱️  Starting new scenario every {}ms",
                rampup_interval.as_millis()
            ).bright_black()
        );

        // Ramp up scenarios gradually
        for round in 0..self.concurrent_requests {
            for user_id in &users {
                scenario_count += 1;
                
                if self.verbose {
                    println!(
                        "{}",
                        format!(
                            "[RAMP-UP] Starting scenario {}/{} for {} ({}ms elapsed)",
                            scenario_count, total_scenarios, user_id, start_time.elapsed().as_millis()
                        ).bright_black()
                    );
                }

                // Start the scenario without spawning a task
                let future = self.run_scenario_for_user(user_id.clone());
                all_futures.push(future);

                // Sleep between scenario starts (except for the last one)
                if scenario_count < total_scenarios {
                    sleep(rampup_interval).await;
                }
            }
        }

        println!(
            "{}",
            format!("🚀 All {} scenarios started, waiting for completion...", total_scenarios).green()
        );

        // Wait for all scenarios to complete using join_all
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

        // Step 6: Check adoptions list (verify adoption was recorded)
        let adoptions_check_result = self
            .make_request(
                "GET",
                &self.endpoints.petlistadoptions,
                &user_id,
                None::<()>,
            )
            .await;
        requests.push(adoptions_check_result);

        // Step 7: Comprehensive Pet Food Testing
        let petfood_base = self.endpoints.petfood.replace("/api/foods", "");
        
        // 7.1: List all foods
        let food_list_result = self
            .make_request(
                "GET",
                &self.endpoints.petfood,
                &user_id,
                None::<()>,
            )
            .await;
        requests.push(food_list_result);

        // 7.2: Search foods with filters (random combinations)
        let pet_types_food = ["puppy", "kitten", "bunny"];
        let max_prices = ["10", "25", "50", "100"];
        let search_terms = ["royal", "premium", "organic", "chicken"];
        
        let random_pet_type_food = pet_types_food[rng.gen_range(0..pet_types_food.len())];
        let random_max_price = max_prices[rng.gen_range(0..max_prices.len())];
        let random_search = search_terms[rng.gen_range(0..search_terms.len())];

        // Filter by pet type and price
        let filter_url = format!("{}?pettype={}&max_price={}", self.endpoints.petfood, random_pet_type_food, random_max_price);
        let filter_result = self
            .make_request(
                "GET",
                &filter_url,
                &user_id,
                None::<()>,
            )
            .await;
        requests.push(filter_result);

        // Search by term
        let search_url = format!("{}?search={}", self.endpoints.petfood, random_search);
        let search_result = self
            .make_request(
                "GET",
                &search_url,
                &user_id,
                None::<()>,
            )
            .await;
        requests.push(search_result);

        // 7.3: Get specific food by ID (simulate getting a food ID from previous responses)
        let food_ids = ["F046a4eca", "Fecd30d31", "F36a222eb", "Fc7f447a1", "F233c473c", "Ffb5ef0e2"];
        let random_food_id = food_ids[rng.gen_range(0..food_ids.len())];
        let food_detail_url = format!("{}/{}", self.endpoints.petfood, random_food_id);
        let food_detail_result = self
            .make_request(
                "GET",
                &food_detail_url,
                &user_id,
                None::<()>,
            )
            .await;
        requests.push(food_detail_result);

        // 7.4: Cart operations
        // List current cart
        let cart_list_url = format!("{}/api/cart/{}", petfood_base, user_id);
        let cart_list_result = self
            .make_request(
                "GET",
                &cart_list_url,
                &user_id,
                None::<()>,
            )
            .await;
        requests.push(cart_list_result);

        // Add item to cart
        let add_to_cart_url = format!("{}/api/cart/{}/items", petfood_base, user_id);
        let add_cart_payload = serde_json::json!({
            "food_id": random_food_id,
            "quantity": rng.gen_range(1..5)
        });
        let add_cart_result = self
            .make_request(
                "POST",
                &add_to_cart_url,
                &user_id,
                Some(add_cart_payload),
            )
            .await;
        requests.push(add_cart_result);

        // Update item quantity in cart
        let update_cart_url = format!("{}/api/cart/{}/items/{}", petfood_base, user_id, random_food_id);
        let update_cart_payload = serde_json::json!({
            "quantity": rng.gen_range(1..10)
        });
        let update_cart_result = self
            .make_request(
                "PUT",
                &update_cart_url,
                &user_id,
                Some(update_cart_payload),
            )
            .await;
        requests.push(update_cart_result);

        // 7.5: Checkout process
        let checkout_url = format!("{}/api/cart/{}/checkout", petfood_base, user_id);
        let checkout_payload = serde_json::json!({
            "payment_method": {
                "CreditCard": {
                    "card_number": "4111111111111111",
                    "expiry_month": 12,
                    "expiry_year": 2025,
                    "cvv": "123",
                    "cardholder_name": format!("User {}", user_id)
                }
            },
            "shipping_address": {
                "name": format!("User {}", user_id),
                "street": "123 Main St",
                "city": "Seattle",
                "state": "WA",
                "zip_code": "98101",
                "country": "USA"
            },
            "billing_address": {
                "name": format!("User {}", user_id),
                "street": "123 Main St",
                "city": "Seattle",
                "state": "WA",
                "zip_code": "98101",
                "country": "USA"
            }
        });
        let checkout_result = self
            .make_request(
                "POST",
                &checkout_url,
                &user_id,
                Some(checkout_payload),
            )
            .await;
        requests.push(checkout_result);

        // Step 8: Cleanup operations
        // Empty the cart
        let empty_cart_url = format!("{}/api/cart/{}", petfood_base, user_id);
        let empty_cart_result = self
            .make_request(
                "DELETE",
                &empty_cart_url,
                &user_id,
                None::<()>,
            )
            .await;
        requests.push(empty_cart_result);

        // Clean up adoption (optional DELETE operations)
        let cleanup_adoption_url = self.endpoints.payforadoption.replace("/api/completeadoption", &format!("/api/adoption/{}", selected_pet_id));
        let cleanup_adoption_result = self
            .make_request(
                "DELETE",
                &cleanup_adoption_url,
                &user_id,
                None::<()>,
            )
            .await;
        requests.push(cleanup_adoption_result);

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
        
        if self.rampup_seconds > 0 {
            println!("{}", format!("Ramp-up Period: {}s", self.rampup_seconds).bright_black());
        }

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