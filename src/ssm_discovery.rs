use aws_config::BehaviorVersion;
use aws_sdk_ssm::Client;
use colored::*;
use std::collections::HashMap;

use crate::types::Endpoints;

pub struct SSMEndpointDiscovery {
    client: Client,
    service_parameters: HashMap<String, String>,
}

impl SSMEndpointDiscovery {
    pub async fn new(region: &str) -> anyhow::Result<Self> {
        let region = aws_config::Region::new(region.to_string());
        let config = aws_config::defaults(BehaviorVersion::latest())
            .region(region)
            .load()
            .await;
        
        let client = Client::new(&config);
        
        // Map service names to their SSM parameter paths
        let mut service_parameters = HashMap::new();
        service_parameters.insert("petlistadoptions".to_string(), "/petstore/petlistadoptionsurl".to_string());
        service_parameters.insert("petsearch".to_string(), "/petstore/searchapiurl".to_string());
        service_parameters.insert("payforadoption".to_string(), "/petstore/paymentapiurl".to_string());
        service_parameters.insert("petfood".to_string(), "/petstore/petfoodapiurl".to_string());
        // Note: petfoodcarturl might be used for cart-specific operations if needed
        service_parameters.insert("petfoodcart".to_string(), "/petstore/petfoodcarturl".to_string());

        Ok(Self {
            client,
            service_parameters,
        })
    }

    /// Create a new SSMEndpointDiscovery with custom service parameter mappings
    pub async fn with_custom_parameters(region: &str, service_parameters: HashMap<String, String>) -> anyhow::Result<Self> {
        let region = aws_config::Region::new(region.to_string());
        let config = aws_config::defaults(BehaviorVersion::latest())
            .region(region)
            .load()
            .await;
        
        let client = Client::new(&config);

        Ok(Self {
            client,
            service_parameters,
        })
    }

    /// Get the current service parameter mappings
    pub fn get_service_parameters(&self) -> &HashMap<String, String> {
        &self.service_parameters
    }

    pub async fn discover_endpoints(&self) -> anyhow::Result<Endpoints> {
        println!("{}", "🔍 Discovering endpoints from SSM...".blue());

        let mut discovered = HashMap::new();

        // Iterate through service name -> SSM parameter mappings
        for (service_name, parameter_path) in &self.service_parameters {
            match self.client
                .get_parameter()
                .name(parameter_path)
                .with_decryption(true)
                .send()
                .await
            {
                Ok(result) => {
                    if let Some(parameter) = result.parameter {
                        if let Some(value) = parameter.value {
                            discovered.insert(service_name.clone(), value.clone());
                            println!("{}", format!("✓ Found {}: {}", service_name, value).green());
                        }
                    }
                }
                Err(err) => {
                    if err.to_string().contains("ParameterNotFound") {
                        println!("{}", format!("⚠️  Parameter not found: {} ({})", service_name, parameter_path).yellow());
                    } else {
                        println!("{}", format!("❌ Error fetching {} ({}): {}", service_name, parameter_path, err).red());
                    }
                }
            }
        }

        // Create endpoints with discovered values or fallbacks
        let mut endpoints = Endpoints::new();

        if let Some(endpoint) = discovered.get("petlistadoptions") {
            endpoints.petlistadoptions = endpoint.clone();
        } else {
            println!("{}", format!("🔄 Using fallback for petlistadoptions: {}", endpoints.petlistadoptions).cyan());
        }

        if let Some(endpoint) = discovered.get("petsearch") {
            endpoints.petsearch = endpoint.clone();
        } else {
            println!("{}", format!("🔄 Using fallback for petsearch: {}", endpoints.petsearch).cyan());
        }

        if let Some(endpoint) = discovered.get("payforadoption") {
            endpoints.payforadoption = endpoint.clone();
        } else {
            println!("{}", format!("🔄 Using fallback for payforadoption: {}", endpoints.payforadoption).cyan());
        }

        if let Some(endpoint) = discovered.get("petfood") {
            endpoints.petfood = endpoint.clone();
        } else {
            println!("{}", format!("🔄 Using fallback for petfood: {}", endpoints.petfood).cyan());
        }

        Ok(endpoints)
    }
}