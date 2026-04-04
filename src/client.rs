use defguard_wireguard_rs::{
    host::Peer, key::Key, net::IpAddrMask, InterfaceConfiguration, Kernel, WGApi,
    WireguardInterfaceApi,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

const SERVER_HTTP_URL: &str = "http://127.0.0.1:3000/connect";
const CLIENT_IFACE: &str = "wg-client";

#[derive(Serialize)]
struct HandshakeRequest {
    client_pub_key: String,
}
#[derive(Deserialize)]
struct HandshakeResponse {
    server_pub_key: String,
    client_assigned_ip: String,
    endpoint: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let private_key = Key::generate();
    let public_key = private_key.clone();
    let pub_key_str = public_key.to_string();
    println!("My Identity: {}", pub_key_str);

    println!("Requesting access from {}...", SERVER_HTTP_URL);
    let resp = reqwest::Client::new()
        .post(SERVER_HTTP_URL)
        .json(&HandshakeRequest {
            client_pub_key: pub_key_str,
        })
        .send()
        .await?
        .json::<HandshakeResponse>()
        .await?;

    println!("Got IP: {}. Configuring tunnel...", resp.client_assigned_ip);

    let wg_api = WGApi::<Kernel>::new(CLIENT_IFACE.to_string())?;

    let _ = wg_api.remove_interface();
    wg_api.create_interface()?;

    wg_api.configure_interface(&InterfaceConfiguration {
        name: CLIENT_IFACE.to_string(),
        prvkey: private_key.to_string(),
        addresses: vec![resp.client_assigned_ip.parse()?].clone(),
        port: 0,
        peers: vec![],
        mtu: None,
    })?;

    let mut server_peer = Peer::new(Key::from_str(resp.server_pub_key.as_str()).expect("asdasd"));
    server_peer
        .allowed_ips
        .push(IpAddrMask::new("0.0.0.0".parse()?, 0));
    server_peer.endpoint = Some(resp.endpoint.parse()?);
    wg_api.configure_peer(&server_peer)?;

    std::process::Command::new("ip")
        .args(["link", "set", "up", "dev", CLIENT_IFACE])
        .output()?;
    std::process::Command::new("ip")
        .args([
            "addr",
            "add",
            format!("{}/24", resp.client_assigned_ip).as_str(),
            "dev",
            CLIENT_IFACE,
        ])
        .output()?;
    std::process::Command::new("ip")
        .args(["route", "add", "10.50.0.0/24", "dev", CLIENT_IFACE])
        .output()?;

    println!("Connected! Ping 10.50.0.1 to test.");
    tokio::signal::ctrl_c().await?;
    Ok(())
}
