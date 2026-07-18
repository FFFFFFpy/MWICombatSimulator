# M3B Rust 自动攻击能力分类

## 状态

- 基线：`agent/rust-combat-data-snapshot`
- 实施状态：完成并达到停止标准。
- 本阶段只生成候选与拒绝原因，没有扩大正式引擎能力。

## 目标

基于 `CombatZoneDataSnapshotV1` 自动判断哪些 tier-0 普通 Zone 可以在不实现技能、Buff、反伤、多敌人等机制的前提下，进入下一阶段的单人自动攻击模拟扩展。

## 分类规则

Zone 只有在以下条件全部满足时才是候选：

- Zone Buff 为空；
- `maxSpawnCount = 1`；
- Boss 出生数不超过 1；
- 所有普通与 Boss spawn 的 `difficultyTier = 0`；
- 单只怪物 `strength <= maxTotalStrength`；
- 所有引用怪物存在；
- 怪物在 tier 0 没有主动能力；
- 怪物恰好有一种受支持的 combat style；
- damage type 属于 physical/water/nature/fire；
- 攻击间隔有效；
- 不含反伤、反击、招架、连击、诅咒、狂怒、削弱、吸血、吸蓝、Blaze/Bloom/Ripple 等特殊被动。

基础命中、伤害、闪避、护甲、元素抗性、增伤、穿透、暴击、攻击速度、HP/MP 修正等直接数值字段保留为后续可实现输入。

## 输出

`AutoAttackCapabilityReportV1` 包含：

- 快照版本；
- 候选 Zone 与引用怪物；
- 拒绝 Zone 与结构化原因；
- 原因代码汇总；
- 候选、拒绝和总数。

CLI：

```bash
cargo run -p mwi-sim-cli -- inspect-auto-attack-zones
```

## 分类结果

| 指标 | 数量 |
|---|---:|
| 普通 Zone 总数 | 55 |
| 自动攻击候选 | 1 |
| 拒绝 | 54 |

唯一候选：

- `/actions/combat/fly`

拒绝原因累计：

| 原因 | 次数 |
|---|---:|
| tier-0 主动技能 | 96 |
| 多敌人出生 | 11 |
| 特殊被动 | 6 |

一个 Zone 可能同时产生多个原因或由多个怪物产生同类原因，因此原因次数不等于拒绝 Zone 数。

## 结论

继续泛化纯自动攻击引擎不会扩大正式支持范围。下一阶段优先迁移基础主动技能框架；多敌人和特殊被动随后处理。

## 能力声明约束

- 正式 `capabilities` 仍只声明 M2 的单人 tier-0 Fly；
- 候选报告没有进入 `targets`；
- 候选 Zone 只有经过 reference 差分后才能升级为正式支持。

## 验证结果

- 55 个 Zone 全部分类：通过；
- candidates + rejected = total：通过；
- Fly 为候选：通过；
- 所有拒绝 Zone 至少一个结构化原因：通过；
- higher-tier ability 不误伤 tier-0 候选：通过；
- cargo fmt、workspace tests、Clippy：通过；
- 14 个历史请求 DTO：通过；
- M2 Fly 模拟、WASM 与正式能力报告：通过；
- JavaScript 全量测试、14 组黄金、基准与生产构建：通过。

M3B 到此冻结。后续技能迁移在独立分支实施。
