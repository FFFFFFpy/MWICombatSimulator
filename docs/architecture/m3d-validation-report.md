# M3D 验证报告

- 分支：`agent/rust-direct-damage-runtime`
- 状态：完成
- 正式能力：单人 tier-0 Fly，level-1 Aqua Arrow，空自定义 Trigger
- 结果类型：`direct_damage_combat_result`
- 兼容级别：`direct-damage-cast-v1`

## Reference 差分

Aqua Arrow fixture：`fixtures/parity/ability-aqua-arrow-basic`

Rust 逐字段对照 JavaScript golden 并通过：

- encounters = 5；
- deaths；
- 完整 attacks histogram；
- Aqua Arrow 施放 3 次，每次 34 伤害；
- manaUsed = 105；
- simulatedTime = 60000000000；
- lastEncounterFinishTime = 46158415841.58417；
- randomDraws = 135。

同请求重复运行完全一致。Fireball 等未升格候选会被正式入口明确拒绝。

## 边界

已接通：

- `mwi_sim_core::simulate_direct_damage`；
- CLI `simulate-direct-damage`；
- WASM `simulateDirectDamageJson`；
- 机器可读 capability。

Capability 仅声明 Aqua Arrow level 1、空 Trigger，没有开放其余九个候选技能。

## 最终检查

- Combat Zone 与 ability 快照检查：通过；
- cargo fmt、workspace tests、Clippy：通过；
- 15 个请求 DTO：通过；
- M2 Fly 回归：通过；
- Aqua Arrow CLI/WASM：通过；
- JavaScript 全量测试：通过；
- 15 组 parity goldens：通过；
- benchmark smoke test：通过；
- Vite production build：通过。

M3D 达到停止标准。DOT、Buff、控制、群攻、自定义 Trigger、多玩家和完整 `SimulationResultV1` 不在本阶段范围内。
