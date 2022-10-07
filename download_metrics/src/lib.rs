use chrono::NaiveDate;
use lazy_static::lazy_static;

pub mod api;

lazy_static!(
    pub static ref LOWER_BOUND_DATE: NaiveDate = chrono::NaiveDate::from_ymd(2015, 1, 10);
    // NOTE: we remove three days because:
    //  1. we remove 1 day beacuse of time zones
    //  2. we remove 1 day because the data of "today" is not yet complete
    //  3. we remove 1 other day because NPM's api only publishes data for "today" the day after
    pub static ref UPPER_BOUND_DATE: NaiveDate = chrono::Utc::now().date().naive_utc()
                                                 - chrono::Duration::days(3);
);
