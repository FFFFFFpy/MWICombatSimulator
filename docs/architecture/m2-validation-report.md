# M2 验证报告

- 生成时间：2026-07-18T18:39:10.861950+00:00
- 分支：`agent/rust-basic-combat`
- 兼容级别：`basic-auto-attack-v1`

## 检查结果

| 检查 | 结果 |
|---|---|
| cargo fmt --check | 失败 (1) |
| cargo test --workspace --locked | 通过 |
| cargo clippy -D warnings | 通过 |
| Rust DTO validates 14 requests | 通过 |
| zone-solo-basic restricted simulation | 通过 |
| WASM target check | 通过 |
| restricted capability report | 通过 |
| npm ci | 通过 |
| JavaScript full tests | 通过 |
| 14 JavaScript parity goldens | 通过 |
| reference benchmark smoke test | 通过 |
| Vite production build | 通过 |

## 失败诊断

### cargo fmt --check

退出码：`1`

```text
Diff in /home/runner/work/MWICombatSimulator/MWICombatSimulator/crates/sim-core/src/basic_data.rs:117:
             ("monster.autoAttackDamage", self.monster.auto_attack_damage),
         ] {
             if !value.is_finite() {
-                return Err(SimError::InvalidGameData(format!(
-                    "{name} must be finite"
-                )));
+                return Err(SimError::InvalidGameData(format!("{name} must be finite")));
             }
         }
         if self.monster.base_attack_interval == SimTime::ZERO {
```
