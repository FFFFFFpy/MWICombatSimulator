# M2 Rust 基础战斗范围

## 状态

- 基线：`agent/rust-core-foundation`
- 目标：建立一个可执行、可拒绝越界输入、可与 JavaScript reference engine 对照的最小普通攻击闭环。
- 非目标：完整 `SimResult`、技能、Buff、Trigger、消耗品、副本、迷宫和 UI 接入。

## 支持范围

M2 只接受满足以下条件的请求：

- `target.kind = zone`；
- 单名玩家；
- 玩家无装备、技能、食物、饮料、房屋、工会 Buff 与成就；
- 难度 0；
- 当前首个支持目标为 `/actions/combat/fly`；
- `statisticsMode = full`；
- 不启用 HP/MP 可视化；
- 不启用事件 trace；
- 随机源为字符串 seeded 或 sequence。

超出范围必须返回明确错误，禁止静默忽略字段或生成部分可信结果。

## 最小战斗闭环

```text
读取请求
→ 校验受限能力
→ 创建玩家与 Fly
→ 安排双方普通攻击
→ 命中、暴击、伤害与减伤
→ 死亡处理
→ 3 秒后刷新下一只 Fly
→ 运行至 simulationTimeLimit
→ 输出 BasicCombatResultV1
```

## BasicCombatResultV1

结果只声明当前已实现字段：

- request ID、引擎与数据版本；
- 模拟时间；
- 遭遇完成数；
- 双方死亡数；
- 双方普通攻击命中、未命中与伤害频数；
- 最后一次遭遇完成时间；
- 随机消费数；
- 事件处理数；
- 兼容级别 `basic-auto-attack-v1`。

它不是 `SimulationResultV1`，不得传给依赖完整 `SimResult` 的现有 UI。

## 验证

1. 单次攻击的命中、暴击、伤害和减伤使用与 reference JS 相同公式。
2. 玩家与 Fly 的攻击间隔一致。
3. 同 seed 重跑完全一致。
4. `zone-solo-basic` 的基础结果子集与 reference golden 一致。
5. 不支持的请求逐类有拒绝测试。
6. CLI 能运行该 fixture，但 capability report 只声明这一受限能力。

达到以上标准后停止，不开始技能或 Buff 迁移。
