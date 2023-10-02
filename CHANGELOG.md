## 2.7.0

 * Update to libredirectionio 2.9.0

## 2.6.0

 * Rewrite worker to use wrangler 2
 * Use last version of libredirection (2.7.1)

## 2.4.3

 * Force libredirectionio to use with this worker
 * Disable gzip compression on libredirectionio as it's already handled by cloudflare

## 2.4.2

 * Fix a bug in filtering where getting the Set-Cookie header would provide a bad value

## 2.4.1 - 11-10-2022

 * Fix a bug in filtering where getting the `Set-Cookie` header would provide a bad value

## 2.4.0 - 07-07-2022

 * Fix a bug when multiple rules where used with a backend status code trigger
