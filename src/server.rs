use axum::{extract::State, routing::post, Json, Router};
use defguard_wireguard_rs::{
    host::Peer, key::Key, net::IpAddrMask, InterfaceConfiguration, WGApi, WireguardInterfaceApi,
};
use serde::{Deserialize, Serialize};
use std::{
    str::FromStr,
    sync::{Arc, Mutex},
};

const WG_IFACE: &str = "wg0";
const SERVER_PORT: u16 = 51820;
const SERVER_VPN_IP: &str = "10.50.0.1";
const PUBLIC_ENDPOINT: &str = "127.0.0.1:51820";

struct AppState {
    wg_api: Mutex<WGApi>,
    server_pub_key: String,
    next_client_ip_octet: Mutex<u8>,
}

#[derive(Deserialize, Debug)]
struct HandshakeRequest {
    client_pub_key: String,
}

#[derive(Serialize, Debug)]
struct HandshakeResponse {
    server_pub_key: String,
    client_assigned_ip: String,
    endpoint: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let wg_api = WGApi::new(WG_IFACE.to_string())?;
    let _ = wg_api.remove_interface();
    wg_api.create_interface()?;

    let server_priv = Key::generate();
    let server_pub = server_priv.clone();

    let interface_config = InterfaceConfiguration {
        name: WG_IFACE.to_string(),
        prvkey: server_priv.to_string(),
        addresses: vec![SERVER_VPN_IP.parse()?],
        port: SERVER_PORT as u32,
        peers: vec![],
        mtu: None,
    };
    wg_api.configure_interface(&interface_config)?;

    std::process::Command::new("ip")
        .args(["link", "set", "up", "dev", WG_IFACE])
        .output()?;
    std::process::Command::new("ip")
        .args([
            "addr",
            "add",
            format!("{}/24", SERVER_VPN_IP).as_str(),
            "dev",
            WG_IFACE,
        ])
        .output()?;

    println!("VPN Server UP at {} ({})", PUBLIC_ENDPOINT, server_pub);

    let state = Arc::new(AppState {
        wg_api: Mutex::new(wg_api),
        server_pub_key: server_pub.to_string(),
        next_client_ip_octet: Mutex::new(2),
    });

    let app = Router::new()
        .route("/connect", post(handle_connect))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_connect(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<HandshakeRequest>,
) -> Json<HandshakeResponse> {
    let mut octet = state.next_client_ip_octet.lock().unwrap();
    let assigned_ip_str = format!("10.50.0.{}", *octet);
    *octet += 1;

    println!("{:#?}", payload);

    let client_key = Key::from_str(&payload.client_pub_key).expect("failed to parse base64 key");
    let mut peer = Peer::new(client_key);
    peer.allowed_ips
        .push(IpAddrMask::new(assigned_ip_str.parse().unwrap(), 32));

    state.wg_api.lock().unwrap().configure_peer(&peer).unwrap();

    println!(
        "Authorized client {} at {}",
        payload.client_pub_key, assigned_ip_str
    );

    Json(HandshakeResponse {
        server_pub_key: state.server_pub_key.clone(),
        client_assigned_ip: assigned_ip_str,
        endpoint: PUBLIC_ENDPOINT.to_string(),
    })
}
