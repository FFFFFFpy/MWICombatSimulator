# M3D Rust 单体直接伤害施法运行时

## 状态

- 基线：`agent/rust-ability-data`
- 当前阶段：核心 reference 差分已通过，CLI、WASM 与正式 capability 已接通，正在执行最终格式化和全链路验证。
- 正式能力仅升格 level-1 Aqua Arrow 与空自定义 Trigger。

## 目标

在 M2 的受限单人 Fly 战斗闭环中加入一个通用的单体直接伤害施法运行时，并首先使用 Aqua Arrow 与 JavaScript reference engine 做逐字段差分。

## 首个支持请求

- 单名玩家；
- tier-0 `/actions/combat/fly`；
- 无装备、食物、饮料、房屋、工会 Buff 与成就；
- 仅配置一个 level-1 `/abilities/aqua_arrow`；
- 自定义 Trigger 为空；
- deterministic seeded 或 sequence RNG；
- 不启用 trace 与 HP/MP 可视化；
- 输出受限结果，不冒充完整 `SimulationResultV1`。

## 对齐的 reference 语义

1. 单位同一时间只保留一个 AutoAttack 或 AbilityCastEnd 事件；
2. 技能满足 Trigger、冷却和法力条件时，优先于普通攻击；
3. 施法结束事件时间为 `now + castDuration / (1 + castSpeed)`；
4. 法力在施法结束时重新校验并扣除；
5. `lastUsed` 在施法结束时设置，冷却从施法结束开始；
6. 技能伤害使用 AbilityEffect 指定的 combat style 与 damage type；
7. 等级成长、命中、远程暴击、随机伤害、增伤、穿透与抗性遵循 reference 公式；
8. 技能处理后重新选择下一次技能或普通攻击；
9. 敌人死亡、三秒刷新、玩家死亡与复活继续复用 M2 行为；
10. 随机消费数量与顺序可复现。

## Aqua Arrow reference fixture

`fixtures/parity/ability-aqua-arrow-basic`

- 60 秒；
- 循环随机序列 `[0.5]`；
- 高生存玩家；
- level-1 Aqua Arrow；
- combat-detail trace；
- 5 次击杀；
- Aqua Arrow 施放 3 次，每次 34 伤害；
- 技能耗蓝 105；
- 总随机消费 135。

首发时间线：

- `0s`：CombatStart 安排 Aqua Arrow 于 `0.49504950495049506s` 完成；
- `0.49504950495049506s`：Fly `50→16`，玩家 MP `300→265`；
- 同时设置 `lastUsed`，并安排玩家下一次普通攻击于 `3.4653465346534657s`；
- 冷却从施法完成时刻开始计算。

## 已接通边界

- `mwi_sim_core::simulate_direct_damage`；
- CLI：`simulate-direct-damage <request.json>`；
- WASM：`simulateDirectDamageJson`；
- `direct_damage_combat_result`；
- capability 只声明 Aqua Arrow level 1、空 Trigger、单人 tier-0 Fly。

## 停止标准

1. Aqua Arrow golden 生成且完整 trace 未截断；
2. Rust 施法结束、法力、冷却与直接伤害事件顺序对齐；
3. encounters、deaths、完整 attacks histogram、manaUsed、时间字段和 randomDraws 与 golden 一致；
4. 同请求重复运行完全一致；
5. 旧 M2 Fly 自动攻击 fixture 保持精确一致；
6. Rust fmt、tests、Clippy、WASM 与全部 JavaScript goldens 通过；
7. 达标后只正式声明 Aqua Arrow，不自动开放其余九个候选。

达到以上标准后停止，不实现 DOT、Buff、控制、穿透概率、群攻或自定义 Trigger。
