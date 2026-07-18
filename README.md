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

## 主要功能

- **Home**：配置角色、目标、难度和时长，运行模拟并查看关键指标与构建快照。
- **Queue**：基于“基线 + 多个变体”执行多轮评分与排名，对比收益增量和波动。
- **Advisor**：批量扫描 Solo/Group Zones 与 Labyrinth，输出刷图目标排行。
- **Enhancement**：比较保护阈值、贤者之镜、分解价值、成本分位和预算成功率。
- **Skilling**：结合角色经验、库存、装备、Buff 和市场价格规划生活技能逐级路线。
- **Multi Results**：汇总多轮结果并支持导出 Excel。
- **Import/Export**：支持配置导入导出，并可配合 Tampermonkey 脚本导入当前角色数据。
- **官方中英游戏词条 + Web Workers**：发布前同步官网词条快照，运行时离线加载；Worker 用于批量计算。

## 在线部署

本仓库目前没有承诺长期可用的官方托管地址。第三方镜像可能存在版本差异，应以本仓库源码、Release 和提交记录为准。

## 快速开始

安装依赖：

```bash
npm install
```

启动开发环境：

```bash
npm run dev
```

生产构建：

```bash
npm run build
```

本地预览：

```bash
npm run preview
```

运行测试：

```bash
npm test
```

## 常用文档

- [`docs/architecture/platform-overview.md`](docs/architecture/platform-overview.md)：长期平台架构与迁移边界。
- [`docs/architecture/parity-strategy.md`](docs/architecture/parity-strategy.md)：固定随机源、事件轨迹和差分验证策略。
- [`docs/architecture/simulation-contracts.md`](docs/architecture/simulation-contracts.md)：模拟协议与版本管理原则。
- [`docs/architecture/benchmarking.md`](docs/architecture/benchmarking.md)：性能基准原则与报告口径。
- [`docs/game-data.md`](docs/game-data.md)：游戏数据、官方词条来源和刷新流程。
- [`docs/init-client-data-key-reference.md`](docs/init-client-data-key-reference.md)：`initClientData` 顶层 key 对照表。
- [`scripts/mwi-main-site-import.README.md`](scripts/mwi-main-site-import.README.md)：Tampermonkey 导入脚本说明。

## Fork 来源与致谢

本项目 fork 自 [shykai/MWICombatSimulatorTest](https://github.com/shykai/MWICombatSimulatorTest)，并继承了多个社区 fork 的长期维护成果。具体历史可通过 Git 提交记录追溯。

独立维护不等于抹掉来源。保留可追溯的上游历史，是工程责任，不是装饰品。

## 开源协议

本项目以 MIT License 开源，详见 [LICENSE](LICENSE)。游戏数据、名称、美术与相关知识产权归 Milky Way Idle 官方及其权利方所有。
