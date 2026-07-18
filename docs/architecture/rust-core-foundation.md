# Rust 模拟核心基础

## 状态

- 阶段：M1
- 分支：`agent/rust-core-foundation`
- PR：Draft / stacked on `agent/platform-foundation`
- 战斗模拟能力：未实现

## 目标

M1 只建立 Rust 计算核心的可验证地基：协议 DTO、时间、随机源、稳定事件队列、WASM/CLI 边界和 CI。

它不实现任何战斗公式，也不返回伪造的 `SimulationResultV1`。Cargo workspace 能编译并不代表战斗迁移完成，人类已经过度奖励“程序启动了”这种成就。

## Workspace

```text
Cargo.toml
Cargo.lock
rust-toolchain.toml
crates/
  sim-core/
  sim-wasm/
  sim-cli/
```

### `mwi-sim-core`

纯 Rust 基础库，不依赖浏览器、Node、Vue、Worker 或网络。

当前模块：

- `contracts`：`SimulationRequestV1` 及关联 DTO；
- `time`：有限、非负、全序的浮点纳秒时间；
- `rng`：显式数列 RNG 和字符串种子 JS 兼容 RNG；
- `event_queue`：同时间 FIFO、句柄取消和惰性失效；
- `error`：协议与基础运行错误。

### `mwi-sim-wasm`

当前只提供：

- 能力报告；
- 请求 JSON 校验与规范化。

能力报告明确包含：

```json
{
  "combatSimulation": false,
  "targets": [],
  "statisticsModes": [],
  "eventTrace": false
}
```

它不会把“可以读请求”冒充“可以运行战斗”。

### `mwi-sim-cli`

当前命令：

```text
capabilities
validate-request <request.json>
validate-fixtures <fixtures/parity>
rng-seeded <seed> <count>
```

CLI 只验证基础设施，不运行模拟。

## 协议 DTO

Rust `SimulationRequestV1`：

- 校验 `contractVersion == 1`；
- 校验 request ID、data version、玩家列表和正模拟时长；
- 强类型区分 Zone 与 Labyrinth；
- 支持数组或计数形式的 Labyrinth crates；
- 支持 native、seeded 和 sequence 随机配置；
- 支持 full/fast 与 basic/combat；
- 玩家内部结构暂时保留为原始 JSON；
- 顶层、options 和 target 的未知扩展字段保留；
- 不在尚未实现玩家模型时丢弃未来字段。

M1 CI 会用 CLI 校验当前 14 个 `fixtures/parity/*/request.json`。

## 时间模型

旧 JavaScript 引擎会因 haste 产生小数纳秒事件时间，因此 M1 不把时间擅自改成整数。

`SimTime`：

- 使用 `f64` 保存纳秒；
- 拒绝 NaN、Infinity 和负数；
- 规范化 `-0`；
- 使用 `total_cmp` 提供稳定全序；
- 保留小数纳秒。

后续若要迁移到定点整数，必须先证明所有黄金轨迹可无损转换，不能为了类型整齐改写事件顺序。

## 随机源

### Sequence RNG

- 输入必须为非空 `[0, 1)` 数列；
- 支持有限和循环模式；
- 有限模式耗尽时报错；
- 耗尽失败不增加 draw count；
- 用于 Rust 与 JS 的首批严格差分。

### Seeded RNG

当前实现复制 reference JS 的：

- UTF-16 code unit FNV-1a seed hash；
- 32 位 wrapping 运算；
- Mulberry32 输出过程。

M1 精确兼容范围为**字符串 seed**。当前 14 个已提交 fixture 的 seeded 配置均使用字符串。

非字符串 seed 暂时返回明确错误。原因是 JavaScript `JSON.stringify` 对对象键顺序和数字格式的细节不能用普通 Rust JSON 序列化假装等价。后续若实现，必须加入跨语言固定向量测试。

## 稳定事件队列

`StableEventQueue<T>` 当前保证：

- 更早时间先执行；
- 同时间按插入 sequence FIFO；
- 每个事件返回 `EventHandle`；
- 取消即时从 live count 移除；
- heap 节点惰性失效，pop/peek 时跳过；
- 已消费或已取消句柄不能再次取消其他事件。

M1 尚未定义攻击、Buff 或其他战斗事件 payload。队列只提供基础调度语义。

## CI

Rust job 与现有 JavaScript M0 job 并行运行。

Rust 验证：

```text
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p mwi-sim-cli -- validate-fixtures fixtures/parity
cargo check -p mwi-sim-wasm --target wasm32-unknown-unknown
cargo run -p mwi-sim-cli -- capabilities
```

CI 上传：

- 14 个 fixture 的 Rust 校验摘要；
- Rust foundation capability report；
- 测试失败时的原始 Cargo 日志。

`Cargo.lock` 已提交。用于生成 lockfile 的临时写权限工作流已经自删除，永久 CI 仍为 `contents: read`。

## M1 成功标准

- JS M0 全部验证继续通过；
- Rust fmt、test、Clippy 全绿；
- 14 个请求 fixture 全部可读取；
- string seeded RNG 与 JS 固定向量一致；
- sequence RNG 循环与耗尽语义一致；
- 同时间事件 FIFO；
- 取消与惰性失效通过测试；
- WASM target 可检查；
- CLI/WASM 能力报告明确 `combatSimulation: false`。

达到这些标准后 M1 停止。M2 才开始单位基础状态、普通攻击、命中、伤害和普通 Zone。
