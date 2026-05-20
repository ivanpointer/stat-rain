# stat-rain Stale/Error And Message Lifecycle Design

Date: 2026-05-20
Status: Approved for implementation planning

## Purpose

This design adds metric health signaling and a stronger message lifecycle to `stat-rain` without turning messages into a generic overlay stack. Text must remain part of the active visual scene so rain can interact with it, wash it away, and future scenes can define their own interactions.

The rendering behavior should stay efficient and predictable. This design creates the right boundaries before a later render-buffer performance pass.

## Goals

- Signal stale/error states only when the affected metric is used by the active visual mappings.
- Distinguish stale and error severity.
- Render unhealthy states as a degraded rain field: greyscale for stale/error, plus a subtle red pulse/tint for error.
- Keep message colors semantic and readable even when the rain field is degraded.
- Support persistent derived health text and queued user messages.
- Add time-based message TTLs and public timing flags in milliseconds.
- Preserve effect-specific text interaction so user messages can be washed away by rain.

## Non-Goals

- Do not build a generic overlay compositor or layer stack.
- Do not dynamically load scene/effect plugins.
- Do not perform render-buffer reuse in this change; it remains the next performance-focused pass.
- Do not render error reasons by default. Reasons are captured for protocol/API usefulness and future diagnostics.

## Metric Status Model

Each metric can be in one of three states:

- `normal`
- `stale`, with an optional reason
- `error`, with an optional reason

A normal metric update clears stale/error status for that metric. A status-only clear command also clears stale/error without requiring a new numeric value.

Metric health is evaluated only for metrics referenced by active mapping expressions. If `thermal_zone` is stale but the active profile maps only `cpu` and `memory`, the visual health state remains normal.

Mapping expressions need to expose the metric names they reference. The health builder uses that referenced-name set to filter the metric registry and produce `HealthState`.

## Health State

`HealthState` is derived each frame after configuration/mapping evaluation has identified the active mappings.

It contains:

- `stale_metrics: Vec<String>`
- `error_metrics: Vec<String>`
- convenience predicates such as `is_degraded` and `has_error`

If both stale and error mapped metrics exist, both are represented. Error names are listed first in text output.

Health text uses concise built-in formatting:

- `ERROR: thermal_zone`
- `STALE: cpu, memory`
- `ERROR: thermal_zone  STALE: cpu, memory`

## Message Model

User messages and health messages are separate concepts.

User messages are queued events from external senders. They have:

- text
- class: `info`, `success`, `warning`, or `error`
- optional `ttl_ms`
- lifecycle timing derived from run defaults plus any per-message TTL

Health text is derived from `HealthState`. It is not queued and has no external TTL. It appears while the mapped health condition remains active and disappears when mapped metrics recover.

Duplicate user messages coalesce by `(text, class)`. TTL differences do not create separate messages. If a duplicate is active or queued, refresh or extend that message's TTL instead of appending another copy.

## Message Selection

Each frame chooses the centered text interaction in this order:

1. Continue the active user message until its lifecycle completes.
2. Otherwise, pop and start the next queued user message.
3. Otherwise, render derived health text if mapped metrics are stale/error.
4. Otherwise, render no centered text.

User messages temporarily preempt health text. The degraded rain field still remains active underneath while health is unhealthy. When the user message finishes, health text returns if the condition still exists.

## Message Lifecycle

Public timings use milliseconds. Frames are an internal implementation detail.

A user message lifecycle has three phases:

1. `fade_in`: randomized reveal over `message_fade_in_ms`.
2. `stay`: fully resolved text for the message `ttl_ms`, or `message_stay_ms` if no TTL is provided.
3. `wash`: rain/embers permanently remove letters they hit over `message_wash_ms`.

Health text uses the same scene text mechanisms where practical, but it is persistent while health remains active. Rain may visually pass over health text if the scene supports that effect, but health letters are not permanently washed away while the condition still exists.

## Scene Interaction API

Text should not be implemented as a generic terminal overlay. The active scene must own how text interacts with the visual field.

The internal boundary should look like a small scene input contract:

```rust
struct SceneInputs<'a> {
    effect_state: EffectState,
    health_state: HealthState,
    active_text: Option<&'a mut TextEvent>,
}

trait Scene {
    fn step(&mut self, inputs: SceneInputs, frame: &mut Frame);
}
```

The exact Rust names may differ, but the responsibility boundary should hold:

- The message subsystem decides which text event is active and which phase it is in.
- The scene decides how that text appears and interacts with the effect.

For `RainScene`:

- user text fades in, stays, then is washed away by rain/embers
- health text fades in and persists while health remains active
- greyscale degradation and error pulse are applied to rain/embers, not message colors

Future scenes can consume the same inputs and choose different interactions, such as dissolving, particle breakup, or distortion.

## Protocol And CLI

Protocol messages should support:

- `MetricUpdate { name, raw, normalized }`
- `MetricStale { name, reason }`
- `MetricError { name, reason }`
- `MetricStatusClear { name }`
- `TextInjection { text, class, ttl_ms }`

CLI examples:

```bash
stat-rain send --socket /tmp/stat-rain.sock --metric cpu --value 0.99
stat-rain send --socket /tmp/stat-rain.sock --metric thermal_zone --stale --reason "sensor timeout"
stat-rain send --socket /tmp/stat-rain.sock --metric thermal_zone --error --reason "read failed"
stat-rain send --socket /tmp/stat-rain.sock --metric thermal_zone --clear-status

stat-rain send --socket /tmp/stat-rain.sock --message "DEPLOY STARTED" --class info --ttl-ms 10000
```

Run timing flags should become duration-based:

```bash
stat-rain run \
  --message-fade-in-ms 1500 \
  --message-stay-ms 3000 \
  --message-wash-ms 2500
```

The existing public frame-based message timing flags should be removed or hidden because the project is pre-1.0 and milliseconds are the correct consumer-facing unit.

## Rendering Behavior

Rain/ember field:

- normal health: existing color behavior
- stale or error: shift rain/embers toward greyscale
- error: add subtle intermittent red pulse/tint on rain/ember field

Messages:

- keep class colors in all health states
- user messages can preempt health text
- health text returns after user message completion if health is still degraded
- user messages wash away permanently during their wash phase
- health text is not permanently washed away while health remains active

## Testing Plan

Focused tests should cover:

- mapped stale metric triggers degraded health state
- unmapped stale metric does not trigger degraded health state
- mapped error metric triggers error health state and error pulse path
- normal metric update clears stale/error
- explicit status clear clears stale/error
- health text includes both error and stale groups when both exist
- user message preempts health text while degraded rain remains active
- health text returns after user message lifecycle completes
- user messages coalesce by `(text, class)`
- duplicate active or queued message refreshes/extends TTL
- message TTL controls resolved stay duration
- public CLI accepts millisecond timing and TTL flags
- protocol round trips new status and TTL fields

## Open Implementation Notes

Use focused refactors while implementing:

- `metrics` owns metric status and status clear/update semantics.
- `mapping` exposes referenced metric names.
- a small health builder turns active mappings plus registry state into `HealthState`.
- a message subsystem owns queueing, coalescing, TTLs, and active user message selection.
- the rain scene owns rain/text interaction and health field rendering.

This keeps implementation modular enough for future scenes while preserving performance-friendly direct frame access.
