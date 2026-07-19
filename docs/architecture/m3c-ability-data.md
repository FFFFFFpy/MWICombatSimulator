# M3C Rust 技能数据与直接伤害分类

## 状态

- 基线：`agent/rust-auto-attack-capabilities`
- 实施状态：技能快照、Rust DTO 与分类器测试已修复，正在执行最终 Rust 与 JavaScript 全链路验证。
- 本阶段只建立技能数据边界与候选分类，不实现施法、冷却、法力消耗或伤害结算。

## 目标

将 `abilityDetailMap.json` 编译为确定性、可追溯、可由 Rust 强类型读取的版本化技能快照，并自动识别可以作为首批 Rust 技能运行时目标的“单体直接伤害技能”。

## 输入

- `src/combatsimulator/data/abilityDetailMap.json`。

## 输出

- `crates/sim-core/data/ability-data-v1.json`；
- `AbilityDataSnapshotV1`；
- `DirectDamageAbilityReportV1`。

## 快照内容

每个技能保留：

- HRID、名称、描述、排序索引；
- 是否特殊技能；
- 法力、冷却和施法时间；
- 有序效果列表；
- 有序默认 Trigger 列表；
- 尚未强类型化的扩展字段。

每个效果强类型化：

- target/effect/combat style/damage type；
- 基础伤害、等级成长和命中加成；
- DOT、破甲、吸血、穿透；
- 致盲、沉默、眩晕；
- 消耗生命；
- Buff DTO 与扩展字段。

数组顺序保持原数据顺序；映射键和对象键稳定排序。

## 单体直接伤害候选

技能只有在以下条件全部满足时才是候选：

- 只有一个效果；
- `effectType = /ability_effect_types/damage`；
- `targetType = enemy`；
- combat style 与 damage type 在当前基础伤害公式支持范围内；
- 无 Buff；
- 无 DOT；
- 无破甲；
- 无吸血；
- 无穿透；
- 无致盲、沉默或眩晕；
- 无生命消耗；
- 无未知、非零的效果扩展字段。

默认 Trigger 可以保留并报告，但本阶段不执行 Trigger。

预期样例：`/abilities/aqua_arrow` 应成为候选。分类规则不得按 HRID 白名单实现。

## 命令

```bash
npm run build-rust-ability-data
npm run check-rust-ability-data
cargo run -p mwi-sim-cli -- inspect-direct-damage-abilities
```

## 能力声明约束

- 正式引擎 `capabilities` 保持不变；
- 候选技能不得进入正式模拟支持列表；
- 只有完成施法、冷却、法力、事件顺序和 reference 差分后，技能才可升级为正式能力。

## 停止标准

1. 同一输入重复生成字节一致快照；
2. `--check` 能检测陈旧或篡改快照；
3. Rust 能读取并验证全部技能；
4. 映射键、HRID、数值范围和效果结构有效；
5. 所有技能均被分类为候选或带结构化原因的拒绝项；
6. Aqua Arrow 为候选；
7. M0 至 M3B 全部验证继续通过。

达到以上标准后停止，不在本 PR 实现技能运行时。
