# M3A Rust 普通区数据快照

## 状态

- 基线：`agent/rust-basic-combat`
- 实施状态：完成并达到停止标准。
- 本阶段只建立数据边界，没有扩大 Rust 模拟能力。

## 目标

将当前 JavaScript 游戏数据中的普通 Combat Zone 与其引用怪物，编译为确定性、可追溯、可由 Rust 强类型读取的版本化快照。

## 输入

- `src/combatsimulator/data/actionDetailMap.json`；
- `src/combatsimulator/data/combatMonsterDetailMap.json`。

## 输出

`crates/sim-core/data/combat-zone-data-v1.json`

快照包含：

- schema 与游戏数据版本；
- 两个源文件的 SHA-256；
- 55 个非 Dungeon Combat Zone；
- Zone 的人数上限、难度、Buff、加权随机出生规则和 Boss 出生规则；
- 54 个被普通区引用的怪物；
- 怪物基础等级、基础 combat stats、能力 DTO、经验、狂暴时间和掉落 DTO；
- 确定排序和规范 JSON 表示。

数组顺序保持源数据顺序，因为 spawn 顺序会影响加权选择；对象键和映射键使用稳定排序。

## 生成命令

```bash
npm run build-rust-combat-data
npm run check-rust-combat-data
```

`check` 模式只比较，不覆盖。源数据改变但快照未更新时 CI 失败。

## Rust 边界

`CombatZoneDataSnapshotV1`：

- 使用 `BTreeMap` 保持稳定顺序；
- 对 Zone、出生规则和基础怪物字段强类型；
- 暂未迁移的能力、Buff 和掉落结构保留 JSON DTO；
- 所有出生规则必须引用快照内存在的怪物；
- 快照不得包含未被普通区引用的怪物；
- SHA-256、schema、数据版本和基本数值范围必须验证。

## 验证结果

- `--check` 字节级校验：通过；
- 连续两次生成的 SHA-256：一致；
- 55 个 Zone 与 54 个怪物的 Rust 读取和交叉引用：通过；
- Fly 通用数据与 M2 最小切片一致性：通过；
- cargo fmt：通过；
- Rust workspace tests：通过；
- Clippy `-D warnings`：通过；
- 14 个历史请求 DTO：通过；
- M2 Fly 受限模拟：通过；
- WASM target 与能力报告：通过；
- JavaScript 全量测试、14 组黄金、基准和生产构建：通过。

## 非目标

- 未扩大 `simulate_basic` 的目标范围；
- 未实现多玩家、威胁、技能、Buff 或掉落计算；
- 未支持 Dungeon 或 Labyrinth；
- 未接入 UI；
- 未删除 M2 的最小 Fly 数据切片。

M3A 到此冻结。后续能力扩展在独立分支实施。
