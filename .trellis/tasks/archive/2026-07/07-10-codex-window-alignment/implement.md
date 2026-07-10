# CodeX Window Alignment 实施计划

## 依赖与范围

单一跨层任务，无需拆分子任务。按以下顺序实施，先完成 core adapter，再接前端条件，最后统一验证。

## Checklist

1. [ ] 在 CodeX quota provider 中提取/暴露可复用的 auth.json 凭据读取能力，确保 quota 与 trigger 不复制解析逻辑。
2. [ ] 扩展 `crates/nexus-core/src/services/provider_trigger.rs`：
   - 增加 CodeX provider 常量与 runner 支持；
   - 请求 `/models?client_version=...` 并映射/过滤稳定模型清单；
   - 构造 `/responses` 最小流式请求并消费 SSE；
   - 映射 CodeX HTTP/SSE/auth/quota/network 错误，保留现有 Claude 行为；
   - 补充纯解析和协议构造测试。
3. [ ] 补充 provider trigger service 测试，验证 CodeX provider 的 supported/active schedule/手动触发路径不回归既有 Claude 调度。
4. [ ] 修改 `src-react/src/components/provider/ProviderPage.tsx`，将 Window Alignment capability、模型查询、保存字段条件从仅 `claude` 扩展为 `claude` 或 `codex`；确认其他 provider 继续写空配置并显示 Coming soon。
5. [ ] 增加或更新前端纯单元/组件测试，覆盖 CodeX capability 条件及保存 payload；必要时只抽取可测试的 provider predicate，不把规则留在 JSX 中。
6. [ ] 运行格式化、Rust 测试、前端 typecheck/unit/component；根据失败回到对应步骤修复。
7. [ ] 做最终跨层审查：serde camelCase、Tauri command DTO、React API 类型、query cache、Provider global resource 语义一致。

## 验证命令

- `cargo fmt --all -- crates/nexus-core/src/services/provider_trigger.rs crates/nexus-core/src/services/provider_quota/providers/codex.rs`
- `pnpm rust:fmt`
- `pnpm rust:test`（Windows，不使用裸 `cargo test -p nexus-core`）
- `cd src-react && pnpm typecheck`
- `cd src-react && pnpm test:unit`
- `cd src-react && pnpm test:component`

## 风险与回滚点

- CodeX `/models` 或 `/responses` 非公开协议变化：所有协议解析限制在 `provider_trigger.rs` 的 CodeX adapter/helper，可单独回退而不影响 scheduler/UI。
- SSE 读取失败或响应体过大：先以完整响应消费保证语义，限制错误详情长度并复用现有日志脱敏；若协议需要真正增量解析，只替换 adapter 读取 helper。
- 凭据读取抽取可能影响 quota：先补保持原行为的测试，再修改调用方；若失败可回滚到单独 helper但不得复制不同解析规则。
- 前端保存若遗漏 CodeX 条件会造成 UI 可选但后端无法激活；必须同时检查 query enabled、triggerSupported 和 save payload 三处。

## Review gate before start

- `prd.md` 已确认用户意图、验收标准和 out-of-scope。
- `design.md` 已确定 CodeX 官方 Responses 协议、错误分类与跨层边界。
- `research/codex-responses-protocol.md` 已记录外部协议证据。
- `implement.jsonl` / `check.jsonl` 已加入相关 backend/frontend/shared spec 与 research 后，才运行 `task.py start`。
