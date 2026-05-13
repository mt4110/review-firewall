# Validation

`review-firewall` v0.1 does not ship a scoring system or a dashboard.
It does ship a small, reviewable validation scaffold so trust claims live in the repo instead of in a release note.

## Checked-in anchors

- Demo run: [`docs/demos/anonymized-dogfood-run/README.md`](demos/anonymized-dogfood-run/README.md)
- Demo artifacts: [`docs/demos/anonymized-dogfood-run/run/latest.json`](demos/anonymized-dogfood-run/run/latest.json)
- Manual label corpus scaffold: [`fixtures/validation/manual_labels_v0.1.yaml`](../fixtures/validation/manual_labels_v0.1.yaml)

The checked-in demo mirrors the current artifact contract and keeps the full 45-entry `candidate_blockers` list so `397 -> 45 -> 11` is reproducible from the repo.
It still trims heavy fields such as the full comment list and the full review-thread list.
For manual validation, sampled non-residual rows are traceable through the checked-in `gate.json.classified_comments` slice.
The checked-in corpus is currently a 12-row seed scaffold, not the full 50-comment release sample.
Its checked-in metadata intentionally stays at `human_confirmed: false` and `release_usable: false`, with both human-recorded rates left unset.
It is enough to prove traceability and schema shape, but not enough to publish the first release-usable human validation rates yet.

## Current scaffold status

```text
sampled_rows: 12
target_release_sample: 50
human_confirmed: false
release_usable: false
false_residual_rate: not yet recorded
missed_obvious_blocker_rate: not yet recorded
```

Promotion gate:

- reaching 50 rows is necessary but not sufficient
- the checked-in sample must be human-confirmed before it can claim release-usable rates
- `recorded_metrics.false_residual_rate` and `recorded_metrics.missed_obvious_blocker_rate` must both be populated from that checked-in sample
- `release_usable_reason` should explain which gate is still unmet, or document the promotion once all gates pass

Next operator step:

- expand the checked-in corpus by at least 38 more human-labeled comments
- keep every new row traceable to the checked-in dogfood demo artifacts
- record the first release-usable `false_residual_rate` and `missed_obvious_blocker_rate`
- flip `human_confirmed: true` and `release_usable: true` only after those rates are actually recorded from the checked-in human sample

## Metrics

### Compression ratio

```text
compression_to_candidate = 1 - candidate_blockers / comments_analyzed
compression_to_residual  = 1 - residual_blockers / comments_analyzed
```

Current checked-in dogfood demo:

```text
comments_analyzed: 397
candidate_blockers: 45
residual_blockers: 11
compression_to_candidate: 88.66%
compression_to_residual: 97.23%
```

### False residual blocker rate

This lives in the manual-label corpus.
Measure it from rows where:

```text
observed_bucket = residual_blocker
```

Formula:

```text
false_residual_rate =
  false_residual_blockers / residual_blockers_in_sample
```

In the scaffold corpus, `false_residual_blockers` are rows with:

```text
observed_bucket = residual_blocker
manual_label != true_blocker
```

### Missed obvious blocker rate

This also lives in the manual-label corpus.
Measure it from rows where:

```text
obvious_blocker = true
```

Formula:

```text
missed_obvious_blocker_rate =
  obvious_blockers_not_in_residual / obvious_blockers_in_sample
```

In the scaffold corpus, `obvious_blockers_not_in_residual` are rows with:

```text
obvious_blocker = true
observed_bucket != residual_blocker
```

### Partial safety rate

This remains code-level validation.
For degraded fixtures and smoke paths:

```text
partial_clear_count must be 0
```

### Noise leak rate

This remains a mix of fixture coverage and dogfood inspection.
The checked-in demo should not surface badge/html fragments in `failure_mode` or `report.md`.

## Release thresholds

| Metric | Threshold |
|---|---:|
| residual compression | >= 90% on noisy PRs |
| false residual blocker rate | <= 20% |
| missed obvious blocker rate | <= 10% |
| partial -> clear cases | 0 |
| noise leak in failure_mode | 0 |
| draft reply unsafe claim | 0 |

## Corpus shape

The v0.1 scaffold intentionally stays manual and file-based.
The checked-in corpus also records top-level scaffold metadata:

- `sample_target`
- `human_confirmed`
- `release_usable`
- `release_usable_reason`
- `recorded_metrics.false_residual_rate`
- `recorded_metrics.missed_obvious_blocker_rate`

`human_confirmed` means a human has reviewed the checked-in labels and the recorded release metrics for this sample.
It should stay `false` for provisional or partially expanded corpus updates.
When recorded, `recorded_metrics.*` values should be stored as decimal ratios derived from the checked-in sample, not rounded percentages, and encoded with the shortest round-trip IEEE-754 `f64` decimal representation.

Each labeled row records:

- `comment_id`
- `source_type`
- `path`
- `observed_bucket`
- `manual_label`
- `obvious_blocker`
- `reason`

Each sampled row should be traceable to the checked-in dogfood demo through one of:

- `gate.json.residual_blockers`
- `gate.json.candidate_blockers`
- `gate.json.classified_comments`

Allowed `manual_label` values:

- `true_blocker`
- `false_blocker`
- `question`
- `suggestion`
- `nit`
- `design_debate`
- `unknown`
- `noise`

`release_usable` must stay `false` whenever any of these remain true:

- sampled rows are still below `sample_target`
- the checked-in sample is not yet human-confirmed
- either recorded release metric is still unset

## What is still manual

- Expanding the checked-in corpus from 12 rows to a human-labeled 50-comment release sample
- Recording the first release-usable `false_residual_rate` from that human sample
- Recording the first release-usable `missed_obvious_blocker_rate` from that human sample

That is deliberate for v0.1.
The product stays a local deterministic triage CLI, while the trust claim gets a concrete place to live in-repo.
