use crate::dedupe::collapse_duplicates;
use crate::domain::CodeownerRule;
use crate::domain::{
    BlockerConcern, ClassifiedComment, CommentRecord, CommentType, GateArtifact,
    GateConfigSnapshot, GateCounts, ResidualBlocker, ScanArtifact, Status,
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

    let duplicates_collapsed = collapse_duplicates(&mut classified);
    let candidate_blockers = classified
        .iter()
        .filter(|comment| {
            comment.duplicate_of_comment_id.is_none()
                && !is_pr_author_comment(&comment.comment, scan)
                && is_candidate_blocker(comment, config)
        })
        .map(to_residual_blocker)
        .collect::<Vec<_>>();

    let residual_blockers = collect_residual_blockers(&classified);
    let downgraded_comments = classified
        .iter()
        .filter(|comment| {
            comment.comment_type != CommentType::Blocker
                && comment.concern.is_some()
                && (comment.failure_mode.is_none()
                    || comment.evidence.is_empty()
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

    GateArtifact {
        status,
        reason: scan.reason.clone(),
        comments_analyzed: classified.len(),
        residual_blockers,
        counts,
        candidate_blockers,
        downgraded_comments,
        duplicates_collapsed,
        warnings: scan.warnings.clone(),
        config_snapshot: config.clone(),
        classified_comments: classified,
        escalation_candidates: evaluate_escalation_candidates(
            &scan.review_threads,
            config.max_pr_thread_roundtrips,
        ),
    }
}

fn is_pr_author_comment(comment: &CommentRecord, scan: &ScanArtifact) -> bool {
    !scan.pr.author.is_empty() && comment.author == scan.pr.author
}

fn classify_comment(
    comment: &CommentRecord,
    scan: &ScanArtifact,
    config: &GateConfigSnapshot,
    codeowner_rules: &[CodeownerRule],
) -> ClassifiedComment {
    let concern = extract_concern(&comment.body);
    let failure_mode = extract_failure_mode(&comment.body);
    let evidence = extract_evidence(comment, scan);
    let present_pr_impact = extract_present_pr_impact(
        comment,
        failure_mode.as_deref(),
        config.require_failure_mode,
        concern,
        !evidence.is_empty(),
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

    let candidate_blocker = !author_comment
        && (!config.require_concern || concern.is_some())
        && (!config.require_failure_mode || failure_mode.is_some())
        && (!config.require_evidence || !evidence.is_empty())
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
        evidence,
        present_pr_impact,
        owner_match: ownership.owner_match,
        ownership_scope: ownership.ownership_scope,
        advisory_weight: ownership.advisory_weight,
        duplicate_of_comment_id: None,
    }
}

fn collect_residual_blockers(classified: &[ClassifiedComment]) -> Vec<ResidualBlocker> {
    let mut seen_threads = Vec::<String>::new();
    let mut residual = Vec::new();

    for comment in classified.iter().filter(|comment| {
        comment.comment_type == CommentType::Blocker && comment.duplicate_of_comment_id.is_none()
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
        && (!config.require_evidence || !comment.evidence.is_empty())
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
            || contains_any(
                &normalized,
                &[
                    "break",
                    "broken",
                    "fail",
                    "fails",
                    "incorrect",
                    "wrong",
                    "regress",
                    "timeout",
                    "leak",
                    "drop",
                    "incompatible",
                    "panic",
                    "crash",
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
            )
    })
}

fn extract_evidence(comment: &CommentRecord, scan: &ScanArtifact) -> Vec<String> {
    let mut evidence = Vec::<String>::new();

    for snippet in backtick_fragments(&comment.body) {
        evidence.push(format!("comment references `{snippet}`"));
    }

    for sentence in split_sentences(&comment.body) {
        let normalized = normalize_body(&sentence);
        if contains_any(
            &normalized,
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
                "because ",
                "理由:",
                "根拠",
                "証拠",
            ],
        ) {
            evidence.push(sentence);
            break;
        }
    }

    if !evidence.is_empty()
        && let Some(path) = comment.path.as_ref()
        && scan.changed_files.iter().any(|changed| changed == path)
    {
        evidence.push(format!("changed path {path}"));
    }

    dedupe_strings(evidence)
}

fn extract_present_pr_impact(
    comment: &CommentRecord,
    failure_mode: Option<&str>,
    require_failure_mode: bool,
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
    failure_mode.is_some() || (!require_failure_mode && concern.is_some() && evidence_present)
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
        || contains_any(
            &lower,
            &[
                "why ",
                "how ",
                "what ",
                "when ",
                "where ",
                "can ",
                "could ",
                "would ",
                "is ",
                "are ",
                "should ",
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

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn count_matches(text: &str, needles: &[&str]) -> usize {
    needles
        .iter()
        .filter(|needle| text.contains(**needle))
        .count()
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
        CommentRecord, CommentSource, GateConfigSnapshot, PullRequestSummary, ScanArtifact, Status,
    };

    use super::gate_scan;

    #[test]
    fn rejects_preference_only_comment() {
        let scan = ScanArtifact {
            status: Status::Ok,
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
}
