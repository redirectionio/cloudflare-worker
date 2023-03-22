use redirectionio::action::Action;
use redirectionio::http::Header;
use worker::{Fetch, Headers, Request as WorkerRequest, Response, ResponseBody, Result};

pub async fn proxy(worker_request: WorkerRequest, action: &mut Action, add_rules_id_header: bool) -> Result<(Response, Vec<Header>, u16)> {
    let mut response = match action.get_status_code(0, None) {
        0 => Fetch::Request(worker_request).send().await?,
        status_code => Response::empty()?.with_status(status_code),
    };

    let backend_status_code = response.status_code();
    let status_code_after_response = action.get_status_code(backend_status_code, None);

    if status_code_after_response > 0 {
        response = response.with_status(status_code_after_response);
    }

    let mut headers = Vec::new();

    for (name, value) in response.headers() {
        if name.to_lowercase() == "set-cookie" {
            continue; // @TODO Split set cookie
        } else {
            headers.push(Header { name, value });
        }
    }

    let filtered_headers = action.filter_headers(headers, backend_status_code, add_rules_id_header, None);
    let mut response_headers = Headers::new();

    for header in &filtered_headers {
        response_headers.append(header.name.as_str(), header.value.as_str())?;
    }

    response = response.with_headers(response_headers);

    match action.create_filter_body(backend_status_code, &filtered_headers) {
        None => Ok((response, filtered_headers, backend_status_code)),
        Some(mut filter_body) => {
            // @TODO Use a stream body
            let body = response.bytes().await?;
            let mut filtered_body_data = filter_body.filter(body, None);
            filtered_body_data.extend_from_slice(filter_body.end(None).as_slice());

            let filtered_body = ResponseBody::Body(filtered_body_data);
            let mut response_filtered = Response::from_body(filtered_body)?;

            response_filtered = response_filtered.with_status(response.status_code());
            response_filtered = response_filtered.with_headers(response.headers().clone());

            Ok((response_filtered, filtered_headers, backend_status_code))
        }
    }
}
