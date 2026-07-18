# M3B Rust 自动攻击能力分类

## 状态

- 基线：`agent/rust-combat-data-snapshot`
- 实施状态：分类器、CLI 与 CI 报告已接通，正在执行格式化和最终验证。
- 本阶段只生成候选与拒绝原因，不扩大正式引擎能力。

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

基础命中、伤害、闪避、护甲、元素抗性、增伤、穿透、暴击、攻击速度、HP/MP 修正等直接数值字段可以保留为下一阶段的可实现输入。

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

## 能力声明约束

- 正式 `capabilities` 仍只声明 M2 的单人 tier-0 Fly；
- 候选报告不得进入 `targets`；
- 候选 Zone 只有经过 reference 差分后，才能升级为正式支持。

## 停止标准

1. 分类结果稳定且机器可读；
2. Fly 必须被分类为候选；
3. 每个拒绝 Zone 至少有一个原因；
4. 规则由单元测试覆盖；
5. CLI 能输出候选和拒绝摘要；
6. M0 至 M3A 全部验证继续通过。

达到以上标准后停止，不在本 PR 修改模拟器支持范围。
