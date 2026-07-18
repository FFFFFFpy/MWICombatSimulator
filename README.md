# MWICombatSimulator

一个用于 **Milky Way Idle** 的非官方独立模拟与规划平台。当前提供战斗模拟、刷图推荐、强化评估和生活技能路线规划，前端使用 Vue 3 + Vite + Tailwind。

## 项目定位

本仓库将长期独立维护，不再仅作为上游页面的功能型 fork。后续演进遵循以下方向：

- 现有 JavaScript 战斗引擎作为行为参考实现；
- 建立版本化、语言无关的模拟输入输出协议；
- 战斗核心逐步迁移到 Rust；
- 浏览器通过 WebAssembly 调用正式战斗核心；
- 原生 CLI 承担高吞吐批量模拟与基准任务；
- Vue 前端、数据同步、强化与生活技能工具继续独立演进；
- 所有底层迁移必须通过固定随机源、事件轨迹和差分测试验证，不以“结果看起来接近”代替兼容性。

架构计划见 [`docs/architecture/platform-overview.md`](docs/architecture/platform-overview.md)。

M0 行为基线已经完成：JavaScript reference engine 具备版本化协议、确定性随机源、基础/详细事件轨迹、可执行基准和 14 组黄金差分场景。兼容策略与已覆盖机制见 [`docs/architecture/parity-strategy.md`](docs/architecture/parity-strategy.md)。

## 主要功能

- **Home**：配置角色、目标、难度和时长，运行模拟并查看关键指标与构建快照。
- **Queue**：基于“基线 + 多个变体”执行多轮评分与排名，对比收益增量和波动。
- **Advisor**：批量扫描 Solo/Group Zones 与 Labyrinth，输出刷图目标排行。
- **Enhancement**：比较保护阈值、贤者之镜、分解价值、成本分位和预算成功率。
- **Skilling**：结合角色经验、库存、装备、Buff 和市场价格规划生活技能逐级路线。
- **Multi Results**：汇总多轮结果并支持导出 Excel。
- **Import/Export**：支持导入导出；可配合 Tampermonkey 脚本从主站导入战斗、强化或生活技能所需的当前角色数据。
- **官方中英游戏词条 + Web Workers**：发布前同步官网词条快照，运行时离线加载；Worker 并行计算批量任务。

## 本地开发

```bash
npm install
npm run dev
```

生产构建：

```bash
npm run build
```

运行测试：

```bash
npm test
```

只读验证全部黄金场景：

```bash
npm run check:parity
```

显式重新生成黄金场景：

```bash
npm run generate:parity
```

运行 reference engine 基准：

```bash
npm run benchmark:combat
```

检查当前战斗目标和机制数据：

```bash
npm run inspect:combat-targets
npm run inspect:combat-mechanics
```

## 部署

本项目使用 Vite 构建，可部署到任意静态站点平台。仓库尚未声明固定官方部署地址时，请以当前仓库 Release、Pages 配置或 README 后续更新为准，不沿用上游 fork 的部署地址。

## 游戏数据

游戏数据来源、免责声明、快照生成与同步流程见 [`docs/game-data.md`](docs/game-data.md)。实时市场价格不属于不可变游戏数据快照，应作为独立输入或价格快照管理。

## 上游与许可证

本仓库基于社区 MWI Combat Simulator 项目及后续 fork 演进而来。感谢原作者与社区维护者提供的基础实现。

项目使用 MIT License。复制、修改、分发或发布时须保留许可证与版权声明。详见 [`LICENSE`](LICENSE)。
