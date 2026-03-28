//! fetch()-based asset loading for the Arachne WASM runtime.
//!
//! Provides async resource fetching with progress tracking and URL-based
//! asset resolution. On WASM, uses the browser Fetch API. On native,
//! provides a mock/stub implementation for testing.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Fetch error
// ---------------------------------------------------------------------------

/// Errors from fetch operations.
#[derive(Clone, Debug, PartialEq)]
pub enum FetchError {
    /// Network error (offline, DNS failure, etc.).
    NetworkError(String),
    /// HTTP error (non-2xx status).
    HttpError(u16, String),
    /// Request was aborted.
    Aborted,
    /// Invalid URL.
    InvalidUrl(String),
    /// Timeout.
    Timeout,
    /// Decoding error.
    DecodeError(String),
}

impl core::fmt::Display for FetchError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FetchError::NetworkError(msg) => write!(f, "network error: {msg}"),
            FetchError::HttpError(status, msg) => write!(f, "HTTP {status}: {msg}"),
            FetchError::Aborted => write!(f, "request aborted"),
            FetchError::InvalidUrl(url) => write!(f, "invalid URL: {url}"),
            FetchError::Timeout => write!(f, "request timed out"),
            FetchError::DecodeError(msg) => write!(f, "decode error: {msg}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Fetch request
// ---------------------------------------------------------------------------

/// A fetch request configuration.
#[derive(Clone, Debug)]
pub struct FetchRequest {
    /// The URL to fetch.
    pub url: String,
    /// HTTP method (GET, POST, etc.).
    pub method: String,
    /// Request headers.
    pub headers: HashMap<String, String>,
    /// Optional timeout in milliseconds.
    pub timeout_ms: Option<u32>,
    /// Whether to report progress.
    pub track_progress: bool,
}

impl FetchRequest {
    /// Create a simple GET request for a URL.
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: "GET".to_string(),
            headers: HashMap::new(),
            timeout_ms: None,
            track_progress: false,
        }
    }

    /// Enable progress tracking.
    pub fn with_progress(mut self) -> Self {
        self.track_progress = true;
        self
    }

    /// Set a timeout.
    pub fn with_timeout(mut self, timeout_ms: u32) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Add a header.
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
}

// ---------------------------------------------------------------------------
// Fetch response
// ---------------------------------------------------------------------------

/// A fetch response.
#[derive(Clone, Debug)]
pub struct FetchResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response headers.
    pub headers: HashMap<String, String>,
    /// Response body as bytes.
    pub body: Vec<u8>,
    /// The original URL.
    pub url: String,
}

impl FetchResponse {
    /// Get the body as a UTF-8 string.
    pub fn text(&self) -> Result<String, FetchError> {
        String::from_utf8(self.body.clone())
            .map_err(|e| FetchError::DecodeError(e.to_string()))
    }

    /// Whether the response indicates success (2xx).
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Content length in bytes.
    pub fn content_length(&self) -> usize {
        self.body.len()
    }
}

// ---------------------------------------------------------------------------
// Fetch progress
// ---------------------------------------------------------------------------

/// Progress information for an ongoing fetch.
#[derive(Clone, Copy, Debug)]
pub struct FetchProgress {
    /// Bytes received so far.
    pub bytes_received: u64,
    /// Total bytes expected (if known from Content-Length header).
    pub total_bytes: Option<u64>,
}

impl FetchProgress {
    /// Create a new progress report.
    pub fn new(bytes_received: u64, total_bytes: Option<u64>) -> Self {
        Self {
            bytes_received,
            total_bytes,
        }
    }

    /// Fraction complete (0.0 to 1.0), or None if total is unknown.
    pub fn fraction(&self) -> Option<f64> {
        self.total_bytes.map(|total| {
            if total == 0 {
                1.0
            } else {
                self.bytes_received as f64 / total as f64
            }
        })
    }

    /// Percentage complete (0 to 100), or None if total is unknown.
    pub fn percentage(&self) -> Option<u32> {
        self.fraction().map(|f| (f * 100.0).round() as u32)
    }
}

// ---------------------------------------------------------------------------
// Asset fetcher
// ---------------------------------------------------------------------------

/// State of an in-flight fetch.
#[derive(Clone, Debug)]
pub enum FetchState {
    /// Request is pending.
    Pending,
    /// Currently downloading with optional progress.
    InProgress(FetchProgress),
    /// Completed successfully.
    Complete(FetchResponse),
    /// Failed with error.
    Failed(FetchError),
}

/// Manages fetch()-based asset loading with URL resolution and progress tracking.
pub struct AssetFetcher {
    /// Base URL for resolving relative asset paths.
    base_url: String,
    /// In-flight and completed fetches, keyed by request URL.
    fetches: HashMap<String, FetchState>,
    /// Mock responses for testing on native.
    #[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
    mock_responses: HashMap<String, Result<FetchResponse, FetchError>>,
}

impl AssetFetcher {
    /// Create a new asset fetcher with the given base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        let mut url = base_url.into();
        if !url.ends_with('/') {
            url.push('/');
        }

        Self {
            base_url: url,
            fetches: HashMap::new(),
            #[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
            mock_responses: HashMap::new(),
        }
    }

    /// Get the base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Resolve a relative path against the base URL.
    pub fn resolve_url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") || path.starts_with("//") {
            // Already absolute.
            path.to_string()
        } else {
            let trimmed = path.trim_start_matches('/');
            format!("{}{}", self.base_url, trimmed)
        }
    }

    /// Start fetching an asset at the given path.
    ///
    /// The path is resolved against the base URL. Returns the full URL.
    pub fn fetch(&mut self, path: &str) -> String {
        let url = self.resolve_url(path);

        if self.fetches.contains_key(&url) {
            return url; // Already in flight or complete.
        }

        self.fetches.insert(url.clone(), FetchState::Pending);

        #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
        {
            // Real WASM implementation would:
            // 1. Create a Request object
            // 2. Call fetch(request)
            // 3. Process the Response stream
            // 4. Update self.fetches via a shared state mechanism
        }

        #[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
        {
            // On native, immediately resolve from mock responses.
            if let Some(result) = self.mock_responses.get(&url).cloned() {
                match result {
                    Ok(response) => {
                        self.fetches.insert(url.clone(), FetchState::Complete(response));
                    }
                    Err(error) => {
                        self.fetches.insert(url.clone(), FetchState::Failed(error));
                    }
                }
            }
            // If no mock response, stays Pending.
        }

        url
    }

    /// Check the state of a fetch.
    pub fn state(&self, url: &str) -> Option<&FetchState> {
        self.fetches.get(url)
    }

    /// Get a completed response if available.
    pub fn get_response(&self, url: &str) -> Option<&FetchResponse> {
        match self.fetches.get(url) {
            Some(FetchState::Complete(response)) => Some(response),
            _ => None,
        }
    }

    /// Check if a fetch is complete (success or failure).
    pub fn is_complete(&self, url: &str) -> bool {
        matches!(
            self.fetches.get(url),
            Some(FetchState::Complete(_)) | Some(FetchState::Failed(_))
        )
    }

    /// Check if a fetch succeeded.
    pub fn is_ok(&self, url: &str) -> bool {
        matches!(self.fetches.get(url), Some(FetchState::Complete(_)))
    }

    /// Remove a completed fetch from the tracker.
    pub fn take_response(&mut self, url: &str) -> Option<FetchResponse> {
        if matches!(self.fetches.get(url), Some(FetchState::Complete(_))) {
            if let Some(FetchState::Complete(response)) = self.fetches.remove(url) {
                return Some(response);
            }
        }
        None
    }

    /// Get the number of pending (in-flight) fetches.
    pub fn pending_count(&self) -> usize {
        self.fetches
            .values()
            .filter(|s| matches!(s, FetchState::Pending | FetchState::InProgress(_)))
            .count()
    }

    /// Clear all completed fetches.
    pub fn clear_completed(&mut self) {
        self.fetches.retain(|_, state| {
            matches!(state, FetchState::Pending | FetchState::InProgress(_))
        });
    }

    /// Register a mock response for testing on native.
    #[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
    pub fn mock_response(
        &mut self,
        url: impl Into<String>,
        response: Result<FetchResponse, FetchError>,
    ) {
        self.mock_responses.insert(url.into(), response);
    }

    /// Manually set the state of a fetch (for testing).
    #[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
    pub fn set_state(&mut self, url: impl Into<String>, state: FetchState) {
        self.fetches.insert(url.into(), state);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ok_response(url: &str, body: &[u8]) -> FetchResponse {
        FetchResponse {
            status: 200,
            headers: HashMap::new(),
            body: body.to_vec(),
            url: url.to_string(),
        }
    }

    #[test]
    fn fetch_error_display() {
        assert_eq!(
            FetchError::NetworkError("offline".to_string()).to_string(),
            "network error: offline"
        );
        assert_eq!(
            FetchError::HttpError(404, "not found".to_string()).to_string(),
            "HTTP 404: not found"
        );
        assert_eq!(FetchError::Aborted.to_string(), "request aborted");
        assert_eq!(FetchError::Timeout.to_string(), "request timed out");
    }

    #[test]
    fn fetch_request_builder() {
        let req = FetchRequest::get("https://example.com/data.json")
            .with_progress()
            .with_timeout(5000)
            .with_header("Accept", "application/json");

        assert_eq!(req.url, "https://example.com/data.json");
        assert_eq!(req.method, "GET");
        assert!(req.track_progress);
        assert_eq!(req.timeout_ms, Some(5000));
        assert_eq!(req.headers.get("Accept").unwrap(), "application/json");
    }

    #[test]
    fn fetch_response_text() {
        let response = FetchResponse {
            status: 200,
            headers: HashMap::new(),
            body: b"Hello, world!".to_vec(),
            url: "https://example.com".to_string(),
        };

        assert_eq!(response.text().unwrap(), "Hello, world!");
        assert!(response.ok());
        assert_eq!(response.content_length(), 13);
    }

    #[test]
    fn fetch_response_not_ok() {
        let response = FetchResponse {
            status: 404,
            headers: HashMap::new(),
            body: b"Not Found".to_vec(),
            url: "https://example.com/missing".to_string(),
        };

        assert!(!response.ok());
    }

    #[test]
    fn fetch_progress_fraction() {
        let progress = FetchProgress::new(500, Some(1000));
        assert_eq!(progress.fraction(), Some(0.5));
        assert_eq!(progress.percentage(), Some(50));
    }

    #[test]
    fn fetch_progress_unknown_total() {
        let progress = FetchProgress::new(500, None);
        assert_eq!(progress.fraction(), None);
        assert_eq!(progress.percentage(), None);
    }

    #[test]
    fn fetch_progress_zero_total() {
        let progress = FetchProgress::new(0, Some(0));
        assert_eq!(progress.fraction(), Some(1.0));
        assert_eq!(progress.percentage(), Some(100));
    }

    #[test]
    fn asset_fetcher_url_resolution() {
        let fetcher = AssetFetcher::new("https://example.com/assets");

        assert_eq!(
            fetcher.resolve_url("textures/brick.png"),
            "https://example.com/assets/textures/brick.png"
        );
        assert_eq!(
            fetcher.resolve_url("/textures/brick.png"),
            "https://example.com/assets/textures/brick.png"
        );
        assert_eq!(
            fetcher.resolve_url("https://cdn.example.com/file.bin"),
            "https://cdn.example.com/file.bin"
        );
        assert_eq!(
            fetcher.resolve_url("http://example.com/file.bin"),
            "http://example.com/file.bin"
        );
        assert_eq!(
            fetcher.resolve_url("//cdn.example.com/file.bin"),
            "//cdn.example.com/file.bin"
        );
    }

    #[test]
    fn asset_fetcher_base_url_trailing_slash() {
        let f1 = AssetFetcher::new("https://example.com/assets/");
        assert_eq!(f1.base_url(), "https://example.com/assets/");

        let f2 = AssetFetcher::new("https://example.com/assets");
        assert_eq!(f2.base_url(), "https://example.com/assets/");
    }

    #[test]
    fn asset_fetcher_mock_fetch_success() {
        let mut fetcher = AssetFetcher::new("https://example.com/assets/");

        let url = fetcher.resolve_url("scene.json");
        let body = br#"{"entities":[]}"#;
        fetcher.mock_response(url.clone(), Ok(make_ok_response(&url, body)));

        let fetched_url = fetcher.fetch("scene.json");
        assert_eq!(fetched_url, url);
        assert!(fetcher.is_complete(&url));
        assert!(fetcher.is_ok(&url));

        let response = fetcher.get_response(&url).unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.text().unwrap(), r#"{"entities":[]}"#);
    }

    #[test]
    fn asset_fetcher_mock_fetch_failure() {
        let mut fetcher = AssetFetcher::new("https://example.com/assets/");

        let url = fetcher.resolve_url("missing.png");
        fetcher.mock_response(
            url.clone(),
            Err(FetchError::HttpError(404, "not found".to_string())),
        );

        fetcher.fetch("missing.png");
        assert!(fetcher.is_complete(&url));
        assert!(!fetcher.is_ok(&url));
    }

    #[test]
    fn asset_fetcher_pending_without_mock() {
        let mut fetcher = AssetFetcher::new("https://example.com/");

        let url = fetcher.fetch("unknown.bin");
        // No mock registered, so it stays pending.
        assert!(!fetcher.is_complete(&url));
        assert_eq!(fetcher.pending_count(), 1);
    }

    #[test]
    fn asset_fetcher_take_response() {
        let mut fetcher = AssetFetcher::new("https://example.com/");

        let url = fetcher.resolve_url("data.bin");
        fetcher.mock_response(url.clone(), Ok(make_ok_response(&url, b"data")));
        fetcher.fetch("data.bin");

        let response = fetcher.take_response(&url).unwrap();
        assert_eq!(response.body, b"data");

        // After taking, the fetch is no longer tracked.
        assert!(fetcher.get_response(&url).is_none());
    }

    #[test]
    fn asset_fetcher_duplicate_fetch_returns_same_url() {
        let mut fetcher = AssetFetcher::new("https://example.com/");

        let url = fetcher.resolve_url("data.bin");
        fetcher.mock_response(url.clone(), Ok(make_ok_response(&url, b"data")));

        let url1 = fetcher.fetch("data.bin");
        let url2 = fetcher.fetch("data.bin");
        assert_eq!(url1, url2);
    }

    #[test]
    fn asset_fetcher_clear_completed() {
        let mut fetcher = AssetFetcher::new("https://example.com/");

        let url = fetcher.resolve_url("file.txt");
        fetcher.mock_response(url.clone(), Ok(make_ok_response(&url, b"hello")));
        fetcher.fetch("file.txt");

        assert!(fetcher.is_complete(&url));
        fetcher.clear_completed();
        assert!(fetcher.state(&url).is_none());
    }

    #[test]
    fn asset_fetcher_pending_count() {
        let mut fetcher = AssetFetcher::new("https://example.com/");

        let url = fetcher.resolve_url("done.txt");
        fetcher.mock_response(url.clone(), Ok(make_ok_response(&url, b"ok")));

        fetcher.fetch("done.txt");   // Completes immediately via mock.
        fetcher.fetch("pending.txt"); // No mock, stays pending.

        assert_eq!(fetcher.pending_count(), 1);
    }

    #[test]
    fn fetch_response_invalid_utf8() {
        let response = FetchResponse {
            status: 200,
            headers: HashMap::new(),
            body: vec![0xFF, 0xFE, 0xFD],
            url: "https://example.com/binary".to_string(),
        };

        assert!(response.text().is_err());
    }
}
