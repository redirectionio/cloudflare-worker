/* attaching the event listener */
addEventListener('fetch', event => {
    event.passThroughOnException();
    event.respondWith(redirectionio_fetch(event.request, event));
});

async function redirectionio_fetch(request, event) {
    const options = {
        token: REDIRECTIONIO_TOKEN || null,
        timeout: parseInt(REDIRECTIONIO_TIMEOUT, 10),
        add_rule_ids_header: REDIRECTIONIO_ADD_HEADER_RULE_IDS === 'true',
        version: REDIRECTIONIO_VERSION || 'redirection-io-cloudflare/dev',
        instance_name: REDIRECTIONIO_INSTANCE_NAME || 'undefined',
        cache_time: REDIRECTIONIO_CACHE_TIME ? parseInt(REDIRECTIONIO_CACHE_TIME, 10) : 0,
    }

    if (options.token === null) {
        return await fetch(request);
    }

    const libredirectionio = wasm_bindgen;
    await wasm_bindgen(wasm);

    const clientIP = request.headers.get("CF-Connecting-IP");
    const redirectionioRequest = create_redirectionio_request(request, libredirectionio, clientIP);
    const [action, registerCachePromise] = await get_action(request, redirectionioRequest, options, libredirectionio);
    const [response, backendStatusCode] = await proxy(request, redirectionioRequest, action, options, libredirectionio);

    event.waitUntil(async function () {
        if (registerCachePromise !== null) {
            await registerCachePromise;
        }

        await log(response, backendStatusCode, redirectionioRequest, action, libredirectionio, options, clientIP || "");
    }());

    return response;
}

function create_redirectionio_request(request, libredirectionio, clientIp) {
    const urlObject = new URL(request.url);
    const redirectionioRequest = new libredirectionio.Request(urlObject.pathname + urlObject.search, urlObject.host, urlObject.protocol.includes('https') ? 'https' : 'http', request.method);

    for (const pair of request.headers.entries()) {
        // remove cloudflare specific headers
        if (pair[0] === "cf-request-id") {
            continue;
        }

        if (pair[0] === "cf-ray") {
            continue;
        }

        if (pair[0] === "cf-visitor") {
            continue;
        }

        redirectionioRequest.add_header(pair[0], pair[1]);
    }

    if (clientIp) {
        redirectionioRequest.set_remote_ip(clientIp);
    }

    return redirectionioRequest;
}

async function get_action(request, redirectionioRequest, options, libredirectionio) {
    const cache = caches.default;
    const cacheUrl = new URL(request.url)
    cacheUrl.pathname = "/get-action/" + redirectionioRequest.get_hash().toString()

    // Convert to a GET to be able to cache
    const cacheKey = new Request(cacheUrl.toString(), {
        headers: [],
        method: "GET",
    });

    let response = await cache.match(cacheKey);
    let registerCachePromise = null;
    let actionStr = '';

    if (!response) {
        response = await Promise.race([
            fetch('https://agent.redirection.io/' + options.token + '/action', {
                method: 'POST',
                body: redirectionioRequest.serialize().toString(),
                headers: {
                    'User-Agent': 'cloudflare-worker/' + options.version,
                    'x-redirectionio-instance-name': options.instance_name,
                },
            }),
            new Promise((_, reject) =>
                setTimeout(() => reject(new Error('Timeout')), options.timeout)
            ),
        ]);

        actionStr = await response.text();

        if (options.cache_time > 0) {
            const cacheResponse = new Response(new Blob([actionStr], { type: "application/json" }), response);
            cacheResponse.headers.append("Cache-Control", `public, max-age=${options.cache_time}`);

            registerCachePromise = cache.put(cacheKey, cacheResponse);
        }
    } else {
        actionStr = await response.text();
    }

    if (actionStr === "") {
        return [libredirectionio.Action.empty(), registerCachePromise]
    }

    try {
        return [new libredirectionio.Action(actionStr), registerCachePromise];
    } catch (e) {
        console.error(e);

        return [libredirectionio.Action.empty(), registerCachePromise];
    }
}

/* Redirection.io logic */
async function proxy(request, redirectionioRequest, action, options, libredirectionio) {
    try {
        const statusCodeBeforeResponse = action.get_status_code(0);

        let response;

        if (statusCodeBeforeResponse === 0) {
            response = await fetch(request);
        } else {
            response = new Response('', {
                status: Number(statusCodeBeforeResponse),
            });
        }

        const backendStatusCode = response.status;
        const statusCodeAfterResponse = action.get_status_code(backendStatusCode);

        if (statusCodeAfterResponse !== 0) {
            response.status = Number(statusCodeAfterResponse);
        }

        const headerMap = new libredirectionio.HeaderMap();

        for (const pair of response.headers.entries()) {
            headerMap.add_header(pair[0], pair[1]);
        }

        const newHeaderMap = action.filter_headers(headerMap, backendStatusCode, options.add_rule_ids_header);
        const newHeaders = new Headers();

        for (let i = 0; i < newHeaderMap.len(); i++) {
            newHeaders.append(newHeaderMap.get_header_name(i), newHeaderMap.get_header_value(i));
        }

        response = new Response(response.body, {
            status: response.status,
            statusText: response.statusText,
            headers: newHeaders,
        });

        const bodyFilter = action.create_body_filter(backendStatusCode);

        // Skip body filtering
        if (bodyFilter.is_null()) {
            return [response, response.status];
        }

        const { readable, writable } = new TransformStream();

        filter_body(response.body, writable, bodyFilter);

        return [new Response(readable, response), backendStatusCode];
    } catch (err) {
        console.error(err);
        const response = await fetch(request);

        return [response, response.status];
    }
}

async function filter_body(readable, writable, bodyFilter) {
    let writer = writable.getWriter();
    let reader = readable.getReader();
    let data = await reader.read();

    while (!data.done) {
        const filteredData = bodyFilter.filter(data.value);

        if (filteredData) {
            await writer.write(filteredData);
        }

        data = await reader.read();
    }

    const lastData = bodyFilter.end();

    if (lastData) {
        await writer.write(lastData);
    }

    await writer.close();
}

async function log(response, backendStatusCode, redirectionioRequest, action, libredirectionio, options, clientIP) {
    if (response === null) {
        return;
    }

    const timestamp = Date.now();
    const responseHeaderMap = new libredirectionio.HeaderMap();

    for (const pair of response.headers.entries()) {
        responseHeaderMap.add_header(pair[0], pair[1]);
    }

    if (action && !action.should_log_request(backendStatusCode)) {
        return;
    }

    try {
        const logAsJson = libredirectionio.create_log_in_json(
            redirectionioRequest,
            response.status,
            responseHeaderMap,
            action,
            'cloudflare-worker/' + options.version,
            BigInt(timestamp),
            clientIP,
        );

        return await fetch(
            'https://agent.redirection.io/' + options.token + '/log',
            {
                method: 'POST',
                body: logAsJson,
                headers: {
                    'User-Agent': 'redirectionio-cloudflare:' + options.version,
                    'x-redirectionio-instance-name': options.instance_name,
                },
            }
        );
    } catch (err) {
        console.error(err);
    }
}
