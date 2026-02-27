use actix_web::{web, HttpResponse};

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

/// Convert token units (6-decimal integer string) to a human-readable USD string.
/// e.g. "142000" -> "$0.142"
fn token_amount_to_usd(amount: &str) -> String {
    let units: u128 = amount.parse().unwrap_or(0);
    let dollars = units / 1_000_000;
    let cents = units % 1_000_000;
    // Show up to 3 decimal places, trimming trailing zeros
    let raw = format!("{}.{:06}", dollars, cents);
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
    let (total_payments, total_revenue) = state.db.get_global_stats()?;

    let endpoints: Vec<serde_json::Value> = stats
        .iter()
        .map(|s| {
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

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "platform_address": format!("{:#x}", state.config.platform_address),
        "endpoints": endpoints,
        "total_revenue": total_revenue,
        "total_revenue_usd": token_amount_to_usd(&total_revenue),
        "total_payments": total_payments,
    })))
}

/// GET /analytics/{slug} — returns stats for a single endpoint
pub async fn get_analytics(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GatewayError> {
    let slug = path.into_inner().to_lowercase();

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
