# M2 验证报告

- 分支：`agent/rust-basic-combat`
- 兼容级别：`basic-auto-attack-v1`
- 状态：完成

## 最终 Rust 检查

| 检查 | 结果 |
|---|---|
| cargo fmt --check | 通过 |
| cargo test --workspace --locked | 通过 |
| cargo clippy -D warnings | 通过 |
| Rust DTO validates 14 requests | 通过 |
| zone-solo-basic restricted simulation | 通过 |
| wasm32-unknown-unknown check | 通过 |
| restricted capability report | 通过 |

## JavaScript 基线

上一轮完整链路验证中，下列检查全部通过：

- `npm ci`；
- JavaScript 全量测试；
- 14 组 reference parity goldens；
- reference benchmark smoke test；
- Vite 生产构建。

此后唯一源码改动是 `basic_data.rs` 的 rustfmt 排版，不影响 JavaScript 路径。

## 差分结论

`zone-solo-basic` 的下列字段由 Rust 单元测试逐字段对照 reference golden 并通过：

- encounters；
- deaths；
- 完整 autoAttack histogram；
- simulatedTime；
- lastEncounterFinishTime；
- randomDraws = 145。

M2 受限普通攻击闭环达到停止标准，不声明或冒充完整 `SimulationResultV1`。
