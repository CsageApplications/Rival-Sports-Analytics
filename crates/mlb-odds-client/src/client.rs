//! HTTP client for The Odds API

use reqwest::{Client, StatusCode};
use std::sync::Arc;
use tracing::{debug, instrument};

use crate::error::OddsApiError;
use crate::rate_limiter::RateLimiter;
use crate::types::*;
use mlb_core::models::{Event, EventOdds, MarketType};

const BASE_URL: &str = "https://api.the-odds-api.com/v4";
const MLB_SPORT_KEY: &str = "baseball_mlb";

/// The only bookmakers this app surfaces. Restricting the `bookmakers`
/// query param (instead of `regions`) also means the API only returns —
/// and only charges quota for — these two books.
pub const ALLOWED_BOOKMAKERS: &[&str] = &["fanduel", "draftkings"];

/// Client for The Odds API
#[derive(Debug, Clone)]
pub struct OddsClient {
    client: Client,
    api_key: String,
    rate_limiter: Arc<RateLimiter>,
}

impl OddsClient {
    /// Create a new client with your API key
    pub fn new(api_key: impl Into<String>) -> Result<Self, OddsApiError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(OddsApiError::Request)?;

        Ok(Self {
            client,
            api_key: api_key.into(),
            rate_limiter: Arc::new(RateLimiter::default()),
        })
    }

    /// Create client with custom rate limiter
    pub fn with_rate_limiter(api_key: impl Into<String>, rate_limiter: RateLimiter) -> Result<Self, OddsApiError> {
        let mut client = Self::new(api_key)?;
        client.rate_limiter = Arc::new(rate_limiter);
        Ok(client)
    }

    /// Get the rate limiter for checking usage
    pub fn rate_limiter(&self) -> &RateLimiter {
        &self.rate_limiter
    }

    /// List all available sports
    #[instrument(skip(self))]
    pub async fn get_sports(&self) -> Result<Vec<ApiSport>, OddsApiError> {
        let url = format!("{}/sports", BASE_URL);
        self.get(&url, &[]).await
    }

    /// Get upcoming events for a sport (defaults to MLB)
    #[instrument(skip(self))]
    pub async fn get_events(&self, sport_key: &str) -> Result<Vec<Event>, OddsApiError> {
        let url = format!("{}/sports/{}/events", BASE_URL, sport_key);
        let events: Vec<ApiEvent> = self.get(&url, &[]).await?;
        Ok(events.into_iter().map(Into::into).collect())
    }

    /// Get MLB events
    pub async fn get_mlb_events(&self) -> Result<Vec<Event>, OddsApiError> {
        self.get_events(MLB_SPORT_KEY).await
    }

    /// Get odds for events, restricted to a specific set of bookmaker keys
    /// (e.g. `["fanduel", "draftkings"]`). When bookmakers are supplied the
    /// `regions` param is omitted, per The Odds API's own filtering rules.
    #[instrument(skip(self))]
    pub async fn get_odds(
        &self,
        sport_key: &str,
        markets: &[MarketType],
        regions: &[Region],
        bookmakers: &[&str],
    ) -> Result<Vec<(Event, EventOdds)>, OddsApiError> {
        let url = format!("{}/sports/{}/odds", BASE_URL, sport_key);

        let markets_str: String = markets
            .iter()
            .map(|m| m.as_api_key())
            .collect::<Vec<_>>()
            .join(",");

        let regions_str: String = regions
            .iter()
            .map(|r| r.as_str())
            .collect::<Vec<_>>()
            .join(",");

        let bookmakers_str: String = bookmakers.join(",");

        let mut params = vec![
            ("markets", markets_str.as_str()),
            ("oddsFormat", "american"),
        ];
        if bookmakers.is_empty() {
            params.push(("regions", regions_str.as_str()));
        } else {
            params.push(("bookmakers", bookmakers_str.as_str()));
        }

        let events: Vec<ApiEventOdds> = self.get(&url, &params).await?;
        Ok(events.iter().map(|e| e.to_domain()).collect())
    }

    /// Get MLB odds restricted to FanDuel and DraftKings only (h2h, spreads, totals).
    pub async fn get_mlb_odds(&self) -> Result<Vec<(Event, EventOdds)>, OddsApiError> {
        self.get_odds(
            MLB_SPORT_KEY,
            &[MarketType::H2h, MarketType::Spreads, MarketType::Totals],
            &[Region::Us],
            ALLOWED_BOOKMAKERS,
        )
        .await
    }

    /// Get player props for a specific MLB event, restricted to FanDuel and
    /// DraftKings only.
    ///
    /// Fetches all available batter and pitcher prop markets.
    /// Note: costs 1 API request per event (not per market).
    #[instrument(skip(self))]
    pub async fn get_mlb_player_props(
        &self,
        event_id: &str,
    ) -> Result<(Event, EventOdds), OddsApiError> {
        self.get_event_odds(
            MLB_SPORT_KEY,
            event_id,
            MarketType::mlb_player_props(),
            &[Region::Us],
            ALLOWED_BOOKMAKERS,
        )
        .await
    }

    /// Get odds for a specific event, restricted to a specific set of
    /// bookmaker keys. When bookmakers are supplied the `regions` param is
    /// omitted, per The Odds API's own filtering rules.
    #[instrument(skip(self))]
    pub async fn get_event_odds(
        &self,
        sport_key: &str,
        event_id: &str,
        markets: &[MarketType],
        regions: &[Region],
        bookmakers: &[&str],
    ) -> Result<(Event, EventOdds), OddsApiError> {
        let url = format!("{}/sports/{}/events/{}/odds", BASE_URL, sport_key, event_id);

        let markets_str: String = markets
            .iter()
            .map(|m| m.as_api_key())
            .collect::<Vec<_>>()
            .join(",");

        let regions_str: String = regions
            .iter()
            .map(|r| r.as_str())
            .collect::<Vec<_>>()
            .join(",");

        let bookmakers_str: String = bookmakers.join(",");

        let mut params = vec![
            ("markets", markets_str.as_str()),
            ("oddsFormat", "american"),
        ];
        if bookmakers.is_empty() {
            params.push(("regions", regions_str.as_str()));
        } else {
            params.push(("bookmakers", bookmakers_str.as_str()));
        }

        let event: ApiEventOdds = self.get(&url, &params).await?;
        Ok(event.to_domain())
    }

    /// Get scores for a sport
    #[instrument(skip(self))]
    pub async fn get_scores(&self, sport_key: &str, days_from: Option<u8>) -> Result<Vec<ApiScore>, OddsApiError> {
        let url = format!("{}/sports/{}/scores", BASE_URL, sport_key);
        let params: Vec<(&str, String)> = match days_from {
            Some(days) => vec![("daysFrom", days.to_string())],
            None => vec![],
        };
        let params: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get(&url, &params).await
    }

    /// Get MLB scores
    pub async fn get_mlb_scores(&self, days_from: Option<u8>) -> Result<Vec<ApiScore>, OddsApiError> {
        self.get_scores(MLB_SPORT_KEY, days_from).await
    }

    /// Make a GET request to the API
    async fn get<T: serde::de::DeserializeOwned>(&self, url: &str, params: &[(&str, &str)]) -> Result<T, OddsApiError> {
        // Rate limit
        self.rate_limiter.acquire().await;

        debug!(url, "Making API request");

        let response = self
            .client
            .get(url)
            .query(&[("apiKey", self.api_key.as_str())])
            .query(params)
            .send()
            .await?;

        // Update rate limiter from headers
        self.rate_limiter.update_from_headers(response.headers());

        let status = response.status();
        match status {
            StatusCode::OK => {
                let text = response.text().await?;
                serde_json::from_str(&text).map_err(|e| OddsApiError::Parse(format!("{}: {}", e, &text[..text.len().min(200)])))
            }
            StatusCode::UNAUTHORIZED => Err(OddsApiError::Unauthorized),
            StatusCode::TOO_MANY_REQUESTS => Err(OddsApiError::RateLimited {
                remaining: self.rate_limiter.remaining(),
            }),
            _ => {
                let message = response.text().await.unwrap_or_default();
                Err(OddsApiError::ApiError {
                    status: status.as_u16(),
                    message,
                })
            }
        }
    }
}
