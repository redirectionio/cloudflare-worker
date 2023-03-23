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
            let values = split_set_cookie(value.as_str());

            for value in values {
                headers.push(Header {
                    name: name.to_string(),
                    value,
                });
            }
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

fn split_set_cookie(set_cookie: &str) -> Vec<String> {
    let mut cookies = Vec::new();

    set_cookie.split(',').for_each(|cookie| {
        let pos_equal = cookie.find('=');
        let pos_semi_colon = cookie.find(';');

        let append_to_last = match pos_equal {
            None => true,
            Some(pos_equal) => match pos_semi_colon {
                None => false,
                Some(pos_semi_colon) => pos_equal > pos_semi_colon,
            },
        };

        if append_to_last {
            match cookies.last_mut() {
                None => cookies.push(cookie.trim().to_string()),
                Some(last_cookie) => {
                    last_cookie.push(',');
                    last_cookie.push_str(cookie);
                }
            }
        } else {
            cookies.push(cookie.trim().to_string());
        }
    });

    cookies
}

#[cfg(test)]
mod tests {
    #[test]
    pub fn test_split_cookies() {
        let cookie_str = "sessionid=6ky4pkr7qoi4me7rwleyvxjove25huef, cid=70125eaa-399a-41b2-b235-8a5092042dba; expires=Thu, 04-Jun-2020 12:17:56 GMT; Max-Age=63072000; Path=/; HttpOnly; Secure, client_id=70125eaa-399a-41b2-b235-8a5092042dba; Max-Age=63072000; Path=/; expires=Thu, 04-Jun-2020 12:17:56 GMT";
        let cookies = super::split_set_cookie(cookie_str);

        assert_eq!(cookies.len(), 3);
        assert_eq!(cookies[0], "sessionid=6ky4pkr7qoi4me7rwleyvxjove25huef");
        assert_eq!(
            cookies[1],
            "cid=70125eaa-399a-41b2-b235-8a5092042dba; expires=Thu, 04-Jun-2020 12:17:56 GMT; Max-Age=63072000; Path=/; HttpOnly; Secure"
        );
        assert_eq!(
            cookies[2],
            "client_id=70125eaa-399a-41b2-b235-8a5092042dba; Max-Age=63072000; Path=/; expires=Thu, 04-Jun-2020 12:17:56 GMT"
        );
    }
}
