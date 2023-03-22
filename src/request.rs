use chrono::Utc;
use redirectionio::http::{PathAndQueryWithSkipped, Request, TrustedProxies};
use redirectionio::router::RouterConfig;
use worker::{Request as WorkerRequest, Result};

pub fn create_redirectionio_request(worker_request: &WorkerRequest) -> Result<(Request, Option<String>)> {
    let config = RouterConfig::default();
    let url = worker_request.url()?;
    let path_and_query = match url.query() {
        Some(query) => format!("{}?{}", url.path(), query).to_string(),
        None => url.path().to_string(),
    };

    let client_ip = worker_request.headers().get("CF-Connecting-IP")?;

    let mut request = Request {
        headers: Vec::new(),
        host: worker_request.url()?.host().map(|h| h.to_string()),
        method: Some(worker_request.method().to_string()),
        scheme: Some(worker_request.url()?.scheme().to_string()),
        path_and_query_skipped: PathAndQueryWithSkipped::from_config(&config, path_and_query.as_str()),
        path_and_query: Some(path_and_query),
        remote_addr: None,
        created_at: Some(Utc::now()),
        sampling_override: None,
    };

    let trusted_proxies = TrustedProxies::default();

    match &client_ip {
        Some(remote_addr) => request.set_remote_ip(remote_addr.clone(), &trusted_proxies),
        None => (),
    }

    for (name, value) in worker_request.headers() {
        request.add_header(name, value, false);
    }

    Ok((request, client_ip))
}
