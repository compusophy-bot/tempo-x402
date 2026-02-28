use actix_web::{web, HttpResponse};

use x402::constants::TOKEN_DECIMALS;

use crate::error::GatewayError;
use crate::state::AppState;

/// Pagination query parameters for analytics
#[derive(Debug, serde::Deserialize)]
pub struct AnalyticsPagination {
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

fn default_limit() -> u32 {
    100
}

/// Convert token units to a human-readable USD string.
/// e.g. "142000" -> "$0.142" (assuming 6 decimals)
fn token_amount_to_usd(amount: &str) -> String {
    let units: u128 = amount.parse().unwrap_or(0);
    let multiplier = 10u128.pow(TOKEN_DECIMALS);
    let dollars = units / multiplier;
    let fraction = units % multiplier;
    // Show up to TOKEN_DECIMALS places, trimming trailing zeros
    let raw = format!("{}.{:0width$}", dollars, fraction, width = TOKEN_DECIMALS as usize);
    let trimmed = raw.trim_end_matches('0');
    let trimmed = trimmed.trim_end_matches('.');
    format!("${}", trimmed)
}

/// GET /analytics — returns all endpoint stats (paginated)
pub async fn list_analytics(
    query: web::Query<AnalyticsPagination>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GatewayError> {
    let stats = state.db.list_endpoint_stats(query.limit, query.offset)?;

    let mut total_revenue: u128 = 0;
    let mut total_payments: i64 = 0;

    let endpoints: Vec<serde_json::Value> = stats
        .iter()
        .map(|s| {
            let rev: u128 = s.revenue_total.parse().unwrap_or(0);
            total_revenue += rev;
            total_payments += s.payment_count;

            serde_json::json!({
                "slug": s.slug,
                "request_count": s.request_count,
                "payment_count": s.payment_count,
                "revenue_total": s.revenue_total,
                "revenue_usd": token_amount_to_usd(&s.revenue_total),
                "last_accessed_at": s.last_accessed_at,
            })
        })
        .collect();

    let total_rev_str = total_revenue.to_string();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "endpoints": endpoints,
        "total_revenue": total_rev_str,
        "total_revenue_usd": token_amount_to_usd(&total_rev_str),
        "total_payments": total_payments,
    })))
}

/// GET /analytics/{slug} — returns stats for a single endpoint
pub async fn get_analytics(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GatewayError> {
    let slug = path.into_inner();

    let stats = state
        .db
        .get_endpoint_stats(&slug)?
        .ok_or_else(|| GatewayError::EndpointNotFound(slug.clone()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "slug": stats.slug,
        "request_count": stats.request_count,
        "payment_count": stats.payment_count,
        "revenue_total": stats.revenue_total,
        "revenue_usd": token_amount_to_usd(&stats.revenue_total),
        "last_accessed_at": stats.last_accessed_at,
    })))
}

/// Configure analytics routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/analytics").route(web::get().to(list_analytics)))
        .service(web::resource("/analytics/{slug}").route(web::get().to(get_analytics)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_amount_to_usd() {
        assert_eq!(token_amount_to_usd("0"), "$0");
        assert_eq!(token_amount_to_usd("1000"), "$0.001");
        assert_eq!(token_amount_to_usd("10000"), "$0.01");
        assert_eq!(token_amount_to_usd("142000"), "$0.142");
        assert_eq!(token_amount_to_usd("1000000"), "$1");
        assert_eq!(token_amount_to_usd("1500000"), "$1.5");
        assert_eq!(token_amount_to_usd("1234567"), "$1.234567");
    }
}
