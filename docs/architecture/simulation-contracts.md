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
      "maxEntries": 100000,
      "detailLevel": "basic"
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
  "labyrinthHrid": "/monsters/fly",
  "roomLevel": 1,
  "crates": []
}
```

后续新增目标类型需要扩展 `kind`，不能复用现有字段表达不同语义。

### random

- `null`：使用运行环境原生随机源；
- `seeded`：用于可重复的日常测试和用户复现；
- `sequence`：用于跨语言严格差分，显式提供 `[0, 1)` 数列。

固定数列耗尽应报错。禁止静默切换到原生随机源，否则差分结果将失去意义。

当前复杂黄金场景使用循环固定数列，例如：

```json
{
  "type": "sequence",
  "values": [0],
  "loop": true
}
```

它用于稳定触发所有正概率分支，并让 JavaScript 与 Rust 消费完全相同的显式随机流。普通用户模拟不应默认使用这种配置。

### statisticsMode

- `full`：兼容现有完整统计输出；
- `fast`：为未来的大规模参数搜索预留，允许省略昂贵明细。

M0 中 reference engine 仍按完整统计运行。启用 `fast` 前必须定义具体字段差异和 UI 降级行为。

### trace

```json
{
  "enabled": true,
  "maxEntries": 100000,
  "detailLevel": "basic"
}
```

字段含义：

- `enabled`：是否记录事件轨迹，默认 `false`；
- `maxEntries`：最多保留的事件条数，至少为 1；
- `detailLevel`：轨迹详细等级，可选，默认 `basic`。

支持两个详细等级。

#### basic

用于基础行为兼容，记录：

- 事件类型、时间、source、target、ability 和 consumable；
- HP、MP；
- 眩晕、致盲和沉默状态；
- 事件队列新增与取消；
- 随机值消费；
- 遭遇、副本完成和失败计数。

现有基础黄金场景使用该模式。未显式提供 `detailLevel` 的旧请求也会归一化为 `basic`。

#### combat

用于复杂机制黄金校验和诊断，在 `basic` 基础上额外记录：

- active Buff、类型、数值、开始时间和持续时间；
- 技能、食物和饮料的 `lastUsed` 与法力消耗；
- OOM 状态；
- 控制状态到期时间；
- 等级、命中、伤害、闪避、护甲和抗性等派生属性；
- 反伤、招架、狂怒、削弱、穿透、吸血、回蓝、威胁等装备或 Buff 状态。

`combat` 模式明显更重，只应在黄金文件、差分测试和定向诊断中使用。普通网页运行、批量模拟和参数搜索不应默认启用详细轨迹。诊断工具存在的意义是抓虫，不是成为热路径里永久居住的寄生虫。

未知 `detailLevel` 会在请求归一化阶段报错，不会静默降级。

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

`trace` 仅在请求显式开启轨迹时存在于运行结果。旧 Worker 兼容层的普通响应不会凭空增加 `trace: null` 字段。

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

后续可以在兼容字段中增加支持的 trace 详细等级。UI 和任务运行时应基于能力协商，而不是通过引擎名称硬编码猜测功能。

## 旧 Worker 兼容

M0 提供：

- `legacyWorkerMessageToSimulationRequestV1`；
- `simulationRequestV1ToLegacyWorkerMessage`。

旧消息仍可继续工作。`trace.detailLevel` 会通过适配器往返；未提供时默认 `basic`。后续页面和 store 应逐步改为先生成协议请求，再由 runtime 选择引擎适配器。

## 版本策略

### 兼容改动

可在 V1 内新增可选字段，但：

- 旧消费者应能忽略；
- 默认值必须明确；
- 不改变已有字段含义。

`trace.detailLevel` 属于兼容新增字段，因为它可选且默认保持原有基础轨迹语义。

### 不兼容改动

以下变化需要 V2：

- 删除或重命名字段；
- 改变单位；
- 改变时间或概率表示；
- 改变目标结构；
- 改变结果统计语义；
- 将可选字段改为必填。

协议版本与产品版本、游戏数据版本、Rust crate 版本分别管理。把所有版本塞进一个数字只会让排错变成考古。
