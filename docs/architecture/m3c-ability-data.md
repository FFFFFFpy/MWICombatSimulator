# M3C Rust 技能数据与直接伤害分类

## 状态

- 基线：`agent/rust-auto-attack-capabilities`
- 实施状态：完成。
- 本阶段只建立技能数据边界与候选分类，没有实现施法、冷却、法力消耗或伤害结算。

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

## 单体直接伤害候选规则

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

默认 Trigger 被保留和报告，但本阶段不执行 Trigger。

## 分类结果

| 指标 | 数量 |
|---|---:|
| 技能总数 | 57 |
| 单体直接伤害候选 | 10 |
| 拒绝 | 47 |

候选：

- `/abilities/aqua_arrow`；
- `/abilities/fireball`；
- `/abilities/flame_arrow`；
- `/abilities/impale`；
- `/abilities/poke`；
- `/abilities/quick_shot`；
- `/abilities/scratch`；
- `/abilities/smack`；
- `/abilities/steady_shot`；
- `/abilities/water_strike`。

主要拒绝原因累计：

| 原因 | 次数 |
|---|---:|
| 目标类型不受支持 | 33 |
| Buff | 26 |
| damage type 不受支持或为空 | 23 |
| effect type 非直接伤害 | 23 |
| combat style 不受支持或为空 | 18 |
| DOT | 4 |
| 眩晕 | 4 |
| 破甲 | 2 |
| 致盲 | 2 |
| 穿透 | 2 |
| 沉默 | 2 |
| 多效果 | 2 |
| 吸血 | 1 |

一个技能可能同时产生多个拒绝原因。

## 命令

```bash
npm run build-rust-ability-data
npm run check-rust-ability-data
cargo run -p mwi-sim-cli -- inspect-direct-damage-abilities
```

## 能力声明约束

- 正式引擎 `capabilities` 仍只声明 M2 的单人 tier-0 Fly；
- 候选技能没有进入正式模拟支持列表；
- 只有完成施法、冷却、法力、事件顺序和 reference 差分后，技能才可升级为正式能力。

## 最终验证

- 技能快照字节级 `--check`：通过；
- 同一输入重复生成：一致；
- Rust 读取并验证 57 个技能：通过；
- 57 个技能全部分类：通过；
- Aqua Arrow 为候选：通过；
- cargo fmt、workspace tests、Clippy：通过；
- 14 个历史请求 DTO：通过；
- M2 Fly 模拟、WASM 与正式能力报告：通过；
- JavaScript 全量测试：通过；
- 14 组 reference parity goldens：通过；
- reference benchmark smoke test：通过；
- Vite 生产构建：通过。

## 结论

下一阶段应实现通用单体直接伤害施法核心，而不是只为 Aqua Arrow 写特例。首个 reference 差分仍使用 Aqua Arrow，以锁定施法、法力、冷却、事件顺序和伤害公式；其余候选在同一运行时上逐个升格。
