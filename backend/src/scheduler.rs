use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use tokio::sync::broadcast;

use crate::models::ScheduleConfig;

pub const MAX_SCHEDULE_RECHECK_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerWaitOutcome {
    RunScheduledCheck,
    RecheckSchedule,
}

pub fn normalized_cron_expression(cron_expression: &str) -> String {
    let mut normalized = if cron_expression.split_whitespace().count() == 5 {
        format!("0 {}", cron_expression)
    } else {
        cron_expression.to_string()
    };

    let field_max = [59, 59, 23, 31, 12, 7];
    let fields: Vec<&str> = normalized.split_whitespace().collect();

    if fields.len() == 6 {
        normalized = fields
            .iter()
            .enumerate()
            .map(|(i, field)| {
                if let Some(rest) = field.strip_prefix("*/") {
                    if let Ok(step) = rest.parse::<u32>() {
                        if step > field_max[i] {
                            return "0".to_string();
                        }
                    }
                }
                field.to_string()
            })
            .collect::<Vec<_>>()
            .join(" ");
    }

    normalized
}

pub fn next_cron_run_at(
    schedule: &ScheduleConfig,
    after: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, cron::error::Error> {
    let cron_schedule =
        cron::Schedule::from_str(&normalized_cron_expression(&schedule.cron_expression))?;

    let next_run = match schedule.timezone.parse::<Tz>() {
        Ok(tz) => {
            let local_after = after.with_timezone(&tz);
            cron_schedule
                .after(&local_after)
                .next()
                .map(|dt| dt.with_timezone(&Utc))
        }
        Err(_) => cron_schedule.after(&after).next(),
    };

    Ok(next_run)
}

pub fn next_cron_run_at_string(schedule: &ScheduleConfig, after: DateTime<Utc>) -> Option<String> {
    next_cron_run_at(schedule, after)
        .ok()
        .flatten()
        .map(|dt| dt.to_rfc3339())
}

pub async fn wait_for_scheduler_event(
    next_run_at: DateTime<Utc>,
    schedule_change_rx: &mut broadcast::Receiver<()>,
    max_recheck_interval: Duration,
) -> SchedulerWaitOutcome {
    let remaining = match (next_run_at - Utc::now()).to_std() {
        Ok(duration) => duration,
        Err(_) => return SchedulerWaitOutcome::RunScheduledCheck,
    };

    let sleep_duration = remaining.min(max_recheck_interval);

    tokio::select! {
        _ = tokio::time::sleep(sleep_duration) => {
            if Utc::now() >= next_run_at {
                SchedulerWaitOutcome::RunScheduledCheck
            } else {
                SchedulerWaitOutcome::RecheckSchedule
            }
        }
        recv = schedule_change_rx.recv() => match recv {
            Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) | Err(broadcast::error::RecvError::Closed) => {
                SchedulerWaitOutcome::RecheckSchedule
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalized_cron_expression_adds_seconds_for_five_fields() {
        assert_eq!(normalized_cron_expression("*/5 * * * *"), "0 */5 * * * *");
    }

    #[test]
    fn test_next_cron_run_at_supports_five_field_cron() {
        let schedule = ScheduleConfig {
            cron_expression: "*/5 * * * *".to_string(),
            timezone: "UTC".to_string(),
        };
        let after = chrono::DateTime::parse_from_rfc3339("2026-09-14T05:44:37Z")
            .unwrap()
            .with_timezone(&Utc);

        let next = next_cron_run_at(&schedule, after).unwrap().unwrap();

        assert_eq!(next.to_rfc3339(), "2026-09-14T05:45:00+00:00");
    }

    #[tokio::test]
    async fn test_wait_for_scheduler_event_rechecks_on_notification() {
        let (tx, mut rx) = broadcast::channel(1);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let _ = tx.send(());
        });

        let outcome = wait_for_scheduler_event(
            Utc::now() + chrono::Duration::seconds(5),
            &mut rx,
            Duration::from_secs(30),
        )
        .await;

        assert_eq!(outcome, SchedulerWaitOutcome::RecheckSchedule);
    }

    #[tokio::test]
    async fn test_wait_for_scheduler_event_rechecks_on_poll_interval() {
        let (_tx, mut rx) = broadcast::channel(1);

        let outcome = wait_for_scheduler_event(
            Utc::now() + chrono::Duration::milliseconds(100),
            &mut rx,
            Duration::from_millis(10),
        )
        .await;

        assert_eq!(outcome, SchedulerWaitOutcome::RecheckSchedule);
    }
}
