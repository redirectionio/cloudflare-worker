## Cloudlfare worker for redirection.io

Look at our documentation about our Cloudflare integration here: [https://redirection.io/documentation/developer-documentation/cloudflare-workers-integration](https://redirection.io/documentation/developer-documentation/cloudflare-workers-integration)

### Pushing to Cloudflare

You can also directly push this repository to a cloudflare worker, but you will still need a redirection.io account to do so:

1. You need to have wrangler 2 installed: `npm install -g wrangler`
2. Login or configure the Cloudflare API token: `wrangler login`
3. Copy the file `wrangler.toml.dist` to `wrangler.toml`, and replace the value
4. Push the redirection.io project key as a secret value: `wrangler secret put REDIRECTIONIO_TOKEN` (enter the project key when asked to do so - this key can be found on the instance panel in the manager)
5. Publish the worker: `wrangler publish`

## License

This code is licensed under the MIT License - see the  [LICENSE](./LICENSE.md)  file for details.
