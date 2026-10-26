use std::env;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

pub struct Config {
    /// subtrans' data folder, with costs.jsonl and state.json.
    pub data_dir: PathBuf,
    /// subtrans logs OpenRouter's exact cost in dollars; same rate as subtrans' own EUR_PER_USD.
    pub eur_per_usd: f64,
    pub port: u16,
    pub poll: Duration,
    /// The built frontend; not served when the folder doesn't exist (API-only during development).
    pub static_dir: PathBuf,
}

fn var<T: FromStr>(name: &str, default: T) -> Result<T, String> {
    match env::var(name) {
        Ok(value) => value.parse().map_err(|_| format!("{name}={value} is not valid")),
        Err(_) => Ok(default),
    }
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let eur_per_usd: f64 = var("EUR_PER_USD", 0.86)?;
        if !eur_per_usd.is_finite() || eur_per_usd <= 0.0 {
            return Err("EUR_PER_USD must be above 0".into());
        }
        Ok(Self {
            data_dir: var("DATA_DIR", PathBuf::from("/data"))?,
            eur_per_usd,
            port: var("PORT", 8080)?,
            poll: Duration::from_secs(var::<u64>("POLL_SECONDS", 15)?.max(1)),
            static_dir: var("STATIC_DIR", PathBuf::from("web/dist"))?,
        })
    }
}
