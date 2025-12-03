use std::{
    collections::hash_map::DefaultHasher,
    future::Future,
    hash::{Hash, Hasher},
    pin::Pin,
    time::Duration,
};

use futures_util::future::Either;
use redirectionio::{action::Action, http::Request};
use worker::{
    wasm_bindgen::JsValue, AbortController, Cache, Delay, Fetch, Headers, Method, Request as WorkerRequest, RequestInit, Response, Result,
};

pub async fn get_action(
    request: &Request,
    agent_host: &str,
    token: &str,
    instance_name: &str,
    version: &str,
    cache_time: u64,
    timeout: u64,
) -> Result<(Action, Option<Pin<Box<dyn Future<Output = Result<()>>>>>)> {
    let cache = Cache::default();
    let mut hasher = DefaultHasher::new();
    request.hash(&mut hasher);

    let cache_key = format!("{}/{}/action/{}", agent_host, token, hasher.finish());

    match cache.get(&cache_key, true).await? {
        Some(mut response) => Ok((response.json::<Action>().await?, None)),
        None => {
            let headers = Headers::new();
            headers.set("Content-Type", "application/json")?;
            headers.set("x-redirectionio-instance-name", instance_name)?;
            headers.set("User-Agent", format!("cloudflare-worker/{}", version).as_str())?;

            let body = JsValue::from_str(&serde_json::to_string(&request)?);

            let mut request_init = RequestInit::new();

            request_init.with_method(Method::Post).with_headers(headers).with_body(Some(body));

            let action_request = WorkerRequest::new_with_init(format!("{}/{}/action", agent_host, token).as_str(), &request_init)?;

            let controller = AbortController::default();
            let signal = controller.signal();

            let fetch_fut = async { Fetch::Request(action_request).send_with_signal(&signal).await };
            let delay_fut = async {
                Delay::from(Duration::from_millis(timeout)).await;
                controller.abort();
                Response::ok("Cancelled")
            };

            futures_util::pin_mut!(fetch_fut);
            futures_util::pin_mut!(delay_fut);

            let mut response = match futures_util::future::select(delay_fut, fetch_fut).await {
                Either::Left(_) => {
                    return Ok((Action::default(), None));
                }
                Either::Right((res, _)) => res?,
            };

            let body = response.bytes().await?;

            let action = serde_json::from_slice(body.as_slice())?;

            if cache_time > 0 {
                let mut cache_response = Response::from_bytes(body.clone())?;
                cache_response.headers_mut().set("Content-Type", "application/json")?;
                cache_response
                    .headers_mut()
                    .set("Cache-Control", format!("public, max-age={}", cache_time).as_str())?;

                let cache_future = Box::pin(async move { cache.put(&cache_key, cache_response).await });

                return Ok((action, Some(cache_future)));
            }

            Ok((action, None))
        }
    }
}
