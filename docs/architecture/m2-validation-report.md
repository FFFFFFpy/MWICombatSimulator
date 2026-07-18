# M2 验证报告

- 生成时间：2026-07-18T18:23:38.614446+00:00
- 分支：`agent/rust-basic-combat`
- 兼容级别：`basic-auto-attack-v1`

## 检查结果

| 检查 | 结果 |
|---|---|
| cargo fmt --check | 通过 |
| cargo test --workspace --locked | 失败 (101) |
| cargo clippy -D warnings | 通过 |
| Rust DTO validates 14 requests | 通过 |
| zone-solo-basic restricted simulation | 通过 |
| wasm32-unknown-unknown check | 通过 |
| restricted capability report | 通过 |
| npm ci | 通过 |
| JavaScript full tests | 通过 |
| 14 JavaScript parity goldens | 通过 |
| reference benchmark smoke test | 通过 |
| Vite production build | 通过 |

## 失败诊断

### cargo test --workspace --locked

退出码：`101`

```text
    Updating crates.io index
 Downloading crates ...
  Downloaded serde_core v1.0.228
  Downloaded itoa v1.0.18
  Downloaded thiserror v2.0.18
  Downloaded zmij v1.0.23
  Downloaded quote v1.0.46
  Downloaded serde_derive v1.0.228
  Downloaded unicode-ident v1.0.24
  Downloaded proc-macro2 v1.0.106
  Downloaded thiserror-impl v2.0.18
  Downloaded memchr v2.8.3
  Downloaded serde v1.0.228
  Downloaded serde_json v1.0.150
  Downloaded syn v2.0.119
   Compiling proc-macro2 v1.0.106
   Compiling quote v1.0.46
   Compiling unicode-ident v1.0.24
   Compiling serde_core v1.0.228
   Compiling zmij v1.0.23
   Compiling serde v1.0.228
   Compiling thiserror v2.0.18
   Compiling serde_json v1.0.150
   Compiling itoa v1.0.18
   Compiling memchr v2.8.3
   Compiling syn v2.0.119
   Compiling thiserror-impl v2.0.18
   Compiling serde_derive v1.0.228
   Compiling mwi-sim-core v0.1.0 (/home/runner/work/MWICombatSimulator/MWICombatSimulator/crates/sim-core)
   Compiling mwi-sim-wasm v0.1.0 (/home/runner/work/MWICombatSimulator/MWICombatSimulator/crates/sim-wasm)
   Compiling mwi-sim-cli v0.1.0 (/home/runner/work/MWICombatSimulator/MWICombatSimulator/crates/sim-cli)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 11.94s
     Running unittests src/main.rs (target/debug/deps/mwi_sim_cli-3a1a7515a5000c14)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/mwi_sim_core-c4ecb2949bd13a3c)

running 16 tests
test basic_combat::tests::rejects_requests_outside_the_basic_capability ... ok
test contracts::tests::parses_zone_and_labyrinth_fixtures_without_parsing_player_internals ... ok
test basic_combat::tests::deterministic_replay_is_exact ... ok
test basic_combat::tests::matches_the_reference_basic_result_subset ... ok
test contracts::tests::rejects_invalid_contracts_before_any_engine_exists ... ok
test event_queue::tests::cancellation_is_lazy_but_removed_from_the_live_count_immediately ... ok
test event_queue::tests::pops_earlier_events_first_and_preserves_fifo_for_equal_times ... ok
test event_queue::tests::stale_or_consumed_handles_cannot_cancel_another_event ... ok
test rng::tests::finite_sequence_exhausts_without_incrementing_the_draw_count ... ok
test contracts::tests::preserves_unknown_top_level_and_player_fields ... ok
test rng::tests::looping_sequence_restarts_in_the_same_order ... ok
test rng::tests::seed_hash_uses_javascript_utf16_code_units ... ok
test rng::tests::seeded_rng_matches_reference_javascript_values ... ok
test time::tests::rejects_invalid_times_and_normalizes_negative_zero ... ok
test time::tests::preserves_fractional_nanoseconds_and_total_ordering ... ok
test basic_data::tests::embedded_slice_matches_the_reference_fly_runtime_calculations ... FAILED

failures:

---- basic_data::tests::embedded_slice_matches_the_reference_fly_runtime_calculations stdout ----

thread 'basic_data::tests::embedded_slice_matches_the_reference_fly_runtime_calculations' (3175) panicked at crates/sim-core/src/basic_data.rs:140:9:
assertion `left == right` failed
  left: 3990024937.655861
 right: 3990024937.6558604
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    basic_data::tests::embedded_slice_matches_the_reference_fly_runtime_calculations

test result: FAILED. 15 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

error: test failed, to rerun pass `-p mwi-sim-core --lib`
```
