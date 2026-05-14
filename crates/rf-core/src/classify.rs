use crate::dedupe::collapse_duplicates_with_primary_filter;
use crate::domain::CodeownerRule;
use crate::domain::{
    BlockerConcern, ClassifiedComment, CommentRecord, CommentType, EvidenceClass, GateArtifact,
    GateConfigSnapshot, GateCounts, ResidualBlocker, ReviewDecisionSummary, ScanArtifact, Status,
    review_signal_for,
};
use crate::escalation::evaluate_escalation_candidates;
use crate::normalize::{normalize_body, split_sentences};
use crate::ownership::build_ownership_advisory;

pub fn gate_scan(
    scan: &ScanArtifact,
    config: &GateConfigSnapshot,
    codeowner_rules: &[CodeownerRule],
) -> GateArtifact {
    let mut classified = scan
        .comments
        .iter()
        .chain(scan.issue_comments.iter())
        .map(|comment| classify_comment(comment, scan, config, codeowner_rules))
        .collect::<Vec<_>>();

    let duplicates_collapsed =
        collapse_duplicates_with_primary_filter(&mut classified, |comment| {
            !is_pr_author_comment(&comment.comment, scan)
        });
    let candidate_blockers = classified
        .iter()
        .filter(|comment| {
            comment.duplicate_of_comment_id.is_none()
                && !is_pr_author_comment(&comment.comment, scan)
                && is_candidate_blocker(comment, config)
        })
        .map(to_residual_blocker)
        .collect::<Vec<_>>();

    let residual_blockers = collect_residual_blockers(&classified, config);
    let downgraded_comments = classified
        .iter()
        .filter(|comment| {
            let evidence_allows_blocker =
                evidence_allows_blocker(comment.evidence_class, config.require_evidence);
            comment.comment_type != CommentType::Blocker
                && comment.concern.is_some()
                && (comment.failure_mode.is_none()
                    || !evidence_allows_blocker
                    || !comment.present_pr_impact)
        })
        .map(|comment| comment.comment.comment_id.clone())
        .collect::<Vec<_>>();

    let mut counts = GateCounts::default();
    for comment in classified
        .iter()
        .filter(|comment| comment.duplicate_of_comment_id.is_none())
    {
        match comment.comment_type {
            CommentType::Blocker => {}
            CommentType::Question => counts.questions += 1,
            CommentType::Suggestion => counts.suggestions += 1,
            CommentType::Nit => counts.nits += 1,
            CommentType::Praise => counts.praise += 1,
            CommentType::Unknown => counts.unknown += 1,
        }
    }

    let status = if scan.status == Status::Error && classified.is_empty() {
        Status::Error
    } else {
        scan.status
    };
    let review_signal = review_signal_for(scan.data_coverage, residual_blockers.len());

    GateArtifact {
        status,
        data_coverage: scan.data_coverage,
        review_signal,
        reason: scan.reason.clone(),
        comments_analyzed: classified.len(),
        residual_blockers,
        counts,
        candidate_blockers,
        downgraded_comments,
        duplicates_collapsed,
        warnings: scan.warnings.clone(),
        config_snapshot: config.clone(),
        review_decision_summary: ReviewDecisionSummary::from_states(&scan.pr.review_decisions),
        classified_comments: classified,
        escalation_candidates: evaluate_escalation_candidates(
            &scan.review_threads,
            config.max_pr_thread_roundtrips,
        ),
    }
}

fn is_pr_author_comment(comment: &CommentRecord, scan: &ScanArtifact) -> bool {
    let pr_author = scan.pr.author.trim();
    !pr_author.is_empty() && comment.author.trim().eq_ignore_ascii_case(pr_author)
}

fn classify_comment(
    comment: &CommentRecord,
    scan: &ScanArtifact,
    config: &GateConfigSnapshot,
    codeowner_rules: &[CodeownerRule],
) -> ClassifiedComment {
    let concern = extract_concern(&comment.body);
    let failure_mode = extract_failure_mode(&comment.body);
    let evidence = extract_evidence(comment, failure_mode.as_deref());
    let evidence_class = extract_evidence_class(comment, &evidence);
    let evidence_supported = evidence_class.is_some_and(EvidenceClass::supports_residual_blocker);
    let present_pr_impact = extract_present_pr_impact(
        comment,
        failure_mode.as_deref(),
        config.require_failure_mode,
        config.require_evidence,
        concern,
        evidence_supported,
    );
    let preference_only = is_preference_only(&comment.body, concern);
    let author_comment = is_pr_author_comment(comment, scan);
    let ownership = build_ownership_advisory(
        comment.author.as_str(),
        comment.path.as_deref(),
        &scan.changed_files,
        codeowner_rules,
        config.use_codeowners,
    );

    let evidence_satisfies_gate = evidence_allows_blocker(evidence_class, config.require_evidence);
    let candidate_blocker = !author_comment
        && (!config.require_concern || concern.is_some())
        && (!config.require_failure_mode || failure_mode.is_some())
        && evidence_satisfies_gate
        && !preference_only;

    let alternative_present = contains_any(
        &normalize_body(&comment.body),
        &[
            "alternative",
            "instead",
            "代替",
            "別案",
            "別の方法",
            "他の方法",
        ],
    );
    let full_blocker = candidate_blocker
        && present_pr_impact
        && (!config.require_alternative || alternative_present);

    let comment_type = if full_blocker {
        CommentType::Blocker
    } else if matches!(evidence_class, Some(EvidenceClass::NoiseOnly)) {
        CommentType::Unknown
    } else if is_praise(&comment.body) {
        CommentType::Praise
    } else if preference_only || is_nit(&comment.body) {
        CommentType::Nit
    } else if is_question(&comment.body) {
        CommentType::Question
    } else if is_suggestion(&comment.body) || concern.is_some() {
        CommentType::Suggestion
    } else if comment.body.trim().is_empty() {
        CommentType::Unknown
    } else {
        CommentType::Question
    };

    ClassifiedComment {
        comment: comment.clone(),
        comment_type,
        concern,
        failure_mode,
        evidence_class,
        evidence,
        present_pr_impact,
        owner_match: ownership.owner_match,
        ownership_scope: ownership.ownership_scope,
        advisory_weight: ownership.advisory_weight,
        duplicate_of_comment_id: None,
    }
}

fn collect_residual_blockers(
    classified: &[ClassifiedComment],
    config: &GateConfigSnapshot,
) -> Vec<ResidualBlocker> {
    let mut seen_threads = Vec::<String>::new();
    let mut residual = Vec::new();

    for comment in classified.iter().filter(|comment| {
        comment.comment_type == CommentType::Blocker
            && comment.duplicate_of_comment_id.is_none()
            && evidence_allows_blocker(comment.evidence_class, config.require_evidence)
    }) {
        if seen_threads
            .iter()
            .any(|thread_id| thread_id == &comment.comment.thread_id)
        {
            continue;
        }
        seen_threads.push(comment.comment.thread_id.clone());
        residual.push(to_residual_blocker(comment));
    }

    residual
}

fn to_residual_blocker(comment: &ClassifiedComment) -> ResidualBlocker {
    ResidualBlocker {
        comment_id: comment.comment.comment_id.clone(),
        concern: comment.concern.unwrap_or(BlockerConcern::Correctness),
        failure_mode: comment
            .failure_mode
            .clone()
            .unwrap_or_else(|| String::from("failure mode was not extracted")),
        evidence_class: comment
            .evidence_class
            .unwrap_or(EvidenceClass::ConcreteReference),
        evidence: if comment.evidence.is_empty() {
            vec![String::from("evidence was not extracted")]
        } else {
            comment.evidence.clone()
        },
        owner_match: comment.owner_match,
        ownership_scope: comment.ownership_scope,
        advisory_weight: comment.advisory_weight,
        path: comment.comment.path.clone(),
        author: comment.comment.author.clone(),
    }
}

fn is_candidate_blocker(comment: &ClassifiedComment, config: &GateConfigSnapshot) -> bool {
    (!config.require_concern || comment.concern.is_some())
        && (!config.require_failure_mode || comment.failure_mode.is_some())
        && evidence_allows_blocker(comment.evidence_class, config.require_evidence)
}

fn extract_concern(body: &str) -> Option<BlockerConcern> {
    for sentence in split_sentences(body) {
        if let Some(concern) = extract_concern_from_text(&sentence) {
            return Some(concern);
        }
    }
    extract_concern_from_text(body)
}

fn extract_concern_from_text(text: &str) -> Option<BlockerConcern> {
    let lower = normalize_body(text);
    if is_metalinguistic_context(&lower) && !has_runtime_risk_context(&lower) {
        return None;
    }
    let scores = [
        (
            BlockerConcern::Correctness,
            count_matches(
                &lower,
                &[
                    "break",
                    "broken",
                    "correctness",
                    "incorrect",
                    "wrong",
                    "regress",
                    "fail",
                    "partial status",
                    "null",
                    "panic",
                    "unwrap",
                    "壊れる",
                    "壊れ",
                    "落ち",
                    "欠け",
                    "不整合",
                    "誤",
                    "ずれ",
                    "二重送信",
                    "stale",
                    "wrap",
                    "feature flag",
                    "新コード側",
                    "旧挙動",
                    "戻らない",
                    "backfill",
                    "sort_unstable",
                    "status=partial",
                    "必須前提",
                    "上限がない",
                    "無限",
                    "送信前",
                    "break the response contract",
                    "break consumers",
                ],
            ),
        ),
        (
            BlockerConcern::Security,
            count_matches(
                &lower,
                &[
                    "auth",
                    "permission",
                    "security",
                    "secret",
                    "token",
                    "authorization",
                    "xss",
                    "csrf",
                    "ssrf",
                    "sql injection",
                    "sql",
                    "path traversal",
                    "open redirect",
                    "vuln",
                    "漏えい",
                    "漏れる",
                    "認可",
                    "権限",
                    "資格情報",
                    "注入",
                    "アクセストークン",
                    "秘密情報",
                    "authorization ヘッダ",
                    "ヘッダ",
                    "unsafe",
                    "デシリアライズ",
                    "deserial",
                    "任意コード実行",
                    "cors",
                    "access-control-allow-origin",
                    "credentials",
                    "origin",
                ],
            ),
        ),
        (
            BlockerConcern::Performance,
            count_matches(
                &lower,
                &[
                    "performance",
                    "slow",
                    "latency",
                    "n+1",
                    "full scan",
                    "oom",
                    "memory",
                    "allocation",
                    "cpu",
                    "throughput",
                    "遅く",
                    "高負荷",
                    "劣化",
                    "メモリ",
                    "同時接続",
                    "ブロック",
                    "効率",
                    "sort",
                    "top 20",
                    "lock",
                    "mutex",
                    "contention",
                    "負荷",
                    "レイテンシ",
                    "alloc",
                    "同期",
                    "i/o",
                    "clone",
                    "batch",
                    "batch endpoint",
                    "逐次",
                    "1件ずつ",
                ],
            ),
        ),
        (
            BlockerConcern::Operability,
            count_matches(
                &lower,
                &[
                    "retry",
                    "timeout",
                    "rollback",
                    "operability",
                    "migration",
                    "logging",
                    "metrics",
                    "alert",
                    "deploy",
                    "切り戻し",
                    "運用",
                    "監視",
                    "ジョブ",
                    "ワーカー",
                    "詰まる",
                    "メトリクス",
                    "観測",
                    "runbook",
                    "owner",
                    "request_id",
                    "trace",
                    "トレーサビリティ",
                    "アラート",
                    "復旧",
                    "kill switch",
                    "ロールバック",
                    "idempotency",
                    "silent failure",
                    "握りつぶし",
                    "気づけません",
                    "ログ",
                    "追跡",
                    "failure path",
                    "夜間",
                    "誰も気づ",
                    "止められない",
                    "止められません",
                    "障害時",
                ],
            ),
        ),
        (
            BlockerConcern::Api,
            count_matches(
                &lower,
                &[
                    "api",
                    "contract",
                    "schema",
                    "response shape",
                    "request shape",
                    "consumer",
                    "endpoint",
                    "レスポンス",
                    "互換",
                    "クライアント",
                    "契約",
                    "後方互換",
                    "互換性",
                    "json キー",
                    "query param",
                    "status code",
                    "404",
                    "200+empty",
                    "wire format",
                    "content-type",
                    "mime",
                    "公開 api",
                    "serializer",
                    "page size",
                    "id の形式",
                    "id 型",
                    "error body",
                    "shape",
                ],
            ),
        ),
    ];

    scores
        .into_iter()
        .filter(|(_, score)| *score > 0)
        .max_by_key(|(concern, score)| (*score, concern_tiebreaker(*concern)))
        .map(|(concern, _)| concern)
}

fn concern_tiebreaker(concern: BlockerConcern) -> usize {
    match concern {
        BlockerConcern::Security => 5,
        BlockerConcern::Api => 4,
        BlockerConcern::Performance => 3,
        BlockerConcern::Correctness => 2,
        BlockerConcern::Operability => 1,
    }
}

fn is_metalinguistic_context(text: &str) -> bool {
    contains_any(
        text,
        &[
            "readme",
            "docs",
            "issue ",
            "ui ",
            "文言",
            "単語",
            "言葉",
            "用語",
            "名前",
            "命名",
            "rename",
            "言い換え",
            "説明",
            "補足",
            "注釈",
            "脚注",
            "表",
            "章",
            "見やす",
            "読みづら",
            "分かりにく",
            "わかりにく",
            "語順",
            "column 名",
            "field 名",
            "メソッド名",
            "サンプル",
            "図",
            "日本語",
            "折りたた",
            "整形",
            "書いて",
            "表現",
            "親切",
            "固い",
            "強すぎる",
            "先頭",
            "上に",
            "後ろ",
        ],
    )
}

fn has_runtime_risk_context(text: &str) -> bool {
    contains_any(
        text,
        &[
            "壊れ",
            "落ち",
            "漏れ",
            "詰ま",
            "遅く",
            "失敗",
            "危険",
            "劣化",
            "崩れ",
            "ずれ",
            "互換",
            "後方互換",
            "契約変更",
            "権限外",
            "攻撃",
            "重複",
            "二重送信",
            "止められ",
            "戻らない",
            "枯渇",
            "panic する",
            "break ",
            "broken",
            "fail",
            "leak",
            "xss",
            "csrf",
            "ssrf",
            "sql injection",
            "path traversal",
            "open redirect",
            "vuln",
            "unsafe",
            "bypass",
            "stale",
            "oom",
            "contention",
            "タイムアウト",
        ],
    )
}

fn extract_failure_mode(body: &str) -> Option<String> {
    split_sentences(body).into_iter().find(|sentence| {
        let normalized = normalize_body(sentence);
        normalized.starts_with("mode:")
            || (!is_metalinguistic_failure_mode_context(&normalized)
                && has_failure_mode_signal(&normalized))
    })
}

fn is_metalinguistic_failure_mode_context(text: &str) -> bool {
    let mentions_failure_mode_logic = contains_any(
        text,
        &[
            "failure-mode extractor",
            "failure mode extractor",
            "failure_mode extractor",
            "failure-mode extraction",
            "failure mode extraction",
            "failure_mode extraction",
            "failure-mode matching",
            "failure mode matching",
            "failure_mode matching",
        ],
    );
    let mentions_meta_wording = contains_scope_marker(
        text,
        &[
            "wording",
            "docs",
            "documentation",
            "explanation",
            "description",
            "phrasing",
            "naming",
            "rename",
            "narrower",
        ],
    ) || contains_any(
        text,
        &["簡潔", "説明", "表現", "文言", "言い方", "言い回し"],
    );
    let mentions_runtime_classifier_breakage =
        has_runtime_failure_signal_in_metalinguistic_context(text)
            || contains_evidence_marker(
                text,
                &[
                    "partial",
                    "status=",
                    "residual blocker",
                    "true blocker",
                    "dropped",
                    "drop",
                    "missing",
                    "誤判定",
                    "見落と",
                    "取りこぼ",
                    "壊れ",
                ],
            );

    mentions_failure_mode_logic && mentions_meta_wording && !mentions_runtime_classifier_breakage
}

fn has_runtime_failure_signal_in_metalinguistic_context(text: &str) -> bool {
    contains_evidence_marker(
        text,
        &[
            "break",
            "breakage",
            "breaks",
            "breaking",
            "broken",
            "regress",
            "regressed",
            "regression",
            "regresses",
            "timeout",
            "timeouts",
            "timed out",
            "times out",
            "leak",
            "leaks",
            "leaked",
            "leaking",
            "drop",
            "drops",
            "dropped",
            "dropping",
            "incompatible",
            "panic",
            "panics",
            "panicked",
            "panicking",
            "crash",
            "crashes",
            "crashed",
            "crashing",
            "壊れる",
            "壊れ",
            "落ちる",
            "漏れる",
            "漏えい",
            "詰まる",
            "遅くなる",
            "崩れる",
            "ずれる",
            "失敗",
            "誤動作",
            "二重送信",
            "タイムアウト",
            "stale",
            "古い値",
            "wrap",
        ],
    ) || contains_contextual_failure_phrase(text)
}

fn has_failure_mode_signal(text: &str) -> bool {
    contains_evidence_marker(
        text,
        &[
            "break",
            "breakage",
            "breaks",
            "breaking",
            "broken",
            "fail",
            "fails",
            "failed",
            "failing",
            "failures",
            "incorrect",
            "wrong",
            "regress",
            "regressed",
            "regression",
            "regresses",
            "timeout",
            "timeouts",
            "timed out",
            "times out",
            "leak",
            "leaks",
            "leaked",
            "leaking",
            "drop",
            "drops",
            "dropped",
            "dropping",
            "incompatible",
            "panic",
            "panics",
            "panicked",
            "panicking",
            "crash",
            "crashes",
            "crashed",
            "crashing",
            "壊れる",
            "壊れ",
            "落ちる",
            "漏れる",
            "漏えい",
            "詰まる",
            "遅くなる",
            "崩れる",
            "ずれる",
            "失敗",
            "誤動作",
            "二重送信",
            "タイムアウト",
            "stale",
            "古い値",
            "wrap",
        ],
    ) || contains_contextual_failure_phrase(text)
}

fn contains_contextual_failure_phrase(text: &str) -> bool {
    contains_any(
        text,
        &[
            "failure in ",
            "failure on ",
            "failure with ",
            "failure when ",
            "failure under ",
            "failure after ",
            "failure during ",
            "failure for ",
            "fail in ",
            "fails in ",
            "failed in ",
            "failing in ",
            "fail on ",
            "fails on ",
            "failed on ",
            "failing on ",
            "fail with ",
            "fails with ",
            "failed with ",
            "failing with ",
            "fail when ",
            "fails when ",
            "failed when ",
            "failing when ",
            "fail under ",
            "fails under ",
            "failed under ",
            "failing under ",
            "fail after ",
            "fails after ",
            "failed after ",
            "failing after ",
            "fail during ",
            "fails during ",
            "failed during ",
            "failing during ",
            "fail for ",
            "fails for ",
            "failed for ",
            "failing for ",
        ],
    )
}

fn extract_evidence(comment: &CommentRecord, failure_mode: Option<&str>) -> Vec<String> {
    let mut evidence = Vec::<String>::new();
    let normalized_failure_mode = failure_mode.map(normalize_body);

    for snippet in backtick_fragments(&comment.body) {
        if looks_like_concrete_reference_fragment(&snippet) {
            evidence.push(format!("comment references `{snippet}`"));
        }
    }

    for sentence in split_sentences(&comment.body) {
        let normalized = normalize_body(&sentence);
        if normalized_failure_mode.as_deref() == Some(normalized.as_str())
            && !sentence_supports_independent_evidence(&normalized)
        {
            continue;
        }
        if sentence_supports_evidence(&normalized) && !looks_like_contract_only_claim(&normalized) {
            evidence.push(sentence);
        }
    }

    dedupe_strings(evidence)
}

fn extract_evidence_class(comment: &CommentRecord, evidence: &[String]) -> Option<EvidenceClass> {
    let normalized_body = normalize_body(&comment.body);
    if normalized_body.is_empty() {
        return Some(EvidenceClass::NoiseOnly);
    }

    if let Some(path) = comment.path.as_ref()
        && (normalize_body(path) == normalized_body
            || looks_like_path_only_text(comment.body.as_str(), path))
    {
        return Some(EvidenceClass::PathOnly);
    }

    if evidence.is_empty() {
        return Some(EvidenceClass::KeywordOnly);
    }

    let normalized_evidence = normalize_body(&evidence.join(" "));

    if contains_evidence_marker(
        &normalized_evidence,
        &[
            "ci",
            "check failed",
            "test fails",
            "test failure",
            "failing test",
            "red build",
            "workflow failed",
        ],
    ) {
        return Some(EvidenceClass::CiTestFailure);
    }

    if contains_evidence_marker(
        &normalized_evidence,
        &[
            "auth",
            "authorization",
            "permission",
            "token",
            "secret",
            "xss",
            "csrf",
            "ssrf",
            "sql injection",
            "path traversal",
            "open redirect",
            "vuln",
            "認可",
            "権限",
            "漏えい",
        ],
    ) {
        return Some(EvidenceClass::SecurityCondition);
    }

    if contains_contract_surface(&normalized_body)
        && contains_contract_change_marker(&normalized_body)
        && contains_contract_specific_marker(&normalized_body)
    {
        return Some(EvidenceClass::ContractDelta);
    }

    if contains_evidence_marker(
        &normalized_evidence,
        &[
            "if ",
            "when ",
            "returns",
            "status=",
            "repro",
            "steps",
            "under ",
            "only when",
            "race",
        ],
    ) {
        return Some(EvidenceClass::ReproCondition);
    }

    if evidence
        .iter()
        .any(|item| item.starts_with("comment references `"))
    {
        return Some(EvidenceClass::ConcreteReference);
    }

    if contains_any(&normalized_evidence, &["because", "理由:", "根拠"]) {
        return Some(EvidenceClass::CausalRuntimeFailure);
    }

    Some(EvidenceClass::ConcreteReference)
}

fn evidence_allows_blocker(evidence_class: Option<EvidenceClass>, require_evidence: bool) -> bool {
    match evidence_class {
        Some(EvidenceClass::PathOnly | EvidenceClass::NoiseOnly) => false,
        Some(EvidenceClass::KeywordOnly) => !require_evidence,
        Some(class) => !require_evidence || class.supports_residual_blocker(),
        None => false,
    }
}

fn extract_present_pr_impact(
    comment: &CommentRecord,
    failure_mode: Option<&str>,
    require_failure_mode: bool,
    require_evidence: bool,
    concern: Option<BlockerConcern>,
    evidence_present: bool,
) -> bool {
    let lower = normalize_body(&comment.body);
    let scope_marker = contains_scope_marker(
        &lower,
        &[
            "this pr",
            "this change",
            "as written",
            "current change",
            "in this diff",
            "in this patch",
            "in this pr",
            "今回の pr",
            "この pr",
            "この変更",
            "merge",
            "here",
        ],
    );
    if !scope_marker {
        return false;
    }
    failure_mode.is_some()
        || (!require_failure_mode && concern.is_some() && (!require_evidence || evidence_present))
}

fn contains_scope_marker(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| {
        if marker.is_ascii() {
            contains_ascii_marker(text, marker)
        } else {
            text.contains(marker)
        }
    })
}

fn starts_with_any_ascii_marker(text: &str, markers: &[&str]) -> bool {
    markers
        .iter()
        .any(|marker| starts_with_ascii_marker(text, marker))
}

fn starts_with_ascii_marker(text: &str, marker: &str) -> bool {
    text.strip_prefix(marker).is_some_and(|remaining| {
        remaining
            .bytes()
            .next()
            .is_none_or(|byte| !is_ascii_word_byte(byte))
    })
}

fn contains_ascii_marker(text: &str, marker: &str) -> bool {
    text.match_indices(marker).any(|(index, _)| {
        let bytes = text.as_bytes();
        let before = index == 0 || !is_ascii_word_byte(bytes[index - 1]);
        let after_index = index + marker.len();
        let after = after_index == bytes.len() || !is_ascii_word_byte(bytes[after_index]);
        before && after
    })
}

fn is_ascii_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_preference_only(body: &str, concern: Option<BlockerConcern>) -> bool {
    let lower = normalize_body(body);
    contains_any(
        &lower,
        &[
            "prefer",
            "preference",
            "familiar",
            "used this before",
            "we usually",
            "convention",
            "personally",
            "feels odd",
            "個人的",
            "しっくりこない",
            "派です",
            "好み",
            "ダサい",
            "エレガント",
            "違和感",
            "嫌です",
            "怖い",
        ],
    ) && concern.is_none()
}

fn is_praise(body: &str) -> bool {
    let lower = normalize_body(body);
    lower.len() <= 160
        && contains_any(
            &lower,
            &[
                "lgtm",
                "looks good",
                "nice",
                "good catch",
                "great",
                "clean",
                "thanks for fixing",
                "動くならいい",
                "良さそう",
            ],
        )
}

fn is_nit(body: &str) -> bool {
    contains_any(
        &normalize_body(body),
        &[
            "nit",
            "style",
            "naming",
            "rename",
            "typo",
            "format",
            "whitespace",
            "minor",
            "small",
            "nits:",
            "スタイル",
            "派です",
        ],
    )
}

fn is_question(body: &str) -> bool {
    let lower = normalize_body(body);
    body.contains('?')
        || starts_with_any_ascii_marker(
            &lower,
            &[
                "why", "how", "what", "when", "where", "can", "could", "would", "is", "are",
                "should",
            ],
        )
        || contains_any(
            &lower,
            &[
                "ですか",
                "ますか",
                "でしょうか",
                "なぜ",
                "どうして",
                "気になります",
                "違和感",
                "大丈夫ですか",
                "しんどい",
                "怖い",
            ],
        )
}

fn is_suggestion(body: &str) -> bool {
    contains_any(
        &normalize_body(body),
        &[
            "suggest",
            "consider",
            "maybe",
            "could",
            "would",
            "recommend",
            "perhaps",
            "instead",
            "しませんか",
            "したいです",
            "ほしいです",
            "寄せたい",
            "寄せませんか",
            "見直したい",
            "増やしたい",
            "始めたい",
            "たいです",
            "方が",
            "方がわかりやすい",
            "分割しませんか",
            "合わせませんか",
        ],
    )
}

fn sentence_supports_evidence(text: &str) -> bool {
    contains_evidence_marker(
        text,
        &[
            "because",
            "for example",
            "for instance",
            "if ",
            "when ",
            "returns",
            "status=",
            "response",
            "contract",
            "schema",
            "理由:",
            "根拠",
            "証拠",
            "test fails",
            "check failed",
            "ci",
        ],
    )
}

fn looks_like_contract_only_claim(text: &str) -> bool {
    contains_contract_surface(text)
        && contains_contract_change_marker(text)
        && !contains_contract_specific_marker(text)
        && !contains_non_contract_impact_marker(text)
}

fn contains_evidence_marker(text: &str, needles: &[&str]) -> bool {
    needles
        .iter()
        .any(|needle| evidence_marker_matches(text, needle))
}

fn evidence_marker_matches(text: &str, needle: &str) -> bool {
    if needle.is_ascii() && needle.bytes().all(is_ascii_word_byte) {
        contains_ascii_marker(text, needle)
    } else {
        text.contains(needle)
    }
}

fn sentence_supports_independent_evidence(text: &str) -> bool {
    contains_evidence_marker(
        text,
        &[
            "because",
            "for example",
            "for instance",
            "if ",
            "when ",
            "returns",
            "status=",
            "理由:",
            "根拠",
            "証拠",
            "test fails",
            "check failed",
            "ci",
            "auth",
            "authorization",
            "permission",
            "token",
            "secret",
            "xss",
            "csrf",
            "ssrf",
            "sql injection",
            "path traversal",
            "open redirect",
            "response shape",
            "request shape",
            "consumer",
            "wire format",
            "status code",
            "後方互換",
            "互換",
            "契約",
        ],
    )
}

fn contains_contract_surface(text: &str) -> bool {
    contains_evidence_marker(
        text,
        &[
            "contract",
            "schema",
            "response",
            "request",
            "consumer",
            "client",
            "compatibility",
            "backward compatibility",
            "wire format",
            "content-type",
            "query param",
            "serializer",
            "後方互換",
            "互換",
            "契約",
            "レスポンス",
            "リクエスト",
            "クライアント",
        ],
    )
}

fn contains_contract_change_marker(text: &str) -> bool {
    contains_evidence_marker(
        text,
        &[
            "change",
            "changes",
            "changed",
            "changing",
            "remove",
            "removed",
            "renamed",
            "rename",
            "add",
            "added",
            "returns",
            "return",
            "returned",
            "now",
            "no longer",
            "instead of",
            "different",
            "mismatch",
            "変わ",
            "変更",
            "消え",
            "削除",
            "増え",
            "追加",
            "返す",
            "返ら",
            "新値",
            "必須",
        ],
    )
}

fn contains_contract_specific_marker(text: &str) -> bool {
    contains_evidence_marker(
        text,
        &[
            "partial status",
            "partial",
            "status code",
            "return code",
            "response shape",
            "request shape",
            "array",
            "object",
            "list",
            "map",
            "string",
            "int",
            "integer",
            "bool",
            "boolean",
            "number",
            "null",
            "nullable",
            "optional",
            "required",
            "field",
            "json key",
            "query param",
            "enum",
            "serializer",
            "content-type",
            "wire format",
            "page size",
            "id type",
            "id 型",
            "header",
            "mime",
            "404",
            "200+empty",
            "レスポンス shape",
            "レスポンス schema",
            "レスポンス形式",
            "status=",
        ],
    )
}

fn contains_non_contract_impact_marker(text: &str) -> bool {
    contains_evidence_marker(
        text,
        &[
            "auth",
            "authorization",
            "credentials",
            "permission",
            "token",
            "secret",
            "leak",
            "xss",
            "csrf",
            "ssrf",
            "sql injection",
            "path traversal",
            "open redirect",
            "500",
            "503",
            "timeout",
            "panic",
            "exception",
            "crash",
            "null",
            "nil",
            "race",
            "stale",
            "retry",
            "latency",
            "memory",
            "worker",
            "rollback",
            "ci",
            "test fails",
            "check failed",
            "status=",
            "理由:",
            "根拠",
            "証拠",
        ],
    )
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn count_matches(text: &str, needles: &[&str]) -> usize {
    needles
        .iter()
        .filter(|needle| concern_keyword_matches(text, needle))
        .count()
}

fn concern_keyword_matches(text: &str, needle: &str) -> bool {
    if needle.is_ascii() && needle.bytes().all(is_ascii_word_byte) {
        contains_ascii_marker(text, needle)
    } else {
        text.contains(needle)
    }
}

fn backtick_fragments(body: &str) -> Vec<String> {
    let mut fragments = Vec::new();
    let mut in_fragment = false;
    let mut current = String::new();
    for character in body.chars() {
        if character == '`' {
            if in_fragment && !current.trim().is_empty() {
                fragments.push(current.trim().to_owned());
                current.clear();
            }
            in_fragment = !in_fragment;
            continue;
        }
        if in_fragment {
            current.push(character);
        }
    }
    fragments
}

fn looks_like_noise_fragment(fragment: &str) -> bool {
    let normalized = normalize_body(fragment);
    normalized.is_empty()
        || normalized.contains("style=flat")
        || normalized.contains("shields.io")
        || normalized.contains("img.shields.io")
        || normalized.contains("badge")
}

fn looks_like_concrete_reference_fragment(fragment: &str) -> bool {
    if looks_like_noise_fragment(fragment) {
        return false;
    }

    let normalized = normalize_body(fragment);
    if normalized.is_empty() {
        return false;
    }
    if matches!(
        normalized.as_str(),
        "foo"
            | "bar"
            | "baz"
            | "qux"
            | "dummy"
            | "example value"
            | "placeholder"
            | "todo"
            | "fixme"
            | "note"
            | "notes"
            | "test"
            | "tests"
            | "tmp"
            | "temp"
    ) {
        return false;
    }
    if contains_any(
        &normalized,
        &[
            "partial",
            "status",
            "json",
            "query",
            "header",
            "content-type",
            "schema",
            "field",
            "enum",
            "format",
            "response",
            "request",
            "serializer",
            "page",
            "mime",
            "メトリクス",
        ],
    ) {
        return true;
    }
    if fragment.contains(char::is_whitespace) {
        return true;
    }
    if matches!(
        normalized.as_str(),
        "sql" | "xss" | "csrf" | "ssrf" | "html" | "utc" | "db" | "ci"
    ) {
        return true;
    }
    if normalized.len() >= 3
        && fragment
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        return true;
    }

    fragment.chars().any(|character| {
        character.is_ascii_digit()
            || matches!(
                character,
                '/' | '\\' | '_' | '-' | '.' | '=' | '(' | ')' | '[' | ']' | ':' | '<' | '>'
            )
    }) || has_mixed_case_identifier(fragment)
}

fn has_mixed_case_identifier(fragment: &str) -> bool {
    let has_lower = fragment
        .chars()
        .any(|character| character.is_ascii_lowercase());
    let has_upper = fragment
        .chars()
        .any(|character| character.is_ascii_uppercase());
    has_lower && has_upper
}

fn looks_like_path_only_text(body: &str, path: &str) -> bool {
    let normalized_body = normalize_body(body);
    let normalized_path = normalize_body(path);

    normalized_body == normalized_path
        || normalized_body == normalize_body(&format!("changed path {path}"))
}

fn dedupe_strings(values: Vec<String>) -> Vec<String> {
    let mut unique = Vec::new();
    for value in values {
        if !unique.iter().any(|existing| existing == &value) {
            unique.push(value);
        }
    }
    unique
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        BlockerConcern, CommentRecord, CommentSource, CommentType, GateConfigSnapshot,
        PullRequestSummary, ScanArtifact, Status,
    };

    use super::{extract_concern, gate_scan, sentence_supports_evidence};

    #[test]
    fn rejects_preference_only_comment() {
        let scan = ScanArtifact {
            status: Status::Ok,
            data_coverage: crate::domain::DataCoverage::Full,
            review_signal: crate::domain::ReviewSignal::Unknown,
            reason: None,
            scan_partial: false,
            repo_root: Some(String::from("/tmp/review-firewall")),
            branch: Some(String::from("feature/test")),
            pr: PullRequestSummary {
                title: String::from("Refactor"),
                ..PullRequestSummary::default()
            },
            files_changed: 1,
            review_comments: 1,
            threads: 1,
            codeowners_found: false,
            policy_found: false,
            product_boundary: Default::default(),
            changed_files: vec![String::from("src/api.rs")],
            comments: vec![CommentRecord {
                comment_id: String::from("1"),
                thread_id: String::from("1"),
                author: String::from("reviewer"),
                body: String::from(
                    "Nit: I prefer renaming this helper because the current name feels odd.",
                ),
                path: Some(String::from("src/api.rs")),
                source: CommentSource::ReviewComment,
                reply_to_comment_id: None,
                created_at: None,
                line: Some(1),
                original_line: Some(1),
            }],
            issue_comments: Vec::new(),
            review_threads: Vec::new(),
            partial_sources: Vec::new(),
            warnings: Vec::new(),
        };

        let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);
        assert_eq!(gate.counts.nits, 1);
        assert!(gate.residual_blockers.is_empty());
    }

    #[test]
    fn runtime_security_terms_survive_metalinguistic_context() {
        let scan = ScanArtifact {
            status: Status::Ok,
            data_coverage: crate::domain::DataCoverage::Full,
            review_signal: crate::domain::ReviewSignal::Unknown,
            reason: None,
            scan_partial: false,
            repo_root: Some(String::from("/tmp/review-firewall")),
            branch: Some(String::from("feature/test")),
            pr: PullRequestSummary {
                title: String::from("Refactor"),
                ..PullRequestSummary::default()
            },
            files_changed: 1,
            review_comments: 1,
            threads: 1,
            codeowners_found: false,
            policy_found: false,
            product_boundary: Default::default(),
            changed_files: vec![String::from("src/profile.rs")],
            comments: vec![CommentRecord {
                comment_id: String::from("1"),
                thread_id: String::from("1"),
                author: String::from("reviewer"),
                body: String::from(
                    "UI allows XSS in this PR when the profile name returns unescaped input and can leak an auth token.",
                ),
                path: Some(String::from("src/profile.rs")),
                source: CommentSource::ReviewComment,
                reply_to_comment_id: None,
                created_at: None,
                line: Some(1),
                original_line: Some(1),
            }],
            issue_comments: Vec::new(),
            review_threads: Vec::new(),
            partial_sources: Vec::new(),
            warnings: Vec::new(),
        };

        let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

        assert_eq!(gate.residual_blockers.len(), 1);
        assert_eq!(gate.residual_blockers[0].concern, BlockerConcern::Security);
    }

    #[test]
    fn question_markers_do_not_match_inside_statement_words() {
        let scan = scan_with_comment_body("This public API behavior is confusing here.");

        let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

        assert_eq!(gate.counts.questions, 0);
        assert_eq!(gate.counts.suggestions, 1);
        assert_eq!(
            gate.classified_comments[0].comment_type,
            CommentType::Suggestion
        );
    }

    #[test]
    fn question_markers_match_question_openers_without_question_mark() {
        let scan = scan_with_comment_body("Is this public API behavior intentional");

        let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

        assert_eq!(gate.counts.questions, 1);
        assert_eq!(
            gate.classified_comments[0].comment_type,
            CommentType::Question
        );
    }

    #[test]
    fn concern_keywords_use_word_boundaries_for_short_ascii_tokens() {
        assert_eq!(extract_concern("This capability label is confusing."), None);
        assert_eq!(
            extract_concern("The public API contract changes here."),
            Some(BlockerConcern::Api)
        );
    }

    #[test]
    fn evidence_markers_use_word_boundaries_for_short_ascii_tokens() {
        assert!(sentence_supports_evidence(
            "CI failed on this PR because the check failed."
        ));
        assert!(!sentence_supports_evidence(
            "This specific decision needs clarification."
        ));
    }

    fn scan_with_comment_body(body: &str) -> ScanArtifact {
        ScanArtifact {
            status: Status::Ok,
            data_coverage: crate::domain::DataCoverage::Full,
            review_signal: crate::domain::ReviewSignal::Unknown,
            reason: None,
            scan_partial: false,
            repo_root: Some(String::from("/tmp/review-firewall")),
            branch: Some(String::from("feature/test")),
            pr: PullRequestSummary {
                title: String::from("Refactor"),
                ..PullRequestSummary::default()
            },
            files_changed: 1,
            review_comments: 1,
            threads: 1,
            codeowners_found: false,
            policy_found: false,
            product_boundary: Default::default(),
            changed_files: vec![String::from("src/api.rs")],
            comments: vec![CommentRecord {
                comment_id: String::from("1"),
                thread_id: String::from("1"),
                author: String::from("reviewer"),
                body: String::from(body),
                path: Some(String::from("src/api.rs")),
                source: CommentSource::ReviewComment,
                reply_to_comment_id: None,
                created_at: None,
                line: Some(1),
                original_line: Some(1),
            }],
            issue_comments: Vec::new(),
            review_threads: Vec::new(),
            partial_sources: Vec::new(),
            warnings: Vec::new(),
        }
    }
}
