//! Optional official rmcp stdio adapter. The owner thread alone holds the runtime.
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt, model::*, service::RequestContext};
use serde_json::json;
use std::{
    pin::Pin,
    sync::mpsc,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, ReadBuf};
use view_automation::{MAX_MESSAGE, Request, Response};

#[derive(Clone)]
struct Adapter {
    sender: mpsc::SyncSender<(Request, tokio::sync::oneshot::Sender<Response>)>,
}
impl ServerHandler for Adapter {
    fn get_info(&self) -> ServerConfig {
        let mut info = ServerConfig::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tool:Tool=serde_json::from_value(json!({"name":"view.request","description":"Versioned view automation request. Discover returns framework operations and registered domain schemas. Session/request IDs, target revisions and scope checks are identical to Rust/CLI. No eval.","inputSchema":{"type":"object","properties":{"version":{"type":"integer","const":1},"session":{"type":"string"},"id":{"type":"integer","minimum":0},"operation":{"type":"object","description":"Tagged op; use discover to inspect supported operations and domain commands."}},"required":["version","session","id","operation"],"additionalProperties":false}})).expect("static tool");
        Ok(ListToolsResult {
            tools: vec![tool],
            ..Default::default()
        })
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if request.name != "view.request" {
            return Err(ErrorData::invalid_params("unknown tool", None));
        }
        let request = match serde_json::from_value::<Request>(serde_json::Value::Object(
            request.arguments.unwrap_or_default(),
        )) {
            Ok(r) => r,
            Err(e) => {
                return Ok(CallToolResult::error(vec![ContentBlock::text(e.to_string())]).into());
            }
        };
        let (tx, rx) = tokio::sync::oneshot::channel();
        if self.sender.try_send((request, tx)).is_err() {
            return Ok(CallToolResult::error(vec![ContentBlock::text(
                "owner queue full or closed; request not accepted",
            )])
            .into());
        }
        match rx.await {
            Ok(response) => {
                let failed = response.result.is_err();
                let content = vec![ContentBlock::text(
                    serde_json::to_string(&response).expect("response"),
                )];
                Ok(if failed {
                    CallToolResult::error(content)
                } else {
                    CallToolResult::success(content)
                }
                .into())
            }
            Err(_) => Ok(CallToolResult::error(vec![ContentBlock::text(
                "unknown_outcome: owner disconnected after acceptance",
            )])
            .into()),
        }
    }
}
/// Bound each JSON-RPC line before rmcp parses it. No HTTP transports are enabled.
struct BoundedRead<R> {
    inner: R,
    line: usize,
}
impl<R: AsyncRead + Unpin> AsyncRead for BoundedRead<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let mut bytes = [0u8; 4096];
        let cap = buf.remaining().min(bytes.len());
        let mut part = ReadBuf::new(&mut bytes[..cap]);
        match Pin::new(&mut this.inner).poll_read(cx, &mut part) {
            Poll::Ready(Ok(())) => {
                for &b in part.filled() {
                    this.line += 1;
                    if this.line > MAX_MESSAGE {
                        return Poll::Ready(Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "MCP line budget",
                        )));
                    }
                    if b == b'\n' {
                        this.line = 0;
                    }
                }
                buf.put_slice(part.filled());
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}
pub fn run(name: String) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = mpsc::sync_channel::<(Request, tokio::sync::oneshot::Sender<Response>)>(32);
    let owner = std::thread::spawn(move || -> Result<(), String> {
        let mut session = crate::fixture(name).map_err(|e| e.to_string())?;
        while let Ok((request, reply)) = rx.recv() {
            let response = session.execute(request);
            let _ = reply.send(response);
        }
        session.disconnect();
        Ok(())
    });
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(async move {
        let (stdin, stdout) = rmcp::transport::stdio();
        let service = Adapter { sender: tx }
            .serve((
                BoundedRead {
                    inner: stdin,
                    line: 0,
                },
                stdout,
            ))
            .await?;
        service.waiting().await?;
        Ok::<_, Box<dyn std::error::Error>>(())
    });
    // Pending handler futures retain sender clones until the runtime drops them.
    drop(runtime);
    owner
        .join()
        .map_err(|_| "owner thread panicked")?
        .map_err(std::io::Error::other)?;
    result
}
