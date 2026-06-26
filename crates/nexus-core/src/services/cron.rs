use time::{OffsetDateTime, UtcOffset};

use crate::{
    error::{AppError, AppResult},
    services::util::required_trimmed,
};

/// Validate the limited 5-field cron syntax Agent Nexus currently supports.
pub fn validate_cron_schedule(schedule: &str) -> AppResult<()> {
    let fields = schedule.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 5 {
        return Err(AppError::Validation(
            "schedule must be 'manual' or a 5-field cron expression".to_string(),
        ));
    }

    validate_cron_field(fields[0], 0, 59, "minute")?;
    validate_cron_field(fields[1], 0, 23, "hour")?;
    validate_cron_field(fields[2], 1, 31, "day of month")?;
    validate_cron_field(fields[3], 1, 12, "month")?;
    validate_cron_field(fields[4], 0, 7, "day of week")?;
    Ok(())
}

/// Whether `schedule` matches `now_epoch_seconds`, rounded by the caller to minute start.
pub fn cron_schedule_matches(schedule: &str, now_epoch_seconds: i64) -> AppResult<bool> {
    validate_cron_schedule(schedule)?;
    let now = OffsetDateTime::from_unix_timestamp(now_epoch_seconds)
        .map_err(|error| AppError::Validation(format!("invalid schedule time: {error}")))?;
    cron_schedule_matches_datetime(schedule, now)
}

/// Whether `schedule` matches `now_epoch_seconds` in the current machine's local time.
pub fn cron_schedule_matches_local(schedule: &str, now_epoch_seconds: i64) -> AppResult<bool> {
    validate_cron_schedule(schedule)?;
    cron_schedule_matches_datetime(schedule, local_datetime(now_epoch_seconds)?)
}

/// First minute-aligned local-time cron occurrence strictly after `after_epoch_seconds`.
pub fn next_cron_occurrence_after_local(
    schedule: &str,
    after_epoch_seconds: i64,
) -> AppResult<i64> {
    validate_cron_schedule(schedule)?;
    let minute_start = after_epoch_seconds - after_epoch_seconds.rem_euclid(60);
    let mut candidate = minute_start + 60;
    let max_minutes = 366 * 24 * 60;

    for _ in 0..max_minutes {
        if cron_schedule_matches_local(schedule, candidate)? {
            return Ok(candidate);
        }
        candidate += 60;
    }

    Err(AppError::Validation(
        "cron schedule has no occurrence within the next year".to_string(),
    ))
}

fn cron_schedule_matches_datetime(schedule: &str, now: OffsetDateTime) -> AppResult<bool> {
    let fields = schedule.split_whitespace().collect::<Vec<_>>();
    let day_of_week = now.weekday().number_days_from_sunday() as u32;

    Ok(cron_field_matches(fields[0], now.minute() as u32, 0)?
        && cron_field_matches(fields[1], now.hour() as u32, 0)?
        && cron_field_matches(fields[2], now.day() as u32, 1)?
        && cron_field_matches(fields[3], u8::from(now.month()) as u32, 1)?
        && cron_day_of_week_matches(fields[4], day_of_week)?)
}

fn local_datetime(epoch_seconds: i64) -> AppResult<OffsetDateTime> {
    let utc = OffsetDateTime::from_unix_timestamp(epoch_seconds)
        .map_err(|error| AppError::Validation(format!("invalid schedule time: {error}")))?;
    let offset = UtcOffset::local_offset_at(utc)
        .map_err(|error| AppError::Internal(format!("read local timezone offset: {error}")))?;
    Ok(utc.to_offset(offset))
}

/// Validate and normalize a task schedule. Link actions cannot be scheduled.
pub fn normalize_task_schedule(raw: &str, action: &str) -> AppResult<String> {
    let schedule = raw.trim();
    if schedule.is_empty() || schedule == "manual" {
        return Ok("manual".to_string());
    }
    if action != "Copy" {
        return Err(AppError::Validation(
            "only Copy tasks can use a schedule".to_string(),
        ));
    }
    validate_cron_schedule(schedule)?;
    Ok(schedule.to_string())
}

fn cron_field_matches(field: &str, value: u32, range_start: u32) -> AppResult<bool> {
    if field.contains(',') {
        let mut matched = false;
        for atom in field.split(',') {
            if atom.trim().is_empty() {
                return Err(AppError::Validation("invalid cron list field".to_string()));
            }
            matched |= cron_field_atom_matches(atom, value, range_start)?;
        }
        return Ok(matched);
    }

    cron_field_atom_matches(field, value, range_start)
}

fn cron_field_atom_matches(field: &str, value: u32, range_start: u32) -> AppResult<bool> {
    if field == "*" {
        return Ok(true);
    }

    if let Some(step) = field.strip_prefix("*/") {
        let step = parse_cron_number(step, "step")?;
        if step == 0 {
            return Err(AppError::Validation("invalid cron step field".to_string()));
        }
        return Ok((value - range_start).is_multiple_of(step));
    }

    Ok(value == parse_cron_number(field, "value")?)
}

fn cron_day_of_week_matches(field: &str, value: u32) -> AppResult<bool> {
    if field == "7" {
        return Ok(value == 0);
    }
    cron_field_matches(field, value, 0)
}

fn validate_cron_field(field: &str, min: u32, max: u32, label: &str) -> AppResult<()> {
    if field.contains(',') {
        for atom in field.split(',') {
            if atom.trim().is_empty() {
                return Err(AppError::Validation(format!("invalid cron {label} field")));
            }
            validate_cron_field_atom(atom, min, max, label)?;
        }
        return Ok(());
    }

    validate_cron_field_atom(field, min, max, label)
}

fn validate_cron_field_atom(field: &str, min: u32, max: u32, label: &str) -> AppResult<()> {
    if field == "*" {
        return Ok(());
    }

    if let Some(step) = field.strip_prefix("*/") {
        let step = parse_cron_number(step, label)?;
        let range_size = max - min + 1;
        if step == 0 || step > range_size {
            return Err(AppError::Validation(format!("invalid cron {label} field")));
        }
        return Ok(());
    }

    let value = parse_cron_number(field, label)?;
    if value < min || value > max {
        return Err(AppError::Validation(format!("invalid cron {label} field")));
    }
    Ok(())
}

fn parse_cron_number(raw: &str, label: &str) -> AppResult<u32> {
    required_trimmed(raw, label)?
        .parse::<u32>()
        .map_err(|_| AppError::Validation(format!("invalid cron {label} field")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_supported_five_field_schedules() {
        validate_cron_schedule("*/5 * * * *").expect("step schedule is valid");
        validate_cron_schedule("0 5 * * *").expect("daily schedule is valid");
        validate_cron_schedule("0 5,10,15,20 * * *").expect("list schedule is valid");
    }

    #[test]
    fn rejects_invalid_step_values() {
        assert!(validate_cron_schedule("*/0 * * * *").is_err());
        assert!(validate_cron_schedule("*/61 * * * *").is_err());
        assert!(validate_cron_schedule("0 5,,10 * * *").is_err());
        assert!(validate_cron_schedule("0 5,24 * * *").is_err());
    }

    #[test]
    fn matches_step_and_literal_fields() {
        // 2026-06-21T05:10:00Z
        let now = 1_782_018_600;
        assert!(cron_schedule_matches("*/5 5 * * *", now).expect("match schedule"));
        assert!(!cron_schedule_matches("*/15 5 * * *", now).expect("miss schedule"));
    }

    #[test]
    fn matches_comma_list_fields() {
        // 2026-06-21T10:00:00Z
        let ten = 1_782_036_000;
        // 2026-06-21T11:00:00Z
        let eleven = 1_782_039_600;

        assert!(cron_schedule_matches("0 5,10,15,20 * * *", ten).expect("match schedule"));
        assert!(!cron_schedule_matches("0 5,10,15,20 * * *", eleven).expect("miss schedule"));
    }

    #[test]
    fn matches_datetime_without_assuming_system_timezone() {
        let now = OffsetDateTime::from_unix_timestamp(1_782_036_000)
            .expect("valid timestamp")
            .to_offset(UtcOffset::from_hms(8, 0, 0).expect("valid offset"));

        assert!(cron_schedule_matches_datetime("0 18 * * *", now).expect("match local datetime"));
    }

    #[test]
    fn treats_seven_as_sunday() {
        // 2026-06-21T05:00:00Z is Sunday.
        let sunday = 1_782_018_000;
        assert!(cron_schedule_matches("0 5 * * 7", sunday).expect("Sunday matches 7"));
    }

    #[test]
    fn normalizes_manual_and_rejects_scheduled_link_actions() {
        assert_eq!(
            normalize_task_schedule(" ", "Copy").expect("blank schedule"),
            "manual"
        );
        assert!(normalize_task_schedule("0 5 * * *", "Symlink").is_err());
    }
}
