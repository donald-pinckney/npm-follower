use api::{ApiError, ApiResult};
use chrono::NaiveDate;
use lazy_static::lazy_static;
use postgres_db::{
    custom_types::DownloadCount, download_metrics::DownloadMetric, packages::Package,
};

pub mod api;

lazy_static!(
    pub static ref LOWER_BOUND_DATE: NaiveDate = chrono::NaiveDate::from_ymd_opt(2015, 1, 10).unwrap();
    // NOTE: we remove three days because:
    //  1. we remove 1 day beacuse of time zones
    //  2. we remove 1 day because the data of "today" is not yet complete
    //  3. we remove 1 other day because NPM's api only publishes data for "today" the day after
    pub static ref UPPER_BOUND_DATE: NaiveDate = chrono::Utc::now().date_naive()
                                                 - chrono::Duration::days(3);
);

pub async fn make_download_metric(
    pkg: &Package,
    api_result: &ApiResult,
) -> Result<DownloadMetric, ApiError> {
    // we need to convert the results into DownloadMetric, merging daily results
    // into weekly results
    let mut weekly_results: Vec<DownloadCount> = Vec::new();
    let mut i = 0;
    let mut total_downloads = 0;

    loop {
        let mut weekly_count = api_result.downloads[i].downloads;
        let mut j = i + 1;
        while j < api_result.downloads.len() && j < i + 7 {
            weekly_count += api_result.downloads[j].downloads;
            j += 1;
        }

        let date =
            chrono::NaiveDate::parse_from_str(&api_result.downloads[i].day, "%Y-%m-%d").unwrap();

        // we set i to j so that we skip the days we already counted
        i = j;

        total_downloads += weekly_count;

        let count = DownloadCount {
            date,
            count: weekly_count,
        };

        // we don't insert zero counts
        if weekly_count > 0 {
            weekly_results.push(count);
        }

        if i >= api_result.downloads.len() {
            // we still want to know the latest, even if it's zero and we didn't insert it
            let latest = date;
            println!("did package {}", pkg.name);
            return Ok(DownloadMetric::new(
                pkg.id,
                weekly_results,
                total_downloads,
                latest,
            ));
        }
    }
}
