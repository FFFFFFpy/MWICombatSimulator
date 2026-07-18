# 性能基准规范

## 目的

性能基准用于回答三个问题：

1. 当前时间花在哪里；
2. 某项改造是否带来真实收益；
3. JavaScript、Rust/WASM 和 Rust Native 在相同行为下分别达到什么吞吐量。

基准不是宣传页面。任何通过关闭正常功能、减少统计或改变随机行为得到的数字，都必须明确标注不同模式，不能和完整兼容模式混在一起比较。

## 基准层级

### Micro

用于定位数据结构和热函数：

- 事件堆 push/pop；
- 事件取消和查找；
- Buff 添加、删除和派生属性更新；
- Trigger 检查；
- 结果统计聚合；
- DTO 编解码；
- Worker 消息序列化。

Micro 基准不能代替真实战斗场景。

### Scenario

固定角色、目标、随机源和模拟时长，运行完整战斗：

- 单人普通 Zone；
- 五人普通 Zone；
- 五人 Dungeon；
- 团灭重开 Dungeon；
- Labyrinth；
- Buff/Trigger 密集场景；
- 开启和关闭 HP/MP 可视化；
- 开启和关闭 trace。

### Batch

衡量平台吞吐量：

- 全 Zone；
- 全 Labyrinth；
- Queue 基线和多个变体；
- 多轮 Monte Carlo；
- 参数组合搜索。

Batch 基准必须分别报告单任务速度与总吞吐，避免通过过度并行把单任务延迟藏起来。

## 必须记录的指标

每次场景基准至少输出：

- 场景 ID 和 fixture 路径；
- 协议版本；
- 数据版本；
- 引擎及版本；
- 统计模式；
- trace 和时序图开关；
- 模拟时间；
- wall-clock 时间；
- 处理事件数；
- 每秒事件数；
- 峰值事件队列长度；
- 随机值消费数量；
- 结果摘要；
- Worker 数量；
- 运行环境摘要。

内存指标在运行环境可可靠提供时记录：

- 峰值 RSS；
- JS heap 或 WASM memory；
- 分配量与 GC 时间。

浏览器 API 无法可靠提供时应标记缺失，不要伪造零值。

## 运行协议

1. 使用固定游戏数据快照；
2. 使用固定请求和随机源；
3. 先执行至少一次预热；
4. 每个场景重复多次；
5. 报告中位数、P90 和最小值；
6. 不以首次构建、模块下载或页面加载时间冒充核心模拟时间；
7. Worker 启动成本单独报告；
8. 比较不同引擎时先通过 parity 测试；
9. 快速统计模式必须和完整模式分开报告；
10. 同一场景的重复轮次必须产生相同事件数、随机消费数和结果摘要。

## 当前命令

M0 提供第一个可执行场景基准：

```bash
npm run benchmark:combat
```

常用参数：

```bash
npm run benchmark:combat -- \
  --iterations 10 \
  --warmup 2 \
  --simulation-seconds 600 \
  --seed benchmark-zone-solo-basic \
  --output tmp/benchmark-zone-solo-basic.json
```

当前场景读取：

```text
fixtures/parity/zone-solo-basic/request.json
```

脚本通过 Vite SSR 加载与网页相同的 JavaScript reference engine，不复制一套简化公式。每轮使用相同请求和 seed，并校验 workload fingerprint；如果处理事件数、随机消费数或结果摘要不一致，命令直接失败。

当前事件计数通过轻量包装 `EventQueue.getNextEvent()` 获得，因此数字适合版本间相对比较，不应声称是完全零开销的绝对吞吐量。

## 输出格式

命令输出机器可读 JSON，包含：

```json
{
  "benchmarkVersion": 1,
  "generatedAt": "2026-07-18T00:00:00.000Z",
  "environment": {},
  "scenario": {
    "id": "zone-solo-basic",
    "engine": "reference-js",
    "statisticsMode": "full"
  },
  "summary": {
    "iterations": 10,
    "medianMs": 0,
    "p90Ms": 0,
    "medianEventsPerSecond": 0,
    "workloadFingerprint": ""
  },
  "runs": []
}
```

## 性能门槛

普通 CI 不使用固定毫秒数作为跨机器门槛。CI 机器、浏览器和宿主负载变化会制造无意义失败。

可以使用：

- 同一次运行中的基线对比；
- 明显数量级回退检查；
- 事件数、随机消费数和结果摘要一致性；
- 专用固定硬件上的趋势报告。

CI 当前只执行一次 30 秒模拟作为命令可运行性检查，不把该耗时视为性能门槛。

## 预期优化顺序

在迁移到 Rust 前，先用基准验证：

1. 逐事件 `async/await` 成本；
2. 事件队列全量扫描与删除成本；
3. Buff 导致的完整属性重算；
4. 全场 Trigger 扫描；
5. 统计对象分配；
6. Worker 启停和重复数据传输。

Rust 重写应采用新的数据结构，而不是逐行翻译当前对象模型。

## 后续场景

M0 后续继续补充五人 Zone、Dungeon、团灭重开和 Labyrinth fixture。每个新场景必须先通过确定性重放，再进入基准套件。否则基准数字精确到小数点后六位，也只是在认真测量不确定性。
