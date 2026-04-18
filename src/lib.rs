use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct HandshakeRequest {
    pub client_pub_key: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct HandshakeResponse {
    pub server_pub_key: String,
    pub client_assigned_ip: String,
    pub endpoint: String,
}
