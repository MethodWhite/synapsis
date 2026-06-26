use crate::presentation::mcp::McpServer;
use quinn::{Endpoint, EndpointConfig, ServerConfig, TransportConfig};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

pub struct QuicTransport {
    server: Arc<McpServer>,
}

impl QuicTransport {
    pub fn new(server: Arc<McpServer>) -> Self {
        Self { server }
    }

    pub fn start(&self, port: u16) {
        let bind_addr =
            std::env::var("SYNAPSIS_QUIC_BIND").unwrap_or_else(|_| "127.0.0.1".to_string());
        let addr: SocketAddr = format!("{}:{}", bind_addr, port)
            .parse()
            .expect("Invalid bind address");
        let server = self.server.clone();

        let (cert_der, key_der) = generate_self_signed_cert();

        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async move {
            let mut transport = TransportConfig::default();
            transport.max_idle_timeout(Some(Duration::from_secs(60).try_into().unwrap()));
            transport.keep_alive_interval(Some(Duration::from_secs(5)));

            let crypto = rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(
                    vec![rustls::pki_types::CertificateDer::from(cert_der)],
                    rustls::pki_types::PrivateKeyDer::try_from(key_der)
                        .expect("Invalid private key"),
                )
                .expect("Failed to build TLS config");

            let quic_config = quinn::crypto::rustls::QuicServerConfig::try_from(crypto)
                .expect("Failed to build QUIC config");

            let mut server_config = ServerConfig::with_crypto(Arc::new(quic_config));
            server_config.transport_config(Arc::new(transport));

            let socket = std::net::UdpSocket::bind(addr).expect("Failed to bind QUIC socket");
            let endpoint = Endpoint::new(
                EndpointConfig::default(),
                Some(server_config),
                socket,
                Arc::new(quinn::TokioRuntime),
            )
            .expect("Failed to create QUIC endpoint");

            eprintln!(
                "[Synapsis MCP] QUIC server listening on {} (self-signed cert)",
                addr
            );

            while let Some(connecting) = endpoint.accept().await {
                let srv = server.clone();
                tokio::spawn(async move {
                    match connecting.await {
                        Ok(connection) => {
                            handle_connection(connection, srv).await;
                        }
                        Err(e) => eprintln!("[Synapsis MCP] QUIC connection error: {}", e),
                    }
                });
            }
        });
    }
}

fn generate_self_signed_cert() -> (Vec<u8>, Vec<u8>) {
    let key_pair = rcgen::KeyPair::generate().expect("Failed to generate key pair");
    let cert_params =
        rcgen::CertificateParams::new(vec!["synapsis.local".to_string(), "127.0.0.1".to_string()])
            .expect("Invalid cert params");
    let cert = cert_params
        .self_signed(&key_pair)
        .expect("Failed to self-sign cert");
    (cert.der().to_vec(), key_pair.serialize_der())
}

async fn handle_connection(connection: quinn::Connection, server: Arc<McpServer>) {
    loop {
        match connection.accept_bi().await {
            Ok((mut send, mut recv)) => {
                let srv = server.clone();
                tokio::spawn(async move {
                    let mut len_buf = [0u8; 4];
                    if recv.read_exact(&mut len_buf).await.is_err() {
                        return;
                    }
                    let msg_len = u32::from_be_bytes(len_buf) as usize;

                    let mut body = vec![0u8; msg_len];
                    if recv.read_exact(&mut body).await.is_err() {
                        return;
                    }
                    let body_str = String::from_utf8_lossy(&body);
                    let response = srv.handle_message(&body_str).unwrap_or_default();

                    let resp_len = response.len() as u32;
                    let _ = send.write_all(&resp_len.to_be_bytes()).await;
                    let _ = send.write_all(response.as_bytes()).await;
                });
            }
            Err(quinn::ConnectionError::ApplicationClosed { .. }) => break,
            Err(_) => break,
        }
    }
}
