# 模拟兼容与差分验证策略

## 状态

- M0 行为基线：Completed / Frozen
- JavaScript reference engine：Current
- Rust 差分接入：M1/M2

## 目标

新的 Rust 引擎必须证明自己与 JavaScript reference engine 行为一致。最终结果“差不多”不够，因为不同的事件顺序、随机消费顺序或 Buff 到期时机会在长模拟中累积成明显偏差。

## 兼容层级

### L1：协议兼容

同一 `SimulationRequestV1` 能够被两个引擎接受，结果都能包装为 `SimulationResultV1`。

检查内容：

- 必填字段；
- 数值范围；
- Zone 与 Labyrinth 目标；
- 数据版本；
- 统计模式；
- 随机源和 trace 配置；
- JSON 序列化往返。

### L2：随机消费兼容

固定随机数序列下，两个引擎必须：

- 消费相同数量的随机值；
- 以相同顺序消费；
- 在相同事件中消费对应值；
- 对序列耗尽使用明确错误，不静默回退。

迁移过程中优先使用显式数列，而不是仅使用同一个 seed。不同语言的 RNG 实现可以不同，但固定数列能够隔离概率公式和事件调度差异。

### L3：事件轨迹兼容

每条基础事件轨迹至少记录：

- 顺序号；
- 模拟时间；
- 事件类型；
- source、target、ability、consumable；
- HP/MP 和控制状态变化；
- 随机值消费；
- 事件队列新增和取消；
- 波次和地下城状态变化；
- 处理异常。

复杂机制场景使用 `trace.detailLevel = "combat"`，额外记录：

- active Buff 及起止时间；
- 技能、食物与饮料冷却；
- OOM 状态；
- 控制状态到期时间；
- 关键基础属性、派生战斗值和装备被动；
- 威胁、反伤、招架、狂怒、削弱、诅咒、穿透等运行时状态。

详细轨迹仅用于黄金校验与诊断。普通模拟默认使用基础模式或完全关闭 trace，避免让诊断结构污染生产热路径。

差分比较在第一个不一致事件处停止，并输出该事件前后的最小状态差异。

### L4：最终结果兼容

完整统计模式下逐字段比较：

- 模拟时间；
- 击杀、死亡、复活、团灭和波次；
- 攻击、命中、伤害与技能统计；
- HP/MP 恢复和消耗；
- 经验；
- 消耗品；
- 掉落倍率与收益相关基础统计；
- Dungeon 与 Labyrinth 特有字段；
- 时间序列数据（启用时）。

浮点字段只有在公式本身不可避免地产生跨语言舍入差异时才允许显式容差。容差必须按字段定义，禁止全局使用宽松百分比。

## 黄金场景

当前已提交十四个确定性场景：

| 场景 | 保护的主要路径 |
|---|---|
| `zone-solo-basic` | 单人普通区、基础攻击、重生、经验与结果聚合 |
| `zone-party-three` | 普通组队区合法最大人数、多人目标选择与结果聚合 |
| `dungeon-party-complete` | 五人 Chimerical Den、50 波推进、完成结算与下一轮初始化 |
| `dungeon-wipe-restart` | 团灭日志、事件选择性清理、三秒重启与失败计数 |
| `labyrinth-fly-room-100` | Labyrinth DTO、房间等级缩放及迷宫特有结果字段 |
| `ability-dot-control-sequence` | Firestorm DOT、致盲、沉默、眩晕及对应到期事件 |
| `oom-yogurt-recovery` | OOM 统计、缺蓝 Trigger、Yogurt 持续回蓝与重新施法 |
| `equipment-passives-reflect` | Griffin Bulwark 削弱、Furious Spear 狂怒、Spike Shell 反伤与 Retribution |
| `regal-sword-parry` | Regal Sword 招架重定向及敌方攻击事件中的反击伤害 |
| `healing-hot-party` | Quick Aid、Rejuvenate、Blueberry Cake 与最低血量治疗目标选择 |
| `ability-pierce-multi-target` | Penetrating Shot 的继续穿透与 Sweep 的全体敌人伤害 |
| `cursed-bow-stacking` | Cursed Bow 诅咒叠层、旧到期事件取消与新事件重排 |
| `revive-dead-ally` | 死亡统计、deadAlly Trigger、Revive 恢复与攻击事件重新调度 |
| `speed-aura-simultaneous-expiration` | 同次施法产生的两个 Buff 在同时间点连续到期及稳定队列顺序 |

其中九个机制场景使用 `random.type = "sequence"` 与循环零值：

- 所有正概率状态稳定触发；
- 不依赖 JavaScript seeded RNG 的实现细节；
- Rust 引擎可使用完全相同的显式随机流；
- 轨迹会固定每次随机消费所在的事件。

基础五场景保留 seeded RNG，用于锁定当前 JavaScript reference engine 的随机实现和现实运行形态。跨语言首轮差分应优先从显式数列场景开始。

当前 reference 数据中：

- `dungeon-party-complete` 完成 Chimerical Den 1 次，最高波次 50；
- `dungeon-wipe-restart` 在 120 秒内记录 13 次团灭与失败重启；
- `ability-dot-control-sequence` 稳定触发 DOT、致盲、沉默与眩晕；
- `oom-yogurt-recovery` 实际进入 OOM，执行持续回蓝并再次施法；
- `healing-hot-party` 实际产生主动治疗和 HP 恢复 Tick；
- `revive-dead-ally` 实际把零 HP 队友恢复为正 HP；
- 所有基础、复杂和高级轨迹均未截断。

## 目标与机制数据检查

官方数据快照中，普通组队 Zone 的最大队伍人数为 3；五人队场景应使用 Dungeon。场景人数和目标类型不得仅凭页面名称推断。

```bash
npm run inspect:combat-targets
npm run inspect:combat-mechanics
```

## 黄金文件结构

每个场景包含：

```text
request.json
expected-result.json
expected-trace.json.gz.b64
# 或超大轨迹：
expected-trace.json.gz.b64.part-000
expected-trace.json.gz.b64.part-001
...
metadata.json
```

随机配置直接包含在 `request.json` 中。事件轨迹采用规范化 JSON 的 gzip+base64 文本归档。超过 120,000 个字符的归档自动分片；校验器按照 `metadata.json.expectedFiles` 拼接，先验证完整归档 SHA-256，再解压比较第一个不同字段。

`metadata.json` 记录：

- 引擎、数据和协议版本；
- 随机消费数量；
- 结果、原始轨迹和压缩归档 SHA-256；
- 轨迹分片数量；
- 场景语义断言。

语义断言可要求：

- 结果字段精确相等；
- 结果字段达到最小值；
- 最少随机消费数和事件数；
- 必须实际出现的事件类型。

名为“副本完成”的场景若没有产生 `dungeonsCompleted >= 1`，生成器会直接失败。文件名不能替程序作证，人类已经有足够多这种制度了。

## 黄金文件命令

显式生成：

```bash
npm run generate:parity
```

只生成一个场景：

```bash
npm run generate:parity -- --fixture ability-dot-control-sequence
```

只读校验：

```bash
npm run check:parity
```

普通测试和永久 CI 只执行校验，不会自动覆盖仓库中的黄金文件。

## 黄金文件更新规则

- 普通测试只读取，不自动覆盖黄金文件；
- 必须通过单独命令显式重新生成；
- 更新黄金文件的 PR 必须说明行为变化原因；
- 如果公式没有计划变更，而黄金结果大量变化，应视为回归而不是“顺手接受”；
- 数据快照升级与引擎逻辑升级尽量分开提交；
- 生成前必须确认 trace 未截断；
- wall-clock 字段必须规范化，不能让当前时间污染黄金结果；
- 场景语义断言和对应集成测试必须先通过；
- 轨迹分片缺失、顺序错误或内容改变均视为校验失败；
- 临时 CI 写权限只用于受控生成，产物提交后必须恢复 `contents: read`。

M0 已冻结。后续一般功能不得通过“顺手更新黄金”写回本 PR；只有明确的 reference 行为修复或数据快照升级才允许更新，并必须单独解释差异。

## Trace 的生产约束

事件 trace 默认关闭：

- 普通结果中不包含 trace；
- 开启 trace 不得改变模拟结果；
- trace 有明确最大条数并报告截断数量；
- trace 只保留 JSON 安全字段，不序列化 class 实例或循环引用；
- 同 HRID 的多个单位必须通过队伍位置键区分；
- `basic` 模式保护基础事件和单位状态；
- `combat` 模式保护 Buff、冷却、OOM、派生属性和装备被动；
- 超长批量模拟不应默认开启完整 trace。

## 差分失败输出

差分工具应输出：

1. 场景和请求 ID；
2. 数据与引擎版本；
3. 第一个不一致事件索引；
4. 两边事件摘要；
5. 随机消费差异；
6. 单位状态最小差异；
7. 队列操作差异；
8. 最终结果差异是否只是后续连锁反应。

不要一次打印数万行完整对象。错误报告的工作是定位，不是惩罚阅读者。

## 当前实现

M0 基础设施包括：

- `src/shared/randomSource.js`：原生、seeded、固定数列和 tracing 随机源；
- `src/services/combatTrace.js`：默认关闭的基础与详细战斗态轨迹；
- `src/contracts/simulationContracts.js`：版本化协议、trace detail level 与旧 Worker 消息适配器；
- `src/services/referenceSimulationRunner.js`：统一装配目标、角色 Buff、随机源、trace 和进度的 reference engine 边界；
- `scripts/parity-fixtures.mjs`：语义断言、黄金生成、分片归档和只读校验；
- `scripts/inspect-combat-targets.mjs`：普通区、副本、队伍上限和迷宫怪物候选；
- `scripts/inspect-combat-mechanics.mjs`：技能效果、装备槽位、消耗品和 Trigger 候选；
- `src/combatsimulator/__tests__/complexMechanicsSimulation.test.js`：四个复杂 fixture 的机制级断言；
- `src/combatsimulator/__tests__/advancedMechanicsSimulation.test.js`：五个高级 fixture 的治疗、群攻、诅咒、复活与顺序断言；
- `fixtures/parity/*`：十四个已锁定的确定性场景。

当前行为基线已经覆盖 Rust 核心第一阶段所需的主要事件、状态、随机消费和最终统计路径。后续仍可在独立 PR 追加新机制，但不再阻塞 Rust workspace、数据模型、RNG 与稳定事件队列。
