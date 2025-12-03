use chrono::Utc;
use redirectionio::http::{PathAndQueryWithSkipped, Request};
use worker::{Request as WorkerRequest, Result};

pub fn create_redirectionio_request(worker_request: &WorkerRequest) -> Result<(Request, Option<String>)> {
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
        path_and_query_skipped: PathAndQueryWithSkipped::from_static(path_and_query.as_str()),
        path_and_query: Some(path_and_query),
        remote_addr: None,
        created_at: Some(Utc::now()),
        sampling_override: None,
    };

    if let Some(remote_addr) = &client_ip {
        if let Ok(ip) = remote_addr.parse::<std::net::IpAddr>() {
            request.remote_addr = Some(ip);
        }
    }

    for (name, value) in worker_request.headers() {
        request.add_header(name, value, false);
    }

    Ok((request, client_ip))
}
