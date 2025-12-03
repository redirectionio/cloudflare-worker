mod action;
mod proxy;
mod request;

use redirectionio::api::Log;
use worker::{console_log, event, wasm_bindgen::JsValue, Context, Env, Fetch, Headers, Method, Request, RequestInit, Response, Result};

#[event(fetch)]
pub async fn main(req: Request, env: Env, ctx: Context) -> Result<Response> {
    ctx.pass_through_on_exception();

    let token = env.secret("REDIRECTIONIO_TOKEN")?.to_string();
    let timeout = match env.var("REDIRECTIONIO_TIMEOUT") {
        Ok(timeout) => timeout.to_string().parse::<u64>().unwrap_or(5000),
        Err(_) => 5000,
    };
    let add_headers = match env.var("REDIRECTIONIO_ADD_HEADER_RULE_IDS") {
        Ok(add_headers) => add_headers.to_string() == "true",
        Err(_) => false,
    };
    let version = env
        .var("REDIRECTIONIO_VERSION")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "redirection-io-cloudflare/dev".to_string());
    let instance_name = env
        .var("REDIRECTIONIO_INSTANCE_NAME")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "undefined".to_string());
    let agent_host = env
        .var("REDIRECTIONIO_AGENT_HOST")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "https://agent.redirection.io".to_string());
    let cache_time = match env.var("REDIRECTIONIO_CACHE_TIME") {
        Ok(timeout) => timeout.to_string().parse::<u64>().unwrap_or(0),
        Err(_) => 0,
    };

    let start_time = chrono::Utc::now().timestamp_millis() as u128;
    let (request, client_ip) = request::create_redirectionio_request(&req)?;
    let (mut action, cache_future) =
        action::get_action(&request, agent_host.as_str(), &token, &instance_name, &version, cache_time, timeout).await?;
    let action_match_time = chrono::Utc::now().timestamp_millis() as u128;
    let (response, filtered_headers, backend_status_code) = proxy::proxy(req, &mut action, add_headers).await?;
    let response_time = chrono::Utc::now().timestamp_millis() as u128;

    let log_request = if action.should_log_request(true, backend_status_code, None) {
        let log = Log::from_proxy(
            &request,
            response.status_code(),
            &filtered_headers,
            action.get_applied_rule_ids_vec(),
            format!("cloudflare-worker/{}", version).as_str(),
            start_time,
            action_match_time,
            Some(response_time),
            client_ip.unwrap_or_default().as_str(),
        );

        Some(log)
    } else {
        None
    };

    ctx.wait_until(async move {
        if let Some(cache_future) = cache_future {
            if let Err(err) = cache_future.await {
                console_log!("error while caching action: {}", err);
            }
        }

        if let Some(log) = log_request {
            let headers = Headers::new();
            headers.set("Content-Type", "application/json").unwrap();
            headers.set("x-redirectionio-instance-name", instance_name.as_str()).unwrap();
            headers
                .set("User-Agent", format!("cloudflare-worker/{}", version).as_str())
                .unwrap();

            if let Ok(log_json) = serde_json::to_string(&log) {
                let body = JsValue::from_str(&log_json);

                let mut request_init = RequestInit::new();

                request_init.with_method(Method::Post).with_headers(headers).with_body(Some(body));

                let log_request = Request::new_with_init(format!("{}/{}/log", agent_host.as_str(), token).as_str(), &request_init).unwrap();

                Fetch::Request(log_request).send().await.unwrap();
            }
        }
    });

    Ok(response)
}
