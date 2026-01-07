/// Client API REST pour communiquer avec le serveur RMDB
/// Permet au TUI d'interagir avec l'API HTTP du serveur

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration du client API
pub struct APIClient {
    base_url: String,
    auth_token: Option<String>,
}

/// Structure pour une VM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VM {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub format: String,
    pub disk_path: String,
    pub size: u64,
    pub compressed: bool,
    pub created_at: String,
    pub updated_at: String,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Structure pour une catégorie de VM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VMCategory {
    pub name: String,
    pub description: String,
}

/// Structure pour un lease DHCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DHCPLease {
    pub mac: String,
    pub ip: String,
    pub hostname: Option<String>,
    pub expires_at: Option<String>,
    pub state: String,
}

/// Structure pour un client connecté
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedClient {
    pub mac: String,
    pub ip: String,
    pub hostname: Option<String>,
    pub connected_at: String,
    pub last_seen: String,
}

/// Réponse de l'API pour les leases DHCP
#[derive(Debug, Deserialize)]
pub struct DHCPLeasesResponse {
    pub leases: Vec<DHCPLease>,
    pub count: usize,
}

/// Réponse de l'API pour les clients connectés
#[derive(Debug, Deserialize)]
pub struct ConnectedClientsResponse {
    pub clients: Vec<ConnectedClient>,
    pub count: usize,
}

/// Métriques système
#[derive(Debug, Deserialize)]
pub struct SystemMetrics {
    pub cpu: CPUMetrics,
    pub memory: MemoryMetrics,
    pub disk: DiskMetrics,
    pub network: NetworkMetrics,
}

#[derive(Debug, Deserialize)]
pub struct CPUMetrics {
    pub usage_percent: f64,
    pub cores: usize,
    pub load_average: Vec<f64>,
}

#[derive(Debug, Deserialize)]
pub struct MemoryMetrics {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub usage_percent: f64,
}

#[derive(Debug, Deserialize)]
pub struct DiskMetrics {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub usage_percent: f64,
}

#[derive(Debug, Deserialize)]
pub struct NetworkMetrics {
    pub interfaces: Vec<NetworkInterface>,
}

#[derive(Debug, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
}

/// Structure pour une entrée iPXE
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IPXEEntry {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub menu_type: String,
    pub boot_target: Option<String>,
    pub enabled: bool,
}

/// Structure pour un overlay de VM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VMOverlay {
    pub id: String,
    pub vm_id: String,
    pub mac_address: String,
    pub overlay_path: String,
    pub size: u64,
    pub created_at: String,
    pub updated_at: String,
}

/// Réponse de l'API pour les overlays
#[derive(Debug, Deserialize)]
pub struct OverlaysResponse {
    pub overlays: Vec<VMOverlay>,
    pub count: usize,
}

/// Réponse de l'API pour la liste des VMs
#[derive(Debug, Deserialize)]
pub struct VMListResponse {
    pub vms: Vec<VM>,
    pub count: usize,
}

/// Erreur API
#[derive(Debug)]
pub enum APIError {
    NetworkError(String),
    AuthError(String),
    NotFound(String),
    ServerError(String),
    ParseError(String),
}

impl APIClient {
    /// Crée un nouveau client API
    pub fn new(base_url: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_token: None,
        }
    }

    /// Configure le token d'authentification
    pub fn with_auth_token(mut self, token: String) -> Self {
        self.auth_token = Some(token);
        self
    }

    /// Effectue une requête GET
    fn get(&self, endpoint: &str) -> Result<String, APIError> {
        let url = format!("{}{}", self.base_url, endpoint);
        
        // Utiliser ureq pour les requêtes HTTP
        let mut request = ureq::get(&url);
        
        if let Some(ref token) = self.auth_token {
            request = request.set("Authorization", &format!("Bearer {}", token));
        }

        match request.call() {
            Ok(response) => {
                response.into_string()
                    .map_err(|e| APIError::ParseError(format!("Failed to read response: {}", e)))
            }
            Err(ureq::Error::Status(code, response)) => {
                let error_msg = response.into_string().unwrap_or_else(|_| format!("HTTP {}", code));
                if code == 404 {
                    Err(APIError::NotFound(error_msg))
                } else if code == 401 || code == 403 {
                    Err(APIError::AuthError(error_msg))
                } else {
                    Err(APIError::ServerError(error_msg))
                }
            }
            Err(e) => {
                Err(APIError::NetworkError(format!("Network error: {}", e)))
            }
        }
    }

    /// Effectue une requête POST
    fn post(&self, endpoint: &str, body: &str) -> Result<String, APIError> {
        let url = format!("{}{}", self.base_url, endpoint);
        
        let mut request = ureq::post(&url)
            .set("Content-Type", "application/json");
        
        if let Some(ref token) = self.auth_token {
            request = request.set("Authorization", &format!("Bearer {}", token));
        }

        match request.send_string(body) {
            Ok(response) => {
                response.into_string()
                    .map_err(|e| APIError::ParseError(format!("Failed to read response: {}", e)))
            }
            Err(ureq::Error::Status(code, response)) => {
                let error_msg = response.into_string().unwrap_or_else(|_| format!("HTTP {}", code));
                if code == 404 {
                    Err(APIError::NotFound(error_msg))
                } else if code == 401 || code == 403 {
                    Err(APIError::AuthError(error_msg))
                } else {
                    Err(APIError::ServerError(error_msg))
                }
            }
            Err(e) => {
                Err(APIError::NetworkError(format!("Network error: {}", e)))
            }
        }
    }

    /// Effectue une requête DELETE
    fn delete(&self, endpoint: &str) -> Result<(), APIError> {
        let url = format!("{}{}", self.base_url, endpoint);
        
        let mut request = ureq::delete(&url);
        
        if let Some(ref token) = self.auth_token {
            request = request.set("Authorization", &format!("Bearer {}", token));
        }

        match request.call() {
            Ok(_) => Ok(()),
            Err(ureq::Error::Status(code, response)) => {
                let error_msg = response.into_string().unwrap_or_else(|_| format!("HTTP {}", code));
                if code == 404 {
                    Err(APIError::NotFound(error_msg))
                } else if code == 401 || code == 403 {
                    Err(APIError::AuthError(error_msg))
                } else {
                    Err(APIError::ServerError(error_msg))
                }
            }
            Err(e) => {
                Err(APIError::NetworkError(format!("Network error: {}", e)))
            }
        }
    }

    /// Récupère la liste des VMs
    pub fn get_vms(&self, category: Option<&str>) -> Result<Vec<VM>, APIError> {
        let endpoint = if let Some(cat) = category {
            format!("/api/vms?category={}", cat)
        } else {
            "/api/vms".to_string()
        };

        let response = self.get(&endpoint)?;
        let vm_response: VMListResponse = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse VM list: {}", e)))?;

        Ok(vm_response.vms)
    }

    /// Récupère une VM par ID
    pub fn get_vm(&self, id: &str) -> Result<VM, APIError> {
        let endpoint = format!("/api/vms/{}", id);
        let response = self.get(&endpoint)?;
        
        let vm: VM = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse VM: {}", e)))?;

        Ok(vm)
    }

    /// Crée une nouvelle VM
    pub fn create_vm(&self, name: &str, description: &str, category: &str, format: &str, disk_path: &str) -> Result<VM, APIError> {
        let body = serde_json::json!({
            "name": name,
            "description": description,
            "category": category,
            "format": format,
            "disk_path": disk_path
        });

        let response = self.post("/api/vms", &body.to_string())?;
        let vm: VM = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse created VM: {}", e)))?;

        Ok(vm)
    }

    /// Supprime une VM
    pub fn delete_vm(&self, id: &str) -> Result<(), APIError> {
        let endpoint = format!("/api/vms/{}", id);
        self.delete(&endpoint)
    }

    /// Met à jour une VM
    pub fn update_vm(&self, id: &str, updates: &serde_json::Value) -> Result<VM, APIError> {
        let endpoint = format!("/api/vms/{}", id);
        let body = updates.to_string();
        let response = self.post(&endpoint, &body)?;
        
        let vm: VM = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse updated VM: {}", e)))?;

        Ok(vm)
    }

    /// Compresse une VM
    pub fn compress_vm(&self, id: &str) -> Result<(), APIError> {
        let endpoint = format!("/api/vms/{}/compress", id);
        self.post(&endpoint, "{}")?;
        Ok(())
    }

    /// Décompresse une VM
    pub fn decompress_vm(&self, id: &str, output_path: &str) -> Result<(), APIError> {
        let endpoint = format!("/api/vms/{}/decompress", id);
        let body = serde_json::json!({
            "output_path": output_path
        });
        self.post(&endpoint, &body.to_string())?;
        Ok(())
    }

    /// Récupère les catégories de VMs
    pub fn get_vm_categories(&self) -> Result<Vec<VMCategory>, APIError> {
        let response = self.get("/api/vms/categories")?;
        let categories: Vec<VMCategory> = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse categories: {}", e)))?;

        Ok(categories)
    }

    /// Récupère les leases DHCP
    pub fn get_dhcp_leases(&self) -> Result<Vec<DHCPLease>, APIError> {
        let response = self.get("/api/dhcp/leases")?;
        let leases_response: DHCPLeasesResponse = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse DHCP leases: {}", e)))?;

        Ok(leases_response.leases)
    }

    /// Récupère les clients connectés
    pub fn get_connected_clients(&self) -> Result<Vec<ConnectedClient>, APIError> {
        let response = self.get("/api/clients/connected")?;
        let clients_response: ConnectedClientsResponse = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse connected clients: {}", e)))?;

        Ok(clients_response.clients)
    }

    /// Récupère les métriques système
    pub fn get_system_metrics(&self) -> Result<SystemMetrics, APIError> {
        let response = self.get("/api/system/metrics")?;
        let metrics: SystemMetrics = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse system metrics: {}", e)))?;

        Ok(metrics)
    }

    /// Récupère le menu iPXE généré
    pub fn get_ipxe_menu(&self) -> Result<String, APIError> {
        let response = self.get("/api/ipxe/menu")?;
        Ok(response)
    }

    /// Récupère les entrées iPXE
    pub fn get_ipxe_entries(&self) -> Result<Vec<IPXEEntry>, APIError> {
        let response = self.get("/api/ipxe/entries")?;
        let entries: Vec<IPXEEntry> = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse iPXE entries: {}", e)))?;

        Ok(entries)
    }

    /// Génère le menu iPXE
    pub fn generate_ipxe_menu(&self) -> Result<String, APIError> {
        let response = self.post("/api/ipxe/generate", "{}")?;
        Ok(response)
    }

    /// Authentifie l'utilisateur et retourne le token de session
    pub fn login(&mut self, username: &str, password: &str) -> Result<String, APIError> {
        let body = serde_json::json!({
            "username": username,
            "password": password
        });

        let response = self.post("/api/login", &body.to_string())?;
        let login_response: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse login response: {}", e)))?;

        if let Some(token) = login_response.get("token").and_then(|t| t.as_str()) {
            self.auth_token = Some(token.to_string());
            Ok(token.to_string())
        } else {
            Err(APIError::AuthError("No token in login response".to_string()))
        }
    }

    /// Récupère tous les overlays
    pub fn get_overlays(&self) -> Result<Vec<VMOverlay>, APIError> {
        let response = self.get("/api/overlays")?;
        let overlays_response: OverlaysResponse = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse overlays: {}", e)))?;

        Ok(overlays_response.overlays)
    }

    /// Récupère un overlay par MAC address
    pub fn get_overlay_by_mac(&self, mac: &str) -> Result<VMOverlay, APIError> {
        let endpoint = format!("/api/overlays/mac/{}", mac);
        let response = self.get(&endpoint)?;
        let overlay: VMOverlay = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse overlay: {}", e)))?;

        Ok(overlay)
    }

    /// Crée un overlay
    pub fn create_overlay(&self, vm_id: &str, mac_address: &str) -> Result<VMOverlay, APIError> {
        let body = serde_json::json!({
            "vm_id": vm_id,
            "mac_address": mac_address
        });
        let response = self.post("/api/overlays", &body.to_string())?;
        let overlay: VMOverlay = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse created overlay: {}", e)))?;

        Ok(overlay)
    }

    /// Supprime un overlay
    pub fn delete_overlay(&self, id: &str) -> Result<(), APIError> {
        let endpoint = format!("/api/overlays/{}", id);
        self.delete(&endpoint)
    }

    /// Récupère la configuration
    pub fn get_config(&self) -> Result<serde_json::Value, APIError> {
        let response = self.get("/api/config")?;
        let config: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse config: {}", e)))?;

        Ok(config)
    }

    /// Met à jour la configuration
    pub fn update_config(&self, config: &serde_json::Value) -> Result<serde_json::Value, APIError> {
        let body = config.to_string();
        let response = self.post("/api/config", &body)?;
        let updated_config: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse updated config: {}", e)))?;

        Ok(updated_config)
    }

    /// Exécute une réparation
    pub fn run_repair(&self, repair_type: &str) -> Result<RepairResult, APIError> {
        let body = serde_json::json!({
            "type": repair_type
        });
        let response = self.post("/api/repair", &body.to_string())?;
        let result: RepairResult = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse repair result: {}", e)))?;

        Ok(result)
    }

    /// Récupère les problèmes détectés
    pub fn get_repair_problems(&self) -> Result<Vec<RepairProblem>, APIError> {
        let response = self.get("/api/repair/problems")?;
        let problems: Vec<RepairProblem> = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse repair problems: {}", e)))?;

        Ok(problems)
    }

    /// Exécute un test
    pub fn run_test(&self, test_type: &str) -> Result<TestResult, APIError> {
        let body = serde_json::json!({
            "type": test_type
        });
        let response = self.post("/api/test", &body.to_string())?;
        let result: TestResult = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse test result: {}", e)))?;

        Ok(result)
    }

    /// Récupère les métriques de sécurité
    pub fn get_security_metrics(&self) -> Result<SecurityMetrics, APIError> {
        let response = self.get("/api/security/metrics")?;
        let metrics: SecurityMetrics = serde_json::from_str(&response)
            .map_err(|e| APIError::ParseError(format!("Failed to parse security metrics: {}", e)))?;

        Ok(metrics)
    }
}

/// Structure pour un résultat de réparation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairResult {
    pub success: bool,
    pub message: String,
    pub details: Option<String>,
}

/// Structure pour un problème de réparation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairProblem {
    pub id: String,
    pub severity: String,
    pub description: String,
    pub category: String,
}

/// Structure pour un résultat de test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub success: bool,
    pub message: String,
    pub details: Option<String>,
    pub duration_ms: Option<u64>,
}

/// Structure pour les métriques de sécurité
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityMetrics {
    pub threats_detected: u64,
    pub active_threats: u64,
    pub blocked_ips: u64,
    pub failed_logins: u64,
}

impl std::fmt::Display for APIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            APIError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            APIError::AuthError(msg) => write!(f, "Authentication error: {}", msg),
            APIError::NotFound(msg) => write!(f, "Not found: {}", msg),
            APIError::ServerError(msg) => write!(f, "Server error: {}", msg),
            APIError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

