#[derive(Debug, serde::Deserialize)]
pub struct Settings {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_delay")]
    pub delay: u64,
}

fn default_model() -> String {
    "gpt-3.5-turbo".to_string()
}

fn default_delay() -> u64 { 10 }

impl Settings {
    pub async fn load() -> Result<Self, std::io::Error> {
        //Read settings from settings.json
        let file = tokio::fs::read_to_string("./settings.json").await?;
        //Deserialize settings from JSON
        let settings = serde_json::from_str(&file)?;

        Ok(settings)
    }
}