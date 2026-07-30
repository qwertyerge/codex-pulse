use anyhow::Context;
use serde_json::Value;

use crate::model::WeeklyQuota;

const WEEKLY_WINDOW_MINUTES: i64 = 10_080;

pub fn weekly_quota_from_message(
    line: &str,
    observed_at_ms: i64,
) -> anyhow::Result<Option<WeeklyQuota>> {
    let message: Value =
        serde_json::from_str(line).context("parse Codex App Server JSONL message")?;

    let rate_limits = match message.get("method").and_then(Value::as_str) {
        Some("account/rateLimits/updated") => message.pointer("/params/rateLimits"),
        Some(_) => return Ok(None),
        None => message
            .pointer("/result/rateLimitsByLimitId/codex")
            .or_else(|| message.pointer("/result/rateLimits")),
    };

    Ok(rate_limits.and_then(|value| parse_default_weekly_quota(value, observed_at_ms)))
}

fn parse_default_weekly_quota(rate_limits: &Value, observed_at_ms: i64) -> Option<WeeklyQuota> {
    if rate_limits.get("limitId").and_then(Value::as_str) != Some("codex") {
        return None;
    }

    let bucket = ["primary", "secondary"].into_iter().find_map(|slot| {
        rate_limits.get(slot).filter(|candidate| {
            candidate.get("windowDurationMins").and_then(Value::as_i64)
                == Some(WEEKLY_WINDOW_MINUTES)
        })
    })?;
    let used_percent = bucket
        .get("usedPercent")
        .and_then(Value::as_f64)?
        .round()
        .clamp(0.0, 100.0) as u8;
    let resets_at_ms = bucket
        .get("resetsAt")
        .and_then(Value::as_i64)?
        .saturating_mul(1_000);

    Some(WeeklyQuota {
        used_percent,
        remaining_percent: 100 - used_percent,
        resets_at_ms,
        observed_at_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::weekly_quota_from_message;

    #[test]
    fn parses_default_codex_weekly_bucket_from_read_response() {
        let line = r#"{
            "id": 2,
            "result": {
                "rateLimits": {
                    "limitId": "codex_bengalfox",
                    "primary": {
                        "usedPercent": 0.0,
                        "windowDurationMins": 10080,
                        "resetsAt": 1786000000
                    }
                },
                "rateLimitsByLimitId": {
                    "codex": {
                        "limitId": "codex",
                        "primary": {
                            "usedPercent": 5.4,
                            "windowDurationMins": 10080,
                            "resetsAt": 1785814394
                        }
                    }
                }
            }
        }"#;

        let quota = weekly_quota_from_message(line, 1_234)
            .unwrap()
            .expect("default quota");

        assert_eq!(quota.used_percent, 5);
        assert_eq!(quota.remaining_percent, 95);
        assert_eq!(quota.resets_at_ms, 1_785_814_394_000);
        assert_eq!(quota.observed_at_ms, 1_234);
    }

    #[test]
    fn ignores_spark_only_read_response() {
        let line = r#"{
            "id": 2,
            "result": {
                "rateLimitsByLimitId": {
                    "codex_bengalfox": {
                        "limitId": "codex_bengalfox",
                        "primary": {
                            "usedPercent": 22,
                            "windowDurationMins": 10080,
                            "resetsAt": 1786000000
                        }
                    }
                }
            }
        }"#;

        assert!(weekly_quota_from_message(line, 1_234).unwrap().is_none());
    }

    #[test]
    fn selects_only_the_weekly_window() {
        let line = r#"{
            "id": 2,
            "result": {
                "rateLimitsByLimitId": {
                    "codex": {
                        "limitId": "codex",
                        "primary": {
                            "usedPercent": 80,
                            "windowDurationMins": 300,
                            "resetsAt": 1785800000
                        },
                        "secondary": {
                            "usedPercent": 7.6,
                            "windowDurationMins": 10080,
                            "resetsAt": 1785900000
                        }
                    }
                }
            }
        }"#;

        let quota = weekly_quota_from_message(line, 9)
            .unwrap()
            .expect("weekly secondary bucket");

        assert_eq!(quota.used_percent, 8);
        assert_eq!(quota.resets_at_ms, 1_785_900_000_000);
    }

    #[test]
    fn parses_default_codex_update_notification() {
        let line = r#"{
            "method": "account/rateLimits/updated",
            "params": {
                "rateLimits": {
                    "limitId": "codex",
                    "primary": {
                        "usedPercent": 6,
                        "windowDurationMins": 10080,
                        "resetsAt": 1785814394
                    }
                }
            }
        }"#;

        let quota = weekly_quota_from_message(line, 2_468)
            .unwrap()
            .expect("notification quota");

        assert_eq!(quota.used_percent, 6);
        assert_eq!(quota.observed_at_ms, 2_468);
    }

    #[test]
    fn returns_none_for_unrelated_or_invalid_messages() {
        assert!(weekly_quota_from_message(r#"{"id":1,"result":{}}"#, 1)
            .unwrap()
            .is_none());
        assert!(weekly_quota_from_message(
            r#"{
                "method":"account/rateLimits/updated",
                "params":{
                    "rateLimits":{
                        "limitId":"codex",
                        "primary":{
                            "usedPercent":5,
                            "windowDurationMins":300,
                            "resetsAt":1785814394
                        }
                    }
                }
            }"#,
            1
        )
        .unwrap()
        .is_none());
        assert!(weekly_quota_from_message(r#"{"method":"other"}"#, 1)
            .unwrap()
            .is_none());
    }
}
