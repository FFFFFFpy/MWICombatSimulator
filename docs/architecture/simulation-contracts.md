# 模拟协议

## 原则

模拟协议是 UI、Worker、JavaScript reference engine、Rust/WASM 和 Rust CLI 之间的稳定边界。

协议必须：

- 仅包含 JSON 可序列化值；
- 不包含 Vue、Pinia、DOM、Worker、class 实例或函数；
- 明确协议版本、数据版本和请求 ID；
- 对未知字段保持可扩展性；
- 对必填字段和数值范围尽早报错；
- 通过显式适配器连接旧 Worker 消息，不让兼容逻辑散落在页面中。

当前实现位于 `src/contracts/simulationContracts.js`。

## SimulationRequestV1

```json
{
  "contractVersion": 1,
  "requestId": "fixture-zone-1",
  "dataVersion": "data-2026-07-18",
  "players": [],
  "target": {
    "kind": "zone",
    "zoneHrid": "/actions/combat/fly",
    "difficultyTier": 0
  },
  "simulationTimeLimit": 3600000000000,
  "random": {
    "type": "seeded",
    "seed": "fixture"
  },
  "options": {
    "statisticsMode": "full",
    "enableHpMpVisualization": false,
    "trace": {
      "enabled": false,
      "maxEntries": 100000
    },
    "extra": {}
  }
}
```

### target

支持两种目标：

```json
{
  "kind": "zone",
  "zoneHrid": "/actions/combat/fly",
  "difficultyTier": 0
}
```

```json
{
  "kind": "labyrinth",
  "labyrinthHrid": "/actions/combat/labyrinth",
  "roomLevel": 1,
  "crates": 0
}
```

后续新增目标类型需要扩展 `kind`，不能复用现有字段表达不同语义。

### random

- `null`：使用运行环境原生随机源；
- `seeded`：用于可重复的日常测试和用户复现；
- `sequence`：用于跨语言严格差分，显式提供 `[0, 1)` 数列。

固定数列耗尽应报错。禁止静默切换到原生随机源，否则差分结果将失去意义。

### statisticsMode

- `full`：兼容现有完整统计输出；
- `fast`：为未来的大规模参数搜索预留，允许省略昂贵明细。

M0 中 reference engine 仍按完整统计运行。启用 `fast` 前必须定义具体字段差异和 UI 降级行为。

## SimulationProgressV1

```json
{
  "contractVersion": 1,
  "type": "simulation_progress",
  "requestId": "fixture-zone-1",
  "progress": 0.5,
  "target": null,
  "detail": null
}
```

`progress` 始终限制在 `0..1`。进度只表达当前任务状态，不应反复携带不断增长的完整结果或时间序列。

## SimulationResultV1

```json
{
  "contractVersion": 1,
  "type": "simulation_result",
  "requestId": "fixture-zone-1",
  "engine": "reference-js",
  "engineVersion": "1.0.28",
  "dataVersion": "data-2026-07-18",
  "result": {},
  "trace": null
}
```

`result` 在迁移初期可承载现有 `SimResult` 的 JSON 形态。后续字段清理必须通过新的结果协议版本完成，不能在 V1 中静默删除历史字段。

## SimulationErrorV1

```json
{
  "contractVersion": 1,
  "type": "simulation_error",
  "requestId": "fixture-zone-1",
  "error": {
    "code": "SIMULATION_FAILED",
    "message": "Simulation failed.",
    "detail": null
  }
}
```

错误代码用于程序判断，错误消息用于展示或诊断。不得要求 UI 解析英文错误文本来判断错误类型。

## GameDataManifestV1

数据清单至少包含：

- 快照版本；
- 生成时间；
- 来源及其哈希；
- 文件路径及其哈希；
- 协议版本。

模拟结果通过 `dataVersion` 关联清单。实时市场价格不属于不可变游戏数据快照，应作为独立输入或价格快照管理。

## EngineCapabilitiesV1

引擎在运行前报告：

- 引擎 ID 和版本；
- 支持的目标类型；
- 支持的统计模式；
- 是否支持确定性随机源；
- 是否支持事件 trace；
- 是否支持原生批处理。

UI 和任务运行时应基于能力协商，而不是通过引擎名称硬编码猜测功能。

## 旧 Worker 兼容

M0 提供：

- `legacyWorkerMessageToSimulationRequestV1`；
- `simulationRequestV1ToLegacyWorkerMessage`。

旧消息仍可继续工作。后续页面和 store 应逐步改为先生成协议请求，再由 runtime 选择引擎适配器。

## 版本策略

### 兼容改动

可在 V1 内新增可选字段，但：

- 旧消费者应能忽略；
- 默认值必须明确；
- 不改变已有字段含义。

### 不兼容改动

以下变化需要 V2：

- 删除或重命名字段；
- 改变单位；
- 改变时间或概率表示；
- 改变目标结构；
- 改变结果统计语义；
- 将可选字段改为必填。

协议版本与产品版本、游戏数据版本、Rust crate 版本分别管理。把所有版本塞进一个数字只会让排错变成考古。
