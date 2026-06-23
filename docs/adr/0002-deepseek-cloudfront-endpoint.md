# DeepSeek 余额 API 使用 CloudFront 端点

部分企业 DNS 环境将 `api.deepseek.com` 解析到腾讯 EdgeOne CDN 的 IP 地址（`58.49.197.113`、`183.131.191.171`），这些 IP 的 TLS 443 端口不可达，导致 `reqwest` 报 `error sending request for url`。公网 DNS（如 `8.8.8.8`）却将该域名解析到 AWS CloudFront 分发域名 `d3bbv8sr76az5s.cloudfront.net`（`3.173.21.63`），该端点直连和代理均可正常访问。

我们决定将 DeepSeek 余额 API 的请求 URL 从 `https://api.deepseek.com/user/balance` 改为 `https://d3bbv8sr76az5s.cloudfront.net/user/balance`，并通过 `Host: api.deepseek.com` 请求头保持正确的路由。

## Considered Options

- 继续使用 `api.deepseek.com` 并依赖 `system-proxy`：代理可以建立 CONNECT 隧道，但代理自身也使用公司 DNS，解析到同一组被阻断的 IP，TLS 握手仍然失败。
- 使用 `hickory-dns` 特性在 reqwest 层做 DNS-over-HTTPS：可以绕过公司 DNS，但代理的 CONNECT 隧道仍会使用代理自身的 DNS 解析，无法解决问题；同时引入额外依赖和复杂度。
- 运行时解析 CloudFront IP 并通过 `ClientBuilder::resolve` 硬编码：IP 可能变更，且 `resolve` 方法不影响代理的 CONNECT 目标。
- 使用 CloudFront 分发域名并设置 `Host` 头：已验证直连和代理均可达，CloudFront 分发域名是 DeepSeek 官方 CDN 配置，稳定可靠；不引入额外依赖。选用该方案。

## Consequences

- `fetch_deepseek_balance` 函数每次请求额外携带 `Host: api.deepseek.com` 头，确保 CloudFront 正确路由到 DeepSeek 源站。
- 若 DeepSeek 更换 CDN 或 CloudFront 分发域名，需更新 `DEEPSEEK_BALANCE_URL` 常量。
- 该方案不影响其他 provider 的 API 调用。
- `system-proxy` 特性仍然启用，对其它 provider（Claude、OpenRouter 等）的代理场景有价值。

## Implementation Notes

- URL 常量定义于 `crates/nexus-core/src/services/provider_quota.rs:42`。
- `Host` 头通过 `reqwest::RequestBuilder::header("Host", "api.deepseek.com")` 设置，`reqwest` 的 `header()` 方法使用 `insert` 语义，可以覆盖 URL 自动推导的 Host 头。
- 30 秒超时通过 `http_client()` 辅助函数统一设置，覆盖所有 provider 的 HTTP 请求。