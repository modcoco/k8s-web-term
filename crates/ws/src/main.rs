use common::PodSecrets;
use futures_util::{SinkExt as _, StreamExt as _};
use logger::logger_trace::init_logger;
use pod_exec::{pod_exec_connector, PodExecParams, PodExecPath, PodExecUrl};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

mod pod_exec;

#[tokio::main]
async fn main() {
    init_logger();
    tracing::info!("init");

    let pod_secrets = PodSecrets::new();
    let pod_exec_url = PodExecUrl {
        domain: String::from(&pod_secrets.kube_host),
        port: String::from(&pod_secrets.kube_port),
        path: PodExecPath {
            base_path: String::from("/api/v1"),
            namespace: String::from("/namespaces/default"),
            pod: String::from("/pods/web-term-559fdfcd89-gndr5"),
            tail_path: String::from("/exec"),
        },
    };
    let pod_exec_params = PodExecParams {
        container: "web-term".to_string(),
        stdin: true,
        stdout: true,
        stderr: true,
        tty: true,
        command: "ls".to_string(),
        pretty: true,
        follow: true,
    };

    let conn = pod_exec_connector(&pod_secrets, &pod_exec_url, &pod_exec_params).await;
    match conn {
        Ok(mut ws_stream) => {
            let mut closed = false;
            handle_websocket(&mut ws_stream, &mut closed).await;
        }
        Err(err) => {
            println!("ERROR, {}", err)
        }
    };
}

async fn handle_websocket(
    ws_stream: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    is_closed: &mut bool,
) {
    while let Some(Ok(msg)) = ws_stream.next().await {
        if *is_closed {
            break;
        }
        match msg {
            Message::Text(text) => {
                println!("Received text message: {}", text);
            }
            Message::Binary(data) => {
                if let Ok(text) = String::from_utf8(data) {
                    println!("Received binary message as text: {}", text);
                    if !*is_closed {
                        let text_message = Message::Text(text);
                        if let Err(e) = ws_stream.send(text_message).await {
                            eprintln!("Failed to send text message: {}", e);
                        }
                    }
                } else {
                    println!("Failed to convert binary message to text");
                }
            }
            Message::Ping(ping) => {
                if !*is_closed {
                    let pong = Message::Pong(ping);
                    if let Err(e) = ws_stream.send(pong).await {
                        eprintln!("Failed to send Pong: {}", e);
                    }
                }
            }
            Message::Pong(_) => todo!(),
            Message::Close(_) => {
                break;
            }
            Message::Frame(_) => todo!(),
        }
    }

    tokio::time::sleep(Duration::from_secs(10)).await;
    if !*is_closed {
        if let Err(e) = ws_stream.close(None).await {
            eprintln!("Failed to close WebSocket connection: {}", e);
        } else {
            println!("WebSocket connection closed");
        }
    }
}
