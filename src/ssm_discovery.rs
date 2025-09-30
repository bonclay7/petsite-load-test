use aws_config::BehaviorVersion;
use aws_sdk_ssm::Client;
use colored::*;
use std::collections::HashMap;

use crate::types::Endpoints;

pub struct SSMEndpointDiscovery {
    client: Client,
    parameter_prefixes: Vec<String>,
}

impl SSMEndpointDiscovery {
    pub async fn new(region: &str) -> anyhow::Result<Self> {
        let region = aws_config::Region::new(region.to_string());
        let config = aws_config::defaults(BehaviorVersion::latest())
            .region(region)
            .load()
            .await;
        
        let client = Client::new(&config);
        
        let parameter_prefixes = vec![
            "/petstore/petlistadoptionsurl".to_string(),
            "/petstore/searchapiurl".to_string(),
            "/petstore/paymentapiurl".to_string(),
            "/petstore/petfoodapiurl".to_string(),
            "/petstore/petfoodcarturl".to_string(),
        ];

        Ok(Self {
            client,
            parameter_prefixes,
        })
    }

    pub async fn discover_endpoints(&self) -> anyhow::Result<Endpoints> {
        println!("{}", "🔍 Discovering endpoints from SSM...".blue());

        let mut discovered = HashMap::new();

        for prefix in &self.parameter_prefixes {
            match self.client
                .get_parameter()
                .name(prefix)
                .with_decryption(true)
                .send()
                .await
            {
                Ok(result) => {
                    if let Some(parameter) = result.parameter {
                        if let Some(value) = parameter.value {
                            let service_name = prefix.split('/').nth(2).unwrap_or("unknown");
                            discovered.insert(service_name.to_string(), value.clone());
                            println!("{}", format!("✓ Found {}: {}", service_name, value).green());
                        }
                    }
                }
                Err(err) => {
                    if err.to_string().contains("ParameterNotFound") {
                        println!("{}", format!("⚠️  Parameter not found: {}", prefix).yellow());
                    } else {
                        println!("{}", format!("❌ Error fetching {}: {}", prefix, err).red());
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