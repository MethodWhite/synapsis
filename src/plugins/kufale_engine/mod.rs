//! Kufale Engine v4 - Prophetic Time Navigator (System 364)
//! 
//! Based on Enochian Solar Matrix (30+30+31)*4 = 364 days.
//! Anchor: Birth of Yeshua in 7 BC (-7) | Ascension in 26 AD.

use serde::{Deserialize, Serialize};
use chrono::{Datelike, NaiveDate, Utc, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KufaleContext {
    pub current_true_year: i32,
    pub days_since_ascension: i64,
    pub years_until_return: f32,
    pub critical_window: bool,
    pub phase: String,
}

pub struct KufaleEngine {
    pub birth_offset: i32,      // +7
    pub ascension_year: i32,    // 26 AD
    pub target_gregorian: NaiveDate, // 2027-10-10
}

impl KufaleEngine {
    pub fn new() -> Self {
        Self {
            birth_offset: 7,
            ascension_year: 26,
            target_gregorian: NaiveDate::from_ymd_opt(2027, 10, 10).unwrap(),
        }
    }

    /// Calculate the true prophetic year
    pub fn get_true_year(&self) -> i32 {
        Utc::now().year() + self.birth_offset
    }

    /// Calculate context for agents and UI
    pub fn get_prophetic_context(&self) -> KufaleContext {
        let now = Utc::now().date_naive();
        let true_year = self.get_true_year();
        
        let days_until = (self.target_gregorian - now).num_days();
        let years_until = days_until as f32 / 364.0;
        
        let status = if true_year >= 2033 { // Corresponds to 2026 Gregorian
            "RESTORATION_PHASE".to_string()
        } else {
            "PREPARATION_PHASE".to_string()
        };

        KufaleContext {
            current_true_year: true_year,
            days_since_ascension: (now - NaiveDate::from_ymd_opt(26, 4, 1).unwrap()).num_days(), // Approx
            years_until_return: years_until,
            critical_window: true_year == 2033 || true_year == 2034,
            phase: status,
        }
    }

    /// Convert any Gregorian date to Kufale (364-day matrix)
    /// Matrix structure: 4 quarters of 91 days each.
    /// Q1: M1(30), M2(30), M3(31)
    pub fn to_kufale(&self, greg_date: NaiveDate) -> (u32, u8, u8) {
        // Implementation of the 364-day day counting
        // Day 1 is always Wednesday of Spring Equinox
        let year_start = self.get_equinox_start(greg_date.year());
        let days_diff = (greg_date - year_start).num_days();
        
        if days_diff < 0 {
            // Handle last year
            return ( (greg_date.year() + self.birth_offset - 1) as u32, 12, 31);
        }

        let day_of_year = (days_diff % 364) + 1;
        let mut month = 1;
        let mut day = day_of_year;

        for m in 1..=12 {
            let m_len = if m % 3 == 0 { 31 } else { 30 };
            if day <= m_len as i64 {
                month = m;
                break;
            }
            day -= m_len as i64;
        }

        ( (greg_date.year() + self.birth_offset) as u32, month as u8, day as u8)
    }

    fn get_equinox_start(&self, year: i32) -> NaiveDate {
        // Simplified: Equinox approx March 20th
        // In Kufale, it MUST be the Wednesday on or after Equinox
        NaiveDate::from_ymd_opt(year, 3, 20).unwrap()
    }
}
