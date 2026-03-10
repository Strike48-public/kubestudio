//! KubeStudio Connector for Matrix
//!
//! This binary runs KubeStudio as a Matrix connector with dual behaviors:
//! - APP: Dioxus liveview with HTTP + WebSocket proxy for the UI
//! - TOOL: Kubernetes cluster management tools for AI agents
//!
//! Architecture:
//! - Dioxus liveview server runs on localhost:3030
//!   - /liveview - HTML page with JS client
//!   - /ws - WebSocket for live updates
//! - Matrix connector proxies requests to the liveview server
//!   - HTTP requests: ExecuteRequest → proxy to Dioxus (APP) or execute tool (TOOL)
//!   - WebSocket: WsOpenRequest/WsFrame → proxy to Dioxus /ws
//!
//! This implementation uses ConnectorClient directly (not ConnectorRunner)
//! to handle WebSocket messages from the proto that the SDK doesn't process.

use axum::Router;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dashmap::DashMap;
use dioxus_liveview::LiveviewRouter as _;
use futures::{SinkExt, StreamExt};
use ks_kube::{KubeClient, PermissionMode, Toolbox, auth, cleanup_orphaned_toolbox};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use strike48_connector::{
    AppManifest, AppPageRequest, AppPageResponse, BodyEncoding, ClientOptions, ConnectorBehavior,
    ConnectorClient, ConnectorConfig, NavigationConfig, OttProvider, PayloadEncoding,
};
use strike48_proto::proto::{
    self, ConnectorCapabilities, ExecuteResponse, HeartbeatRequest,
    InstanceMetadata, RegisterConnectorRequest, StreamMessage, WebSocketCloseRequest,
    WebSocketFrame, WebSocketFrameType, WebSocketOpenRequest, WebSocketOpenResponse,
    stream_message::Message,
};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsMessage;

/// Global IPC address for the Dioxus liveview server.
static DIOXUS_IPC: OnceLock<ks_ui::ipc::IpcAddr> = OnceLock::new();

/// Whether the token refresh loop has been spawned.
static REFRESH_SPAWNED: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------
// Token auto-refresh (matches Matrix's full_injection.js pattern)
// ---------------------------------------------------------------------------

/// Refresh constants matching Matrix Studio's injection scripts.
const MIN_REFRESH_DELAY_SECS: u64 = 30;
const REFRESH_TTL_FRACTION: f64 = 0.70;
const TOKEN_REFRESH_MAX_RETRIES: u32 = 3;
const TOKEN_REFRESH_RETRY_DELAY_SECS: u64 = 5;

/// Spawn a background token refresh loop (idempotent — only first call starts it).
fn spawn_token_refresh(api_base: String) {
    if REFRESH_SPAWNED.swap(true, Ordering::SeqCst) {
        return; // Already running
    }
    tokio::spawn(async move {
        tracing::info!("Token refresh loop started");
        let client = build_http_client();
        loop {
            let token = ks_ui::session::get_auth_token();
            if token.is_empty() {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }

            let remaining = match parse_token_remaining_secs(&token) {
                Some(r) if r > 0 => r as u64,
                _ => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(MIN_REFRESH_DELAY_SECS))
                        .await;
                    continue;
                }
            };

            // Schedule at 70% of remaining TTL, minimum 30s
            let delay =
                ((remaining as f64 * REFRESH_TTL_FRACTION) as u64).max(MIN_REFRESH_DELAY_SECS);
            tracing::debug!(
                "Token refresh scheduled in {}s (remaining {}s)",
                delay,
                remaining
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;

            let current_token = ks_ui::session::get_auth_token();
            if current_token.is_empty() {
                continue;
            }

            let mut success = false;
            for attempt in 0..TOKEN_REFRESH_MAX_RETRIES {
                match do_token_refresh(&client, &api_base, &current_token).await {
                    Ok(new_token) => {
                        ks_ui::session::set_auth_token(&new_token);
                        tracing::info!("Token refreshed successfully");
                        success = true;
                        break;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Token refresh attempt {}/{} failed: {}",
                            attempt + 1,
                            TOKEN_REFRESH_MAX_RETRIES,
                            e
                        );
                        if attempt + 1 < TOKEN_REFRESH_MAX_RETRIES {
                            tokio::time::sleep(tokio::time::Duration::from_secs(
                                TOKEN_REFRESH_RETRY_DELAY_SECS,
                            ))
                            .await;
                        }
                    }
                }
            }
            if !success {
                tracing::error!(
                    "Token refresh failed after {} attempts",
                    TOKEN_REFRESH_MAX_RETRIES
                );
            }
        }
    });
}

/// POST /api/app/token/refresh with Bearer auth, return new token string.
async fn do_token_refresh(
    client: &reqwest::Client,
    api_base: &str,
    token: &str,
) -> anyhow::Result<String> {
    let url = format!("{}/api/app/token/refresh", api_base.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .send()
        .await?;
    if !resp.status().is_success() {
        anyhow::bail!("Refresh returned status {}", resp.status());
    }
    let body: serde_json::Value = resp.json().await?;
    body.get("token")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("No token in refresh response"))
}

/// Parse remaining seconds until expiry from a sandbox token.
///
/// Sandbox tokens are `base64url(payload).signature` (2-part format).
/// The payload contains an `exp` claim with a Unix timestamp.
fn parse_token_remaining_secs(token: &str) -> Option<i64> {
    let payload_b64 = token.split('.').next()?;
    // base64url → standard base64
    let standard = payload_b64.replace('-', "+").replace('_', "/");
    let padded = match standard.len() % 4 {
        2 => format!("{}==", standard),
        3 => format!("{}=", standard),
        _ => standard,
    };
    let bytes = BASE64.decode(&padded).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let exp = claims.get("exp")?.as_i64()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    Some(exp - now)
}

/// Get the IPC address for the Dioxus server.
fn dioxus_ipc() -> &'static ks_ui::ipc::IpcAddr {
    DIOXUS_IPC
        .get()
        .expect("Dioxus IPC address not yet initialized")
}

/// Perform an HTTP GET over the IPC transport, returning (status, content_type, body).
async fn ipc_http_get(
    addr: &ks_ui::ipc::IpcAddr,
    path: &str,
) -> Result<(u16, String, Vec<u8>), String> {
    use http_body_util::BodyExt;
    use hyper_util::rt::TokioIo;

    let stream = ks_ui::ipc::IpcClientStream::connect(addr)
        .await
        .map_err(|e| format!("IPC connect: {}", e))?;

    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .map_err(|e| format!("HTTP handshake: {}", e))?;

    let conn_handle = tokio::spawn(async move {
        let _ = conn.await;
    });

    let req = hyper::Request::builder()
        .uri(path)
        .header("host", "localhost")
        .body(http_body_util::Empty::<hyper::body::Bytes>::new())
        .map_err(|e| format!("Request build: {}", e))?;

    let resp = sender
        .send_request(req)
        .await
        .map_err(|e| format!("Request send: {}", e))?;

    let status = resp.status().as_u16();
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("text/html")
        .to_string();

    let body = resp
        .into_body()
        .collect()
        .await
        .map_err(|e| format!("Body read: {}", e))?
        .to_bytes()
        .to_vec();

    drop(sender);
    conn_handle.abort();

    Ok((status, content_type, body))
}

/// Proxy HTTP requests to the Dioxus backend server over IPC.
async fn proxy_to_dioxus(path: &str, _params: &HashMap<String, String>) -> AppPageResponse {
    let target_path = if path == "/" || path.is_empty() {
        "/liveview"
    } else {
        path
    };

    let addr = dioxus_ipc();
    tracing::debug!("Proxying {} -> {}{}", path, addr, target_path);

    match ipc_http_get(addr, target_path).await {
        Ok((status, content_type, body)) => {
            let mut body_str = String::from_utf8_lossy(&body).to_string();

            if content_type.contains("html") {
                body_str = rewrite_dioxus_websocket_url(&body_str);
            }

            AppPageResponse {
                content_type,
                body: body_str,
                status,
                encoding: BodyEncoding::Utf8,
                headers: HashMap::new(),
            }
        }
        Err(e) => {
            tracing::error!("Failed to proxy to {}{}: {}", addr, target_path, e);
            AppPageResponse::error(502, format!("Backend unavailable: {}", e))
        }
    }
}

/// Inject Phoenix Socket shim for Matrix WebSocket proxy
fn rewrite_dioxus_websocket_url(html: &str) -> String {
    let phoenix_shim = r#"<script>
// Matrix Phoenix Socket Shim for Dioxus LiveView
(function() {
  console.log('[MatrixWsShim] Installing WebSocket shim...');

  const PHX_VSN = '2.0.0';
  const SOCKET_STATES = {connecting: 0, open: 1, closing: 2, closed: 3};

  const NativeWebSocket = window.WebSocket;
  window.__MATRIX_NATIVE_WEBSOCKET__ = NativeWebSocket;

  class MatrixWebSocket {
    constructor(url) {
      this.url = url;
      this.readyState = SOCKET_STATES.connecting;
      this.onopen = null;
      this.onclose = null;
      this.onerror = null;
      this.onmessage = null;
      this._ref = 0;
      this._joinRef = null;
      this.binaryType = 'blob';
      this._eventListeners = {open: [], close: [], error: [], message: []};

      const urlObj = new URL(url, window.location.origin);
      const isLiveViewWs = urlObj.pathname.includes('/ws') || urlObj.pathname.includes('/live');

      if (!isLiveViewWs) {
        console.log('[MatrixWsShim] Non-LiveView WebSocket, using native:', url);
        return new NativeWebSocket(url);
      }

      this._wsPath = urlObj.pathname;
      console.log('[MatrixWsShim] LiveView WebSocket detected, path:', this._wsPath);
      this._waitForMatrixAndConnect();
    }

    _waitForMatrixAndConnect() {
      const checkMatrix = () => {
        if (window.__MATRIX_SESSION_TOKEN__ && window.__MATRIX_APP_ADDRESS__) {
          console.log('[MatrixWsShim] Matrix ready, connecting...');
          this._connectToMatrix();
        } else {
          setTimeout(checkMatrix, 50);
        }
      };
      checkMatrix();
    }

    _connectToMatrix() {
      const token = window.__MATRIX_SESSION_TOKEN__;
      const appAddress = window.__MATRIX_APP_ADDRESS__;

      let matrixHost = window.location.host;
      const baseTag = document.querySelector('base');
      if (baseTag && baseTag.href) {
        try {
          const baseUrl = new URL(baseTag.href);
          matrixHost = baseUrl.host;
        } catch (e) {}
      }

      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      const phoenixUrl = protocol + '//' + matrixHost +
        '/api/app/ws/websocket?__st=' + encodeURIComponent(token) +
        '&app=' + encodeURIComponent(appAddress) +
        '&vsn=' + PHX_VSN;

      console.log('[MatrixWsShim] Connecting to Phoenix socket');

      this._socket = new NativeWebSocket(phoenixUrl);
      this._socket.binaryType = 'arraybuffer';

      this._socket.onopen = () => {
        console.log('[MatrixWsShim] Phoenix socket connected, joining channel');
        this._joinChannel();
      };

      this._socket.onclose = (event) => {
        this.readyState = SOCKET_STATES.closed;
        if (this._heartbeatInterval) clearInterval(this._heartbeatInterval);
        this._dispatchEvent('close', event);
      };

      this._socket.onerror = (event) => {
        this._dispatchEvent('error', event);
      };

      this._socket.onmessage = (event) => {
        this._handlePhoenixMessage(event.data);
      };
    }

    _joinChannel() {
      this._joinRef = String(++this._ref);
      const topic = 'app_ws:' + this._wsPath;
      const joinMsg = JSON.stringify([this._joinRef, String(++this._ref), topic, 'phx_join', {}]);
      console.log('[MatrixWsShim] Joining channel:', topic);
      this._socket.send(joinMsg);
    }

    _handlePhoenixMessage(data) {
      let msg;
      try { msg = JSON.parse(data); } catch (e) { return; }

      const [joinRef, ref, topic, event, payload] = msg;

      if (event === 'phx_reply' && joinRef === this._joinRef) {
        if (payload.status === 'ok') {
          console.log('[MatrixWsShim] Channel joined successfully');
          this.readyState = SOCKET_STATES.open;
          this._startHeartbeat();
          this._dispatchEvent('open', {type: 'open'});
        } else {
          this._dispatchEvent('error', new Error('Channel join failed'));
        }
      } else if (event === 'frame') {
        const frameData = payload.data;
        let messageData;
        try {
          const binary = atob(frameData);
          const bytes = new Uint8Array(binary.length);
          for (let i = 0; i < binary.length; i++) {
            bytes[i] = binary.charCodeAt(i);
          }
          messageData = bytes.buffer;
        } catch (e) {
          messageData = frameData;
        }
        this._dispatchEvent('message', {data: messageData, type: 'message'});
      } else if (event === 'close' || event === 'phx_close') {
        this.readyState = SOCKET_STATES.closed;
        this._dispatchEvent('close', {code: 1000, reason: 'closed'});
      } else if (event === 'phx_error') {
        this._dispatchEvent('error', new Error('Channel error'));
      }
    }

    _startHeartbeat() {
      this._heartbeatInterval = setInterval(() => {
        if (this.readyState !== SOCKET_STATES.open) return;
        const heartbeat = JSON.stringify([null, String(++this._ref), 'phoenix', 'heartbeat', {}]);
        this._socket.send(heartbeat);
      }, 30000);
    }

    send(data) {
      if (this.readyState !== SOCKET_STATES.open) return;

      const topic = 'app_ws:' + this._wsPath;
      let framePayload;

      if (data instanceof ArrayBuffer || data instanceof Uint8Array) {
        const bytes = new Uint8Array(data);
        let binary = '';
        for (let i = 0; i < bytes.length; i++) {
          binary += String.fromCharCode(bytes[i]);
        }
        framePayload = {data: btoa(binary), type: 'binary'};
      } else {
        const str = String(data);
        const bytes = new TextEncoder().encode(str);
        let binary = '';
        for (let i = 0; i < bytes.length; i++) {
          binary += String.fromCharCode(bytes[i]);
        }
        framePayload = {data: btoa(binary), type: 'text'};
      }

      const msg = JSON.stringify([this._joinRef, String(++this._ref), topic, 'frame', framePayload]);
      this._socket.send(msg);
    }

    close(code, reason) {
      if (this.readyState === SOCKET_STATES.closed) return;
      this.readyState = SOCKET_STATES.closing;
      if (this._heartbeatInterval) clearInterval(this._heartbeatInterval);
      this._socket.close(code || 1000, reason || '');
    }

    addEventListener(type, listener) {
      this._eventListeners[type] = this._eventListeners[type] || [];
      this._eventListeners[type].push(listener);
    }

    removeEventListener(type, listener) {
      if (this._eventListeners[type]) {
        this._eventListeners[type] = this._eventListeners[type].filter(l => l !== listener);
      }
    }

    _dispatchEvent(type, event) {
      const handler = this['on' + type];
      if (handler) handler.call(this, event);
      if (this._eventListeners[type]) {
        this._eventListeners[type].forEach(l => l.call(this, event));
      }
    }
  }

  window.WebSocket = MatrixWebSocket;
  console.log('[MatrixWsShim] WebSocket constructor replaced');
})();
</script>"#;

    let replacement_fn = r#"function __dioxusGetWsUrl(path) {
      let loc = window.location;
      let new_url = loc.protocol === "https:" ? "wss:" : "ws:";
      new_url += "//" + loc.host + path;
      console.log('[Dioxus] WebSocket URL:', new_url);
      return new_url;
    }"#;

    let re = regex::Regex::new(
        r#"function __dioxusGetWsUrl\(path\) \{[\s\S]*?new_url \+= "\/\/" \+ loc\.host \+ path;[\s\S]*?return new_url;[\s\S]*?\}"#
    ).unwrap();

    let mut result = html.to_string();

    if re.is_match(html) {
        tracing::info!("Rewriting Dioxus WebSocket URL function and injecting Phoenix shim");
        result = re.replace(&result, replacement_fn).to_string();

        // Matrix Studio's TokenInjector handles injecting
        // window.__MATRIX_SESSION_TOKEN__ into the HTML before it reaches the
        // browser, so we only inject the Phoenix WebSocket shim here.
        let injected = phoenix_shim.to_string();

        if let Some(head_end) = result.find("</head>") {
            result.insert_str(head_end, &injected);
        } else if let Some(body_start) = result.find("<body") {
            result.insert_str(body_start, &injected);
        }
    }

    result
}

/// Start the Dioxus liveview server on an IPC transport.
///
/// Uses `IpcListener` (Unix sockets on Unix, named pipes on Windows).
/// Drives the accept loop manually via hyper since axum 0.7's `serve()`
/// only accepts `TcpListener`.
async fn start_dioxus_server(ipc_addr: ks_ui::ipc::IpcAddr) {
    use axum::routing::get;
    use hyper::body::Incoming;
    use hyper_util::rt::TokioIo;
    use tower_service::Service;

    let router = Router::new()
        .with_app("/", ks_ui::App)
        .route("/health", get(|| async { "OK" }))
        .route(
            "/connector/info",
            get(|| async { axum::Json(serde_json::to_value(app_manifest()).unwrap()) }),
        )
        .route(
            "/auth/status",
            get(|| async {
                let has_token = !ks_ui::session::get_auth_token().is_empty();
                axum::Json(serde_json::json!({ "authenticated": has_token }))
            }),
        );

    let mut listener = ks_ui::ipc::IpcListener::bind(&ipc_addr)
        .expect("failed to bind IPC listener for Dioxus server");
    tracing::info!("Dioxus liveview server listening on {}", ipc_addr);
    DIOXUS_IPC.set(ipc_addr).expect("DIOXUS_IPC already set");

    let make_svc = router.into_make_service();

    loop {
        let stream = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("IPC accept error: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                continue;
            }
        };

        let io = TokioIo::new(stream);
        let mut svc = make_svc.clone();
        let tower_svc = svc.call(()).await.unwrap();

        tokio::spawn(async move {
            let svc = hyper::service::service_fn(move |req: hyper::Request<Incoming>| {
                let mut svc = tower_svc.clone();
                async move { svc.call(req).await }
            });

            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, svc)
                .with_upgrades()
                .await
            {
                tracing::debug!("IPC connection error: {}", e);
            }
        });
    }
}

/// WebSocket connection state
struct WsConnectionState {
    to_backend_tx: mpsc::Sender<Vec<u8>>,
}

/// Main connector that handles the gRPC stream directly
struct StudioKubeConnector {
    ws_connections: Arc<DashMap<String, WsConnectionState>>,
    matrix_tx: mpsc::UnboundedSender<StreamMessage>,
    shutdown: Arc<AtomicBool>,
}

impl StudioKubeConnector {
    fn new(matrix_tx: mpsc::UnboundedSender<StreamMessage>) -> Self {
        Self {
            ws_connections: Arc::new(DashMap::new()),
            matrix_tx,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    async fn handle_execute(&self, req: proto::ExecuteRequest) {
        let request_id = req.request_id.clone();

        // Try to parse as JSON to check if it's a tool request
        let payload_json: Option<serde_json::Value> = serde_json::from_slice(&req.payload).ok();

        let payload = if let Some(ref json) = payload_json
            && json.get("tool").is_some()
        {
            // This is a TOOL request
            let tool = json
                .get("tool")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let params = json
                .get("parameters")
                .or_else(|| json.get("args"))
                .cloned()
                .unwrap_or(serde_json::json!({}));

            tracing::info!("Executing tool: {}", tool);
            let result = self.handle_tool_execute(tool, params).await;

            let response = serde_json::json!({
                "tool": tool,
                "result": result,
                "timestamp": chrono::Utc::now().to_rfc3339()
            });

            serde_json::to_vec(&response).unwrap_or_default()
        } else {
            // This is an APP request - proxy to Dioxus
            let page_request: AppPageRequest =
                serde_json::from_slice(&req.payload).unwrap_or_else(|_| AppPageRequest {
                    path: "/".to_string(),
                    params: HashMap::new(),
                });

            let response = proxy_to_dioxus(&page_request.path, &page_request.params).await;
            serde_json::to_vec(&response).unwrap_or_default()
        };

        let response_msg = StreamMessage {
            message: Some(Message::ExecuteResponse(ExecuteResponse {
                request_id,
                success: true,
                payload,
                payload_encoding: PayloadEncoding::Json as i32,
                error: String::new(),
                duration_ms: 0,
            })),
        };

        if let Err(e) = self.matrix_tx.send(response_msg) {
            tracing::error!("Failed to send execute response: {}", e);
        }
    }

    async fn handle_ws_open(&self, req: WebSocketOpenRequest) {
        let connection_id = req.connection_id.clone();

        // Extract __st (user session token) from query string if present
        if !req.query_string.is_empty() {
            for pair in req.query_string.split('&') {
                if let Some(value) = pair.strip_prefix("__st=") {
                    let decoded = urlencoding::decode(value).unwrap_or_default();
                    if !decoded.is_empty() {
                        tracing::info!("Captured user session token from __st param");
                        ks_ui::session::set_auth_token(&decoded);

                        // Start server-side token refresh loop if we have a Matrix API URL
                        let api_base = std::env::var("STRIKE48_API_URL").unwrap_or_default();
                        if !api_base.is_empty() {
                            spawn_token_refresh(api_base);
                        }

                        // Try to extract display name from JWT payload
                        if let Some(payload) = decoded.split('.').nth(1) {
                            // JWT base64 may need padding
                            let padded = match payload.len() % 4 {
                                2 => format!("{}==", payload),
                                3 => format!("{}=", payload),
                                _ => payload.to_string(),
                            };
                            if let Ok(bytes) = BASE64.decode(&padded)
                                && let Ok(claims) =
                                    serde_json::from_slice::<serde_json::Value>(&bytes)
                                && let Some(email) = claims
                                    .get("email")
                                    .or_else(|| claims.get("preferred_username"))
                                    .and_then(|v| v.as_str())
                            {
                                tracing::info!("Extracted display name from token: {}", email);
                                ks_ui::session::set_display_name(email);
                            }
                        }
                    }
                }
            }
        }

        let ws_path = if req.path.is_empty() {
            "/ws"
        } else {
            &req.path
        };
        let ws_uri = if req.query_string.is_empty() {
            format!("ws://localhost{}", ws_path)
        } else {
            format!("ws://localhost{}?{}", ws_path, req.query_string)
        };

        let ipc_addr = dioxus_ipc();
        tracing::info!("Opening WebSocket to backend: {}{}", ipc_addr, ws_path);

        let stream = match ks_ui::ipc::IpcClientStream::connect(ipc_addr).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to connect to Dioxus server: {}", e);
                let response = StreamMessage {
                    message: Some(Message::WsOpenResponse(WebSocketOpenResponse {
                        connection_id,
                        success: false,
                        error: format!("Failed to connect: {}", e),
                    })),
                };
                let _ = self.matrix_tx.send(response);
                return;
            }
        };

        match tokio_tungstenite::client_async(ws_uri, stream).await {
            Ok((ws_stream, _)) => {
                tracing::info!("WebSocket connected for connection_id: {}", connection_id);

                let (mut ws_sink, mut ws_source) = ws_stream.split();
                let (to_backend_tx, mut to_backend_rx) = mpsc::channel::<Vec<u8>>(100);

                self.ws_connections
                    .insert(connection_id.clone(), WsConnectionState { to_backend_tx });

                let response = StreamMessage {
                    message: Some(Message::WsOpenResponse(WebSocketOpenResponse {
                        connection_id: connection_id.clone(),
                        success: true,
                        error: String::new(),
                    })),
                };
                let _ = self.matrix_tx.send(response);

                let conn_id_write = connection_id.clone();
                tokio::spawn(async move {
                    while let Some(data) = to_backend_rx.recv().await {
                        let decoded = match String::from_utf8(data.clone()) {
                            Ok(base64_str) => BASE64.decode(&base64_str).unwrap_or(data),
                            Err(_) => data,
                        };

                        let msg = WsMessage::Binary(decoded);
                        if let Err(e) = ws_sink.send(msg).await {
                            tracing::error!("Error sending to backend WS {}: {}", conn_id_write, e);
                            break;
                        }
                    }
                });

                let conn_id_read = connection_id.clone();
                let matrix_tx = self.matrix_tx.clone();
                let ws_connections = self.ws_connections.clone();
                tokio::spawn(async move {
                    while let Some(msg_result) = ws_source.next().await {
                        match msg_result {
                            Ok(msg) => {
                                let (frame_type, data) = match msg {
                                    WsMessage::Text(text) => (
                                        WebSocketFrameType::WebsocketFrameTypeText,
                                        text.into_bytes(),
                                    ),
                                    WsMessage::Binary(data) => (
                                        WebSocketFrameType::WebsocketFrameTypeBinary,
                                        data.to_vec(),
                                    ),
                                    WsMessage::Ping(data) => {
                                        (WebSocketFrameType::WebsocketFrameTypePing, data.to_vec())
                                    }
                                    WsMessage::Pong(data) => {
                                        (WebSocketFrameType::WebsocketFrameTypePong, data.to_vec())
                                    }
                                    WsMessage::Close(_) => {
                                        tracing::info!("Backend WS closed for {}", conn_id_read);
                                        break;
                                    }
                                    WsMessage::Frame(_) => continue,
                                };

                                let encoded_data = BASE64.encode(&data);

                                let frame = StreamMessage {
                                    message: Some(Message::WsFrame(WebSocketFrame {
                                        connection_id: conn_id_read.clone(),
                                        frame_type: frame_type as i32,
                                        data: encoded_data.into_bytes(),
                                    })),
                                };

                                if let Err(e) = matrix_tx.send(frame) {
                                    tracing::error!("Error sending frame to Matrix: {}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Error reading from backend WS {}: {}",
                                    conn_id_read,
                                    e
                                );
                                break;
                            }
                        }
                    }
                    ws_connections.remove(&conn_id_read);
                });
            }
            Err(e) => {
                tracing::error!("Failed to connect to backend WS: {}", e);
                let response = StreamMessage {
                    message: Some(Message::WsOpenResponse(WebSocketOpenResponse {
                        connection_id,
                        success: false,
                        error: format!("Failed to connect: {}", e),
                    })),
                };
                let _ = self.matrix_tx.send(response);
            }
        }
    }

    async fn handle_ws_frame(&self, frame: WebSocketFrame) {
        if let Some(conn) = self.ws_connections.get(&frame.connection_id)
            && let Err(e) = conn.to_backend_tx.send(frame.data).await
        {
            tracing::error!("Error forwarding frame to backend: {}", e);
        }
    }

    fn handle_ws_close(&self, req: WebSocketCloseRequest) {
        tracing::info!("Closing WebSocket: {}", req.connection_id);
        self.ws_connections.remove(&req.connection_id);
    }

    /// Handle tool execution requests
    async fn handle_tool_execute(
        &self,
        tool: &str,
        params: serde_json::Value,
    ) -> serde_json::Value {
        match tool {
            "list_clusters" => {
                let kubeconfig_path = params
                    .get("kubeconfig_path")
                    .and_then(|v| v.as_str())
                    .map(std::path::PathBuf::from);

                match auth::load_kubeconfig(kubeconfig_path).await {
                    Ok(config) => {
                        let contexts = auth::list_contexts(&config);
                        let current = auth::current_context(&config);

                        let context_details: Vec<serde_json::Value> = config
                            .contexts
                            .iter()
                            .map(|ctx| {
                                let is_current = current.as_ref() == Some(&ctx.name);
                                let cluster_name = ctx
                                    .context
                                    .as_ref()
                                    .map(|c| c.cluster.clone())
                                    .unwrap_or_default();
                                let namespace =
                                    ctx.context.as_ref().and_then(|c| c.namespace.clone());
                                let user = ctx
                                    .context
                                    .as_ref()
                                    .map(|c| c.user.clone())
                                    .unwrap_or_default();

                                let server = config
                                    .clusters
                                    .iter()
                                    .find(|c| c.name == cluster_name)
                                    .and_then(|c| c.cluster.as_ref())
                                    .and_then(|c| c.server.clone());

                                serde_json::json!({
                                    "name": ctx.name,
                                    "cluster": cluster_name,
                                    "namespace": namespace,
                                    "user": user,
                                    "server": server,
                                    "is_current": is_current
                                })
                            })
                            .collect();

                        serde_json::json!({
                            "success": true,
                            "current_context": current,
                            "contexts": context_details,
                            "count": contexts.len()
                        })
                    }
                    Err(e) => serde_json::json!({
                        "success": false,
                        "error": format!("Failed to load kubeconfig: {}", e)
                    }),
                }
            }

            "get_current_context" => {
                let kubeconfig_path = params
                    .get("kubeconfig_path")
                    .and_then(|v| v.as_str())
                    .map(std::path::PathBuf::from);

                match auth::load_kubeconfig(kubeconfig_path).await {
                    Ok(config) => {
                        let current = auth::current_context(&config);
                        serde_json::json!({
                            "success": true,
                            "current_context": current
                        })
                    }
                    Err(e) => serde_json::json!({
                        "success": false,
                        "error": format!("Failed to load kubeconfig: {}", e)
                    }),
                }
            }

            "get_cluster_info" => {
                let context_name = match params.get("context_name").and_then(|v| v.as_str()) {
                    Some(name) => name,
                    None => {
                        return serde_json::json!({
                            "success": false,
                            "error": "Missing required parameter: context_name"
                        });
                    }
                };

                let kubeconfig_path = params
                    .get("kubeconfig_path")
                    .and_then(|v| v.as_str())
                    .map(std::path::PathBuf::from);

                match auth::load_kubeconfig(kubeconfig_path).await {
                    Ok(config) => {
                        let context = config.contexts.iter().find(|c| c.name == context_name);

                        match context {
                            Some(ctx) => {
                                let cluster_name = ctx
                                    .context
                                    .as_ref()
                                    .map(|c| c.cluster.clone())
                                    .unwrap_or_default();
                                let namespace =
                                    ctx.context.as_ref().and_then(|c| c.namespace.clone());
                                let user = ctx
                                    .context
                                    .as_ref()
                                    .map(|c| c.user.clone())
                                    .unwrap_or_default();

                                let cluster =
                                    config.clusters.iter().find(|c| c.name == cluster_name);

                                let (server, tls_server_name, insecure) = cluster
                                    .and_then(|c| c.cluster.as_ref())
                                    .map(|c| {
                                        (
                                            c.server.clone(),
                                            c.tls_server_name.clone(),
                                            c.insecure_skip_tls_verify,
                                        )
                                    })
                                    .unwrap_or((None, None, None));

                                let is_current =
                                    auth::current_context(&config).as_ref() == Some(&ctx.name);

                                serde_json::json!({
                                    "success": true,
                                    "context": {
                                        "name": ctx.name,
                                        "cluster": cluster_name,
                                        "namespace": namespace,
                                        "user": user,
                                        "is_current": is_current
                                    },
                                    "cluster": {
                                        "server": server,
                                        "tls_server_name": tls_server_name,
                                        "insecure_skip_tls_verify": insecure
                                    }
                                })
                            }
                            None => serde_json::json!({
                                "success": false,
                                "error": format!("Context '{}' not found", context_name)
                            }),
                        }
                    }
                    Err(e) => serde_json::json!({
                        "success": false,
                        "error": format!("Failed to load kubeconfig: {}", e)
                    }),
                }
            }

            // Toolbox tools - for executing commands inside the cluster
            "toolbox_status" => {
                let context_name = params.get("context_name").and_then(|v| v.as_str());

                match get_toolbox(context_name).await {
                    Ok(toolbox) => match toolbox.status().await {
                        Ok(status) => serde_json::json!({
                            "success": true,
                            "status": status
                        }),
                        Err(e) => serde_json::json!({
                            "success": false,
                            "error": format!("Failed to get toolbox status: {}", e)
                        }),
                    },
                    Err(e) => serde_json::json!({
                        "success": false,
                        "error": e
                    }),
                }
            }

            "toolbox_deploy" => {
                let context_name = params.get("context_name").and_then(|v| v.as_str());

                match get_toolbox(context_name).await {
                    Ok(toolbox) => match toolbox.ensure_running().await {
                        Ok(status) => serde_json::json!({
                            "success": true,
                            "message": "Toolbox deployed and ready",
                            "status": status
                        }),
                        Err(e) => serde_json::json!({
                            "success": false,
                            "error": format!("Failed to deploy toolbox: {}", e)
                        }),
                    },
                    Err(e) => serde_json::json!({
                        "success": false,
                        "error": e
                    }),
                }
            }

            "toolbox_exec" => {
                let command = match params.get("command").and_then(|v| v.as_str()) {
                    Some(cmd) => cmd,
                    None => {
                        return serde_json::json!({
                            "success": false,
                            "error": "Missing required parameter: command"
                        });
                    }
                };

                let context_name = params.get("context_name").and_then(|v| v.as_str());

                match get_toolbox(context_name).await {
                    Ok(toolbox) => {
                        // Ensure toolbox is running first
                        if let Err(e) = toolbox.ensure_running().await {
                            return serde_json::json!({
                                "success": false,
                                "error": format!("Failed to ensure toolbox is running: {}", e)
                            });
                        }

                        match toolbox.exec(command).await {
                            Ok(result) => serde_json::json!({
                                "success": result.success,
                                "exit_code": result.exit_code,
                                "stdout": result.stdout,
                                "stderr": result.stderr,
                                "command": command
                            }),
                            Err(e) => serde_json::json!({
                                "success": false,
                                "error": format!("Failed to execute command: {}", e)
                            }),
                        }
                    }
                    Err(e) => serde_json::json!({
                        "success": false,
                        "error": e
                    }),
                }
            }

            "toolbox_delete" => {
                let context_name = params.get("context_name").and_then(|v| v.as_str());

                match get_toolbox(context_name).await {
                    Ok(toolbox) => match toolbox.delete().await {
                        Ok(()) => serde_json::json!({
                            "success": true,
                            "message": "Toolbox deleted"
                        }),
                        Err(e) => serde_json::json!({
                            "success": false,
                            "error": format!("Failed to delete toolbox: {}", e)
                        }),
                    },
                    Err(e) => serde_json::json!({
                        "success": false,
                        "error": e
                    }),
                }
            }

            "get_permissions" => {
                let mode = get_permission_mode();
                let mode_str = match mode {
                    PermissionMode::ReadOnly => "read",
                    PermissionMode::ReadWrite => "write",
                };

                serde_json::json!({
                    "success": true,
                    "permission_mode": mode_str,
                    "can_write": mode == PermissionMode::ReadWrite,
                    "description": match mode {
                        PermissionMode::ReadOnly => "Read-only mode: Only read operations allowed (get, list, describe, logs). Write operations (apply, create, delete, etc.) are blocked.",
                        PermissionMode::ReadWrite => "Read-write mode: Full access to all operations including create, apply, delete, scale, etc."
                    }
                })
            }

            _ => serde_json::json!({
                "success": false,
                "error": format!("Unknown tool: {}", tool)
            }),
        }
    }
}

/// Get the current permission mode from environment
fn get_permission_mode() -> PermissionMode {
    std::env::var("KUBESTUDIO_MODE")
        .map(|s| PermissionMode::from_str(&s))
        .unwrap_or_default()
}

/// Helper to get a Toolbox instance for the given context
async fn get_toolbox(context_name: Option<&str>) -> Result<Toolbox, String> {
    // Determine which context to use
    let context = match context_name {
        Some(name) => name.to_string(),
        None => {
            // Load kubeconfig to get current context
            let config = auth::load_kubeconfig(None)
                .await
                .map_err(|e| format!("Failed to load kubeconfig: {}", e))?;
            auth::current_context(&config)
                .ok_or_else(|| "No current context set in kubeconfig".to_string())?
        }
    };

    // Create kube client for the context
    let kube_client = KubeClient::from_context(&context)
        .await
        .map_err(|e| format!("Failed to connect to cluster '{}': {}", context, e))?;

    // Create toolbox with the permission mode from environment
    let permission_mode = get_permission_mode();
    Ok(Toolbox::with_permission_mode(
        kube_client.client().clone(),
        permission_mode,
    ))
}

/// Tool schemas for TOOL behavior registration
fn tool_schemas() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "list_clusters",
            "description": "List all available Kubernetes clusters/contexts from the kubeconfig file. Returns context names, cluster endpoints, and which context is currently active.",
            "parameters": {
                "type": "object",
                "properties": {
                    "kubeconfig_path": {
                        "type": "string",
                        "description": "Optional path to kubeconfig file. If not provided, uses default location (~/.kube/config)"
                    }
                },
                "required": []
            }
        },
        {
            "name": "get_current_context",
            "description": "Get the name of the currently active Kubernetes context from kubeconfig.",
            "parameters": {
                "type": "object",
                "properties": {
                    "kubeconfig_path": {
                        "type": "string",
                        "description": "Optional path to kubeconfig file. If not provided, uses default location (~/.kube/config)"
                    }
                },
                "required": []
            }
        },
        {
            "name": "get_cluster_info",
            "description": "Get detailed information about a specific Kubernetes cluster context, including server endpoint, namespace, and user.",
            "parameters": {
                "type": "object",
                "properties": {
                    "context_name": {
                        "type": "string",
                        "description": "Name of the context to get info for"
                    },
                    "kubeconfig_path": {
                        "type": "string",
                        "description": "Optional path to kubeconfig file. If not provided, uses default location (~/.kube/config)"
                    }
                },
                "required": ["context_name"]
            }
        },
        {
            "name": "toolbox_status",
            "description": "Get the status of the KubeStudio toolbox pod. The toolbox is a persistent pod running in the cluster that allows executing commands with kubectl and other CLI tools.",
            "parameters": {
                "type": "object",
                "properties": {
                    "context_name": {
                        "type": "string",
                        "description": "Kubernetes context to use. If not provided, uses current context."
                    }
                },
                "required": []
            }
        },
        {
            "name": "toolbox_deploy",
            "description": "Deploy or ensure the KubeStudio toolbox pod is running. Creates namespace, service account, RBAC, and pod if they don't exist. Permissions depend on KUBESTUDIO_MODE: 'write' = cluster-admin, 'read' = view-only.",
            "parameters": {
                "type": "object",
                "properties": {
                    "context_name": {
                        "type": "string",
                        "description": "Kubernetes context to use. If not provided, uses current context."
                    }
                },
                "required": []
            }
        },
        {
            "name": "toolbox_exec",
            "description": "Execute a shell command in the KubeStudio toolbox pod. In read-only mode, write commands (apply, create, delete, etc.) are blocked. Use get_permissions to check current mode.",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute (e.g., 'kubectl get pods -A', 'kubectl apply -f -', 'helm list')"
                    },
                    "context_name": {
                        "type": "string",
                        "description": "Kubernetes context to use. If not provided, uses current context."
                    }
                },
                "required": ["command"]
            }
        },
        {
            "name": "toolbox_delete",
            "description": "Delete the KubeStudio toolbox pod. Use this to clean up resources when done.",
            "parameters": {
                "type": "object",
                "properties": {
                    "context_name": {
                        "type": "string",
                        "description": "Kubernetes context to use. If not provided, uses current context."
                    }
                },
                "required": []
            }
        },
        {
            "name": "get_permissions",
            "description": "Get the current KubeStudio permission mode. Returns whether the connector is in 'read' (view-only) or 'write' (full access) mode. In read mode, write operations are blocked.",
            "parameters": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }
    ])
}

/// Return the app manifest for this connector.
fn app_manifest() -> AppManifest {
    AppManifest::new("KubeStudio", "/")
        .description("Kubernetes cluster management dashboard")
        .icon("hero-server-stack")
        .navigation(NavigationConfig::nested(&["Apps"]))
}

/// Build the registration message
fn build_registration_message(config: &ConnectorConfig, jwt_token: Option<&str>) -> StreamMessage {
    let manifest = app_manifest();

    // Serialize manifest and inject api_access (field not yet in SDK 0.1.x).
    // Matrix reads api_access from the JSON manifest to decide on user consent.
    let mut manifest_value = serde_json::to_value(&manifest).unwrap_or_default();
    if let Some(obj) = manifest_value.as_object_mut() {
        obj.insert("api_access".to_string(), serde_json::json!(true));
    }
    let manifest_json = serde_json::to_string(&manifest_value).unwrap_or_default();

    let mut metadata = HashMap::new();
    // APP behavior metadata
    metadata.insert("app_manifest".to_string(), manifest_json);
    metadata.insert("timeout_ms".to_string(), "10000".to_string());

    // TOOL behavior metadata
    metadata.insert(
        "tool_schemas".to_string(),
        serde_json::to_string(&tool_schemas()).unwrap_or_default(),
    );
    metadata.insert("tool_count".to_string(), "8".to_string());
    metadata.insert(
        "tool_names".to_string(),
        "list_clusters,get_current_context,get_cluster_info,toolbox_status,toolbox_deploy,toolbox_exec,toolbox_delete,get_permissions".to_string(),
    );

    let capabilities = ConnectorCapabilities {
        connector_type: "app-kube-studio".to_string(),
        version: "1.0.0".to_string(),
        supported_encodings: vec![PayloadEncoding::Json as i32],
        behaviors: vec![
            ConnectorBehavior::App as i32,
            ConnectorBehavior::Tool as i32,
        ],
        metadata,
        task_types: vec![],
    };

    let register_request = RegisterConnectorRequest {
        tenant_id: config.tenant_id.clone(),
        connector_type: "app-kube-studio".to_string(),
        instance_id: config.instance_id.clone(),
        capabilities: Some(capabilities),
        jwt_token: jwt_token.unwrap_or(&config.auth_token).to_string(),
        session_token: String::new(),
        scope: 0,
        instance_metadata: Some(InstanceMetadata {
            display_name: "KubeStudio".to_string(),
            tags: Vec::new(),
            metadata: std::collections::HashMap::new(),
        }),
    };

    StreamMessage {
        message: Some(Message::RegisterRequest(register_request)),
    }
}

/// Build a reqwest client that respects MATRIX_TLS_INSECURE
fn build_http_client() -> reqwest::Client {
    let insecure = std::env::var("MATRIX_TLS_INSECURE")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    reqwest::Client::builder()
        .danger_accept_invalid_certs(insecure)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Auth context for reconnection
struct AuthContext {
    ott_provider: OttProvider,
}

/// Sleep that can be interrupted by shutdown
async fn sleep_with_shutdown(duration: tokio::time::Duration, shutdown: &AtomicBool) -> bool {
    let interval = tokio::time::Duration::from_millis(100);
    let iterations = (duration.as_millis() / interval.as_millis()).max(1) as u64;

    for _ in 0..iterations {
        if shutdown.load(Ordering::SeqCst) {
            return true; // Shutdown requested
        }
        tokio::time::sleep(interval).await;
    }
    false // Normal completion
}

/// Result of message loop - whether to reconnect and with what auth
#[allow(clippy::large_enum_variant)]
enum MessageLoopResult {
    /// Stream closed normally or shutdown requested
    Exit,
    /// Need to reconnect with new credentials
    Reconnect(AuthContext),
}

/// Run the connector message loop
async fn run_message_loop(
    connector: Arc<StudioKubeConnector>,
    mut rx: mpsc::UnboundedReceiver<StreamMessage>,
    config: &ConnectorConfig,
    shutdown: &AtomicBool,
) -> MessageLoopResult {
    // Application-level heartbeat interval (30s) to keep the server session alive.
    // The server reaps sessions after 90s of inactivity. HTTP/2 PING frames do NOT
    // reset the server's last_seen timer — only proto HeartbeatRequest messages do.
    let mut heartbeat_interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
    heartbeat_interval.tick().await; // skip immediate first tick

    loop {
        // Check shutdown flag at the start of each iteration
        if shutdown.load(Ordering::SeqCst) {
            tracing::info!("Shutdown requested, exiting message loop");
            return MessageLoopResult::Exit;
        }

        tokio::select! {
            biased;  // Check in order - shutdown check first

            // Periodic shutdown check (every 100ms)
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                if shutdown.load(Ordering::SeqCst) {
                    tracing::info!("Shutdown requested, exiting message loop");
                    return MessageLoopResult::Exit;
                }
            }

            // Send application-level heartbeat to keep server session alive
            _ = heartbeat_interval.tick() => {
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                let heartbeat_msg = StreamMessage {
                    message: Some(Message::HeartbeatRequest(HeartbeatRequest {
                        gateway_id: String::new(), // server injects from stream state
                        timestamp_ms: now_ms,
                    })),
                };
                if let Err(e) = connector.matrix_tx.send(heartbeat_msg) {
                    tracing::warn!("Failed to send heartbeat: {}", e);
                } else {
                    tracing::trace!("Sent heartbeat");
                }
            }

            msg_opt = rx.recv() => {
                let Some(msg) = msg_opt else {
                    tracing::info!("Stream closed by server");
                    return MessageLoopResult::Exit;
                };

                match msg.message {
                    Some(Message::RegisterResponse(resp)) => {
                        if resp.success {
                            tracing::info!("Registered successfully: {}", resp.connector_arn);
                        } else {
                            tracing::error!("Registration failed: {}", resp.error);
                            return MessageLoopResult::Exit;
                        }
                    }
                    Some(Message::ExecuteRequest(req)) => {
                        tracing::info!("Received ExecuteRequest: {}", req.request_id);
                        let connector = connector.clone();
                        tokio::spawn(async move {
                            connector.handle_execute(req).await;
                        });
                    }
                    Some(Message::WsOpenRequest(req)) => {
                        tracing::info!("Received WsOpenRequest: {} path={} query_string={} headers={:?}", req.connection_id, req.path, req.query_string, req.headers);
                        let connector = connector.clone();
                        tokio::spawn(async move {
                            connector.handle_ws_open(req).await;
                        });
                    }
                    Some(Message::WsFrame(frame)) => {
                        connector.handle_ws_frame(frame).await;
                    }
                    Some(Message::WsCloseRequest(req)) => {
                        connector.handle_ws_close(req);
                    }
                    Some(Message::CredentialsIssued(creds)) => {
                        tracing::info!(
                            "Received CredentialsIssued - attempting OTT registration (api_url={}, register_url={})",
                            creds.matrix_api_url,
                            creds.register_url
                        );

                        // In StrikeHub mode, always use the server-provided URL for OTT registration
                        // because STRIKE48_API_URL points to StrikeHub's local auth proxy which
                        // doesn't have the /api/connectors/register-with-ott endpoint.
                        let api_base = if std::env::var("STRIKEHUB_SOCKET").is_ok() {
                            String::new()
                        } else {
                            std::env::var("STRIKE48_API_URL").unwrap_or_default()
                        };
                        let ott_api_url = if api_base.is_empty() {
                            &creds.matrix_api_url
                        } else {
                            &api_base
                        };

                        let mut ott_provider = OttProvider::new(
                            Some(config.connector_type.clone()),
                            Some(config.instance_id.clone()),
                        );

                        match ott_provider
                            .register_public_key_with_ott_data(
                                &creds.ott,
                                ott_api_url,
                                &creds.register_url,
                                &config.connector_type,
                                Some(&config.instance_id),
                            )
                            .await
                        {
                            Ok(response) => {
                                tracing::info!(
                                    "OTT registration successful (client_id={}), will reconnect with JWT",
                                    response.client_id
                                );
                                return MessageLoopResult::Reconnect(AuthContext {
                                    ott_provider,
                                });
                            }
                            Err(e) => {
                                tracing::error!(
                                    "OTT registration failed: {} (matrix_api_url={}, register_url={}, STRIKE48_API_URL={:?})",
                                    e,
                                    creds.matrix_api_url,
                                    creds.register_url,
                                    std::env::var("STRIKE48_API_URL").ok()
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("Starting KubeStudio Connector");
    tracing::info!("================================");

    // Build IPC address: StrikeHub provides a Unix socket path, otherwise PID-based
    let is_strikehub_mode;
    let ipc_addr;

    #[cfg(unix)]
    {
        if let Ok(sock_path) = std::env::var("STRIKEHUB_SOCKET") {
            ipc_addr = ks_ui::ipc::IpcAddr::from_path(PathBuf::from(sock_path));
            is_strikehub_mode = true;
        } else {
            ipc_addr = ks_ui::ipc::IpcAddr::for_connector(std::process::id());
            is_strikehub_mode = false;
        }
    }
    #[cfg(not(unix))]
    {
        if let Ok(sock_val) = std::env::var("STRIKEHUB_SOCKET") {
            ipc_addr = ks_ui::ipc::IpcAddr::from_string(sock_val);
            is_strikehub_mode = true;
        } else {
            ipc_addr = ks_ui::ipc::IpcAddr::for_connector(std::process::id());
            is_strikehub_mode = false;
        }
    }

    // Start Dioxus liveview server in background
    let dioxus_handle = tokio::spawn(start_dioxus_server(ipc_addr));

    // Wait for server to start and DIOXUS_IPC to be set
    for _ in 0..40 {
        if DIOXUS_IPC.get().is_some() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    if is_strikehub_mode && std::env::var("STRIKE48_URL").is_err() {
        // StrikeHub mode without gateway URL: serve liveview only.
        tracing::info!("StrikeHub mode: serving liveview only (no Matrix URL configured)");
    } else if is_strikehub_mode {
        tracing::info!("StrikeHub mode: will register with gateway and serve liveview");
    }

    let ipc = dioxus_ipc();

    match ipc_http_get(ipc, "/health").await {
        Ok((status, _, _)) if status == 200 => {
            tracing::info!("Dioxus liveview server is ready ({})", ipc);
        }
        Ok((status, _, _)) => {
            tracing::error!("Dioxus health check returned status {}", status);
        }
        Err(e) => {
            tracing::error!("Failed to health-check Dioxus server: {}", e);
        }
    }

    // Build config from environment
    // Preserve the original URL with scheme for transport auto-detection (wss:// vs grpcs://).
    // ConnectorConfig::from_env() strips the scheme, storing only host:port in config.host.
    let connector_url = std::env::var("STRIKE48_URL")
        .or_else(|_| std::env::var("STRIKE48_HOST"))
        .unwrap_or_default();
    let mut config = ConnectorConfig::from_env();
    config.connector_type = "app-kube-studio".to_string();

    if let Ok(instance_id) = std::env::var("INSTANCE_ID") {
        config.instance_id = instance_id;
    }

    // Store tenant_id in the global session so the ChatPanel (liveview)
    // can read it when auto-creating the agent persona.
    ks_ui::session::set_tenant_id(&config.tenant_id);

    let permission_mode = get_permission_mode();
    let mode_str = match permission_mode {
        PermissionMode::ReadOnly => "READ-ONLY (view)",
        PermissionMode::ReadWrite => "READ-WRITE (cluster-admin)",
    };

    tracing::info!("Registering with Matrix as APP + TOOL connector...");
    tracing::info!("  - Host: {}", config.host);
    tracing::info!("  - Tenant: {}", config.tenant_id);
    tracing::info!("  - Instance: {}", config.instance_id);
    tracing::info!(
        "  - Permission Mode: {} (set KUBESTUDIO_MODE=read|write to change)",
        mode_str
    );
    tracing::info!("  - Behaviors: APP (UI), TOOL (AI agent tools)");
    tracing::info!(
        "  - Tools: list_clusters, get_current_context, get_cluster_info, get_permissions"
    );
    tracing::info!("  - Tools: toolbox_status, toolbox_deploy, toolbox_exec, toolbox_delete");

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("Received shutdown signal (Ctrl+C)");
        shutdown_clone.store(true, Ordering::SeqCst);
    });

    // Clean up any orphaned toolbox from a previous crashed session
    if let Ok(config) = auth::load_kubeconfig(None).await
        && let Some(context_name) = auth::current_context(&config)
    {
        match KubeClient::from_context(&context_name).await {
            Ok(kube_client) => {
                if let Err(e) = cleanup_orphaned_toolbox(kube_client.client().clone()).await {
                    tracing::debug!("No orphaned toolbox to clean up: {}", e);
                }
            }
            Err(e) => {
                tracing::debug!("Could not connect to cluster for orphan cleanup: {}", e);
            }
        }
    }

    // Try to load saved credentials first
    let mut auth_context: Option<AuthContext> = None;
    {
        let mut ott_provider = OttProvider::new(
            Some(config.connector_type.clone()),
            Some(config.instance_id.clone()),
        );
        if ott_provider
            .load_saved_credentials(&config.connector_type, Some(&config.instance_id))
            .is_some()
        {
            tracing::info!("Loaded saved credentials, will use JWT authentication");
            auth_context = Some(AuthContext { ott_provider });
        }
    }

    // Connection loop - handles reconnection after OTT auth
    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        // Get JWT token if we have credentials
        let jwt_token = if let Some(ref mut ctx) = auth_context {
            match ctx.ott_provider.get_token().await {
                Ok(token) => {
                    tracing::info!("Got access token via OttProvider");
                    Some(token)
                }
                Err(e) => {
                    tracing::error!("Failed to get access token: {}", e);
                    // Clear auth context and try fresh registration
                    auth_context = None;
                    None
                }
            }
        } else {
            None
        };

        // Create client and connect (SDK auto-detects transport from URL scheme)
        #[allow(deprecated)]
        let mut client = ConnectorClient::with_options(ClientOptions {
            url: Some(connector_url.clone()),
            ..Default::default()
        });

        if let Err(e) = client.connect_channel().await {
            tracing::error!("Failed to connect: {}", e);
            if sleep_with_shutdown(tokio::time::Duration::from_secs(5), &shutdown).await {
                break;
            }
            continue;
        }

        tracing::info!("Connected to Matrix, starting stream...");

        // Build registration message (with JWT if we have one)
        let registration_msg = build_registration_message(&config, jwt_token.as_deref());

        // Start bidirectional stream
        let (tx, rx) = match client
            .start_stream_with_registration(registration_msg)
            .await
        {
            Ok(streams) => streams,
            Err(e) => {
                tracing::error!("Failed to start stream: {}", e);
                if sleep_with_shutdown(tokio::time::Duration::from_secs(5), &shutdown).await {
                    break;
                }
                continue;
            }
        };

        // Create connector with outbound channel
        let connector = Arc::new(StudioKubeConnector::new(tx));
        connector
            .shutdown
            .store(shutdown.load(Ordering::SeqCst), Ordering::SeqCst);

        tracing::info!("Waiting for registration response...");

        // Run message loop
        match run_message_loop(connector, rx, &config, &shutdown).await {
            MessageLoopResult::Reconnect(ctx) => {
                tracing::info!("Reconnecting with JWT authentication...");
                auth_context = Some(ctx);
                if sleep_with_shutdown(tokio::time::Duration::from_millis(500), &shutdown).await {
                    break;
                }
            }
            MessageLoopResult::Exit => {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
                if auth_context.is_some() {
                    // We have credentials, so reconnect on disconnect
                    tracing::info!("Connection closed, reconnecting...");
                    if sleep_with_shutdown(tokio::time::Duration::from_secs(2), &shutdown).await {
                        break;
                    }
                } else {
                    // No credentials, exit
                    break;
                }
            }
        }
    }

    tracing::info!("Connector shutting down...");

    // Clean up toolbox resources before exit
    tracing::info!("Cleaning up toolbox resources...");
    if let Ok(config) = auth::load_kubeconfig(None).await
        && let Some(context_name) = auth::current_context(&config)
    {
        match KubeClient::from_context(&context_name).await {
            Ok(kube_client) => {
                let toolbox = Toolbox::new(kube_client.client().clone());
                match toolbox.cleanup_all_with_timeout(5).await {
                    Ok(true) => tracing::info!("Toolbox cleanup completed"),
                    Ok(false) => tracing::warn!("Toolbox cleanup timed out"),
                    Err(e) => tracing::warn!("Toolbox cleanup error: {}", e),
                }
            }
            Err(e) => {
                tracing::debug!("Could not connect to cluster for cleanup: {}", e);
            }
        }
    }

    dioxus_handle.abort();
    if let Some(addr) = DIOXUS_IPC.get() {
        addr.cleanup();
    }
    tracing::info!("Shutdown complete");

    Ok(())
}
