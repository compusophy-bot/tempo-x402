//! Railway GraphQL API client.
//!
//! Wraps the Railway platform API (`https://backboard.railway.app/graphql/v2`)
//! for programmatic service creation, environment variable configuration,
//! Docker image deployment, and domain management.

use serde::{Deserialize, Serialize};

const RAILWAY_API_URL: &str = "https://backboard.railway.app/graphql/v2";

#[derive(Debug, thiserror::Error)]
pub enum RailwayError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("GraphQL error: {0}")]
    GraphQL(String),

    #[error("missing field in response: {0}")]
    MissingField(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct GraphQLRequest {
    query: String,
    variables: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct GraphQLResponse {
    data: Option<serde_json::Value>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLError {
    message: String,
}

/// Client for the Railway platform GraphQL API.
pub struct RailwayClient {
    http: reqwest::Client,
    token: String,
    project_id: String,
}

impl RailwayClient {
    pub fn new(token: String, project_id: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to create HTTP client");

        Self {
            http,
            token,
            project_id,
        }
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    /// Execute a GraphQL query/mutation against the Railway API.
    async fn execute(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<serde_json::Value, RailwayError> {
        let request = GraphQLRequest {
            query: query.to_string(),
            variables,
        };

        let response = self
            .http
            .post(RAILWAY_API_URL)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let gql_response: GraphQLResponse = response.json().await?;

        if let Some(errors) = gql_response.errors {
            let messages: Vec<String> = errors.into_iter().map(|e| e.message).collect();
            return Err(RailwayError::GraphQL(messages.join("; ")));
        }

        gql_response
            .data
            .ok_or_else(|| RailwayError::MissingField("data".to_string()))
    }

    /// Create a new service in the project.
    pub async fn create_service(&self, name: &str) -> Result<String, RailwayError> {
        let query = r#"
            mutation ServiceCreate($input: ServiceCreateInput!) {
                serviceCreate(input: $input) {
                    id
                    name
                }
            }
        "#;

        let variables = serde_json::json!({
            "input": {
                "projectId": self.project_id,
                "name": name,
            }
        });

        let data = self.execute(query, variables).await?;
        data["serviceCreate"]["id"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| RailwayError::MissingField("serviceCreate.id".to_string()))
    }

    /// Get the default environment ID for the project.
    pub async fn get_default_environment(&self) -> Result<String, RailwayError> {
        let query = r#"
            query Project($id: String!) {
                project(id: $id) {
                    environments {
                        edges {
                            node {
                                id
                                name
                            }
                        }
                    }
                }
            }
        "#;

        let variables = serde_json::json!({ "id": self.project_id });
        let data = self.execute(query, variables).await?;

        // Return the first environment (Railway projects have a default "production" env)
        data["project"]["environments"]["edges"]
            .as_array()
            .and_then(|edges| edges.first())
            .and_then(|edge| edge["node"]["id"].as_str())
            .map(String::from)
            .ok_or_else(|| RailwayError::MissingField("environment id".to_string()))
    }

    /// Set environment variables on a service.
    pub async fn set_variables(
        &self,
        service_id: &str,
        environment_id: &str,
        variables: serde_json::Value,
    ) -> Result<(), RailwayError> {
        let query = r#"
            mutation VariableCollectionUpsert($input: VariableCollectionUpsertInput!) {
                variableCollectionUpsert(input: $input)
            }
        "#;

        let input = serde_json::json!({
            "input": {
                "projectId": self.project_id,
                "serviceId": service_id,
                "environmentId": environment_id,
                "variables": variables,
            }
        });

        self.execute(query, input).await?;
        Ok(())
    }

    /// Set the Docker image source for a service.
    pub async fn set_docker_image(
        &self,
        service_id: &str,
        image: &str,
    ) -> Result<(), RailwayError> {
        let query = r#"
            mutation ServiceInstanceUpdate($input: ServiceInstanceUpdateInput!) {
                serviceInstanceUpdate(input: $input) {
                    id
                }
            }
        "#;

        let variables = serde_json::json!({
            "input": {
                "serviceId": service_id,
                "source": {
                    "image": image,
                }
            }
        });

        self.execute(query, variables).await?;
        Ok(())
    }

    /// Create a public domain for a service in an environment.
    pub async fn create_domain(
        &self,
        service_id: &str,
        environment_id: &str,
    ) -> Result<String, RailwayError> {
        let query = r#"
            mutation ServiceDomainCreate($input: ServiceDomainCreateInput!) {
                serviceDomainCreate(input: $input) {
                    id
                    domain
                }
            }
        "#;

        let variables = serde_json::json!({
            "input": {
                "serviceId": service_id,
                "environmentId": environment_id,
            }
        });

        let data = self.execute(query, variables).await?;
        data["serviceDomainCreate"]["domain"]
            .as_str()
            .map(|d| format!("https://{d}"))
            .ok_or_else(|| RailwayError::MissingField("serviceDomainCreate.domain".to_string()))
    }

    /// Add a persistent volume to a service.
    pub async fn add_volume(
        &self,
        service_id: &str,
        environment_id: &str,
        mount_path: &str,
    ) -> Result<String, RailwayError> {
        let query = r#"
            mutation VolumeCreate($input: VolumeCreateInput!) {
                volumeCreate(input: $input) {
                    id
                }
            }
        "#;

        let variables = serde_json::json!({
            "input": {
                "projectId": self.project_id,
                "serviceId": service_id,
                "environmentId": environment_id,
                "mountPath": mount_path,
            }
        });

        let data = self.execute(query, variables).await?;
        data["volumeCreate"]["id"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| RailwayError::MissingField("volumeCreate.id".to_string()))
    }

    /// Trigger a deployment for a service in an environment.
    pub async fn deploy_service(
        &self,
        service_id: &str,
        environment_id: &str,
    ) -> Result<String, RailwayError> {
        let query = r#"
            mutation DeploymentCreate($input: DeploymentCreateInput!) {
                deploymentCreate(input: $input) {
                    id
                    status
                }
            }
        "#;

        let variables = serde_json::json!({
            "input": {
                "serviceId": service_id,
                "environmentId": environment_id,
            }
        });

        let data = self.execute(query, variables).await?;
        data["deploymentCreate"]["id"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| RailwayError::MissingField("deploymentCreate.id".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_railway_client_creation() {
        let client = RailwayClient::new("test-token".to_string(), "test-project".to_string());
        assert_eq!(client.project_id(), "test-project");
    }
}
