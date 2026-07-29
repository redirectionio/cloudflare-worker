mod action;
mod proxy;
mod request;

use redirectionio::api::Log;
use worker::{Context, Env, Fetch, Headers, Method, Request, RequestInit, Response, Result, console_log, event, wasm_bindgen::JsValue};

/// What to report to the manager after a request: either a full log, or only the ids of
/// the executed rules (when logging was disabled for the request by a "configuration"
/// action), to count their executions without sending the full log.
enum RequestReport {
    Log(Box<Log>),
    RuleCount(Vec<String>),
}

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

    let report = if action.should_log_request(true, backend_status_code, None) {
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

        Some(RequestReport::Log(Box::new(log)))
    } else if action.agent_supports_rule_count() {
        // Logging was disabled for this request by a "configuration" action: still count
        // the executed rules, without sending the full request log. Only when the agent
        // understands the RULE_COUNT command (protocol >= 1.1), as advertised in the match
        // response; an older agent has no "rule-count" endpoint and would just reject it.
        let rule_ids = action.get_applied_rule_ids_vec();

        if rule_ids.is_empty() {
            None
        } else {
            Some(RequestReport::RuleCount(rule_ids))
        }
    } else {
        // Older agent without rule-count support: nothing to report for this request.
        None
    };

    ctx.wait_until(async move {
        if let Some(cache_future) = cache_future
            && let Err(err) = cache_future.await
        {
            console_log!("error while caching action: {}", err);
        }

        if let Some(report) = report {
            let (path, body_json) = match report {
                RequestReport::Log(log) => ("log", serde_json::to_string(&log)),
                RequestReport::RuleCount(rule_ids) => ("rule-count", serde_json::to_string(&rule_ids)),
            };

            if let Ok(body_json) = body_json {
                let headers = Headers::new();
                headers.set("Content-Type", "application/json").unwrap();
                headers.set("x-redirectionio-instance-name", instance_name.as_str()).unwrap();
                headers
                    .set("User-Agent", format!("cloudflare-worker/{}", version).as_str())
                    .unwrap();

                let body = JsValue::from_str(&body_json);

                let mut request_init = RequestInit::new();

                request_init.with_method(Method::Post).with_headers(headers).with_body(Some(body));

                let report_request =
                    Request::new_with_init(format!("{}/{}/{}", agent_host.as_str(), token, path).as_str(), &request_init).unwrap();

                Fetch::Request(report_request).send().await.unwrap();
            }
        }
    });

    Ok(response)
}
