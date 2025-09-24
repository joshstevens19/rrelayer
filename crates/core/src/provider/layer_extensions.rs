use alloy::{
    rpc::json_rpc::{RequestPacket, ResponsePacket},
    transports::TransportError,
};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Instant,
};
use tower::{Layer, Service};
use tracing::{error, info};

#[derive(Clone)]
pub struct RpcLoggingLayer {
    rpc_url: String,
}

impl RpcLoggingLayer {
    pub fn new(rpc_url: String) -> Self {
        Self { rpc_url }
    }
}

impl<S> Layer<S> for RpcLoggingLayer {
    type Service = RpcLoggingService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RpcLoggingService { inner, rpc_url: self.rpc_url.clone() }
    }
}

#[derive(Debug, Clone)]
pub struct RpcLoggingService<S> {
    inner: S,
    rpc_url: String,
}

impl<S> Service<RequestPacket> for RpcLoggingService<S>
where
    S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: RequestPacket) -> Self::Future {
        let start_time = Instant::now();
        let rpc_url = self.rpc_url.clone();

        let method_name = match &req {
            RequestPacket::Single(r) => r.method().to_string(),
            RequestPacket::Batch(reqs) => {
                if reqs.is_empty() {
                    "empty_batch".to_string()
                } else if reqs.len() == 1 {
                    reqs[0].method().to_string()
                } else {
                    format!("batch_{}_requests", reqs.len())
                }
            }
        };

        let fut = self.inner.call(req);

        Box::pin(async move {
            match fut.await {
                Ok(response) => {
                    let duration = start_time.elapsed();

                    if duration.as_secs() >= 10 {
                        info!(
                            "SLOW RPC call - method: {}, duration: {:?}, url: {}",
                            method_name, duration, rpc_url
                        );
                    }

                    Ok(response)
                }
                Err(err) => {
                    let duration = start_time.elapsed();
                    let error_str = err.to_string();

                    if error_str.contains("timeout") || error_str.contains("timed out") {
                        error!("RPC TIMEOUT (free public nodes do this a lot consider a using a paid node) - method: {}, duration: {:?}, url: {}, error: {}",
                                       method_name, duration, rpc_url, err);
                    } else if error_str.contains("429") || error_str.contains("rate limit") {
                        // TODO: Sampling this would be nice since this is actually an expected
                        //       part of the flow for many high-throughput applications.
                        error!("RPC RATE LIMITED (free public nodes do this a lot consider a using a paid node) - method: {}, duration: {:?}, url: {}, error: {}",
                                      method_name, duration, rpc_url, err);
                    } else if error_str.contains("connection") || error_str.contains("network") {
                        error!("RPC CONNECTION ERROR (free public nodes do this a lot consider a using a paid node) - method: {}, duration: {:?}, url: {}, error: {}",
                                       method_name, duration, rpc_url, err);
                    } else {
                        error!("RPC ERROR (free public nodes do this a lot consider a using a paid node) - method: {}, duration: {:?}, url: {}, error: {}",
                                       method_name, duration, rpc_url, err);
                    }

                    Err(err)
                }
            }
        })
    }
}
