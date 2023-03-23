## Cloudlfare worker for redirection.io

Look at our documentation about cloudflare integration here : [https://redirection.io/documentation/developer-documentation/cloudflare-workers-integration](https://redirection.io/documentation/developer-documentation/cloudflare-workers-integration)

### Pushing to cloudflare

You can also directly push this repository to a cloudflare worker, but you will still need a redirection io account to do so:

1. You need to have wrangler installed: `npm install -g wrangler`
2. Login or configure your api token for cloudflare `wrangler login`
3. Copy file `wrangler.toml.dist` to `wrangler.toml` and replace needed value
4. Push your redirection io token as a secret value `wrangler secret put REDIRECTIONIO_TOKEN` and enter you redirection io project key when asked (available in your the instance panel of your project)
5. Publish your worker: `wrangler publish`

## License

This code is licensed under the MIT License - see the  [LICENSE](./LICENSE.md)  file for details.
