# M3A Rust 普通区数据快照

## 目标

将当前 JavaScript 游戏数据中的普通 Combat Zone 与其引用怪物，编译为确定性、可追溯、可由 Rust 强类型读取的版本化快照。

本阶段只建立数据边界，不扩大 Rust 模拟能力。

## 输入

- `src/combatsimulator/data/actionDetailMap.json`；
- `src/combatsimulator/data/combatMonsterDetailMap.json`。

## 输出

`crates/sim-core/data/combat-zone-data-v1.json`

快照包含：

- schema 与游戏数据版本；
- 两个源文件的 SHA-256；
- 所有非 Dungeon Combat Zone；
- Zone 的人数上限、难度、Buff 和随机出生规则；
- 所有被普通区引用的怪物；
- 怪物基础等级、基础 combat stats、能力 DTO、经验、狂暴时间和掉落 DTO；
- 确定排序和规范 JSON 表示。

## 生成命令

```bash
npm run build-rust-combat-data
npm run check-rust-combat-data
```

`check` 模式只比较，不覆盖。源数据改变但快照未更新时 CI 必须失败。

## Rust 边界

新增 `CombatZoneDataSnapshotV1`：

- 使用 `BTreeMap` 保持稳定顺序；
- 对 Zone、出生规则和基础怪物字段强类型；
- 暂未迁移的能力、Buff 和掉落结构保留 JSON DTO；
- 所有出生规则必须引用快照内存在的怪物；
- SHA-256、schema、数据版本和基本数值范围必须验证。

## 非目标

- 不扩大 `simulate_basic` 的目标范围；
- 不实现多玩家、威胁、技能、Buff 或掉落计算；
- 不支持 Dungeon 或 Labyrinth；
- 不接入 UI；
- 不删除 M2 的最小 Fly 数据切片。

## 停止标准

1. 生成器对同一输入产生字节一致输出；
2. `--check` 在快照一致时通过，在篡改后失败；
3. Rust 能读取并验证整个快照；
4. 所有普通区出生怪物均存在；
5. Fly 数据可从通用快照解析，并与 M2 切片的基础字段一致；
6. M0、M1、M2 的全部验证继续通过。

达到以上标准后停止，不在本 PR 扩大模拟能力。
