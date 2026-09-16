use crate::config::{AcousticConfig, AcousticProvider};
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcousticMood {
    pub energy: f32,
    pub valence: f32,
    pub tags: Vec<String>,
    pub provider: String,
}

impl Default for AcousticMood {
    fn default() -> Self {
        Self {
            energy: 0.5,
            valence: 0.5,
            tags: Vec::new(),
            provider: "default".into(),
        }
    }
}

#[allow(dead_code)]
pub struct AcousticClassifier {
    config: AcousticConfig,
    http: Client,
}

#[allow(dead_code)]
impl AcousticClassifier {
    pub fn new(config: AcousticConfig) -> Self {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { config, http }
    }

    fn cache_path(&self, title: &str, artist: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(artist.to_lowercase().as_bytes());
        hasher.update(b":");
        hasher.update(title.to_lowercase().as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        self.config.cache_dir.join(format!("{}.json", hash))
    }

    pub async fn classify(&self, title: &str, artist: &str) -> AcousticMood {
        let cache_file = self.cache_path(title, artist);
        if cache_file.exists()
            && let Ok(content) = std::fs::read_to_string(&cache_file)
            && let Ok(cached) = serde_json::from_str::<AcousticMood>(&content)
        {
            return cached;
        }

        // Try primary provider
        let mood = match self.config.provider {
            AcousticProvider::Lastfm => self.fetch_lastfm(title, artist).await,
            AcousticProvider::Musicbrainz => self.fetch_musicbrainz(title, artist).await,
            AcousticProvider::Local => Ok(self.classify_local(title, artist)),
        };

        let mood = match mood {
            Ok(m) => m,
            Err(_) => {
                // Try fallback provider
                match self.config.fallback {
                    AcousticProvider::Lastfm => self
                        .fetch_lastfm(title, artist)
                        .await
                        .unwrap_or_else(|_| self.classify_local(title, artist)),
                    AcousticProvider::Musicbrainz => self
                        .fetch_musicbrainz(title, artist)
                        .await
                        .unwrap_or_else(|_| self.classify_local(title, artist)),
                    AcousticProvider::Local => self.classify_local(title, artist),
                }
            }
        };

        // Cache result
        if let Some(parent) = cache_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(serialized) = serde_json::to_string(&mood) {
            let _ = std::fs::write(&cache_file, serialized);
        }

        mood
    }

    async fn fetch_lastfm(&self, title: &str, artist: &str) -> Result<AcousticMood> {
        let api_key = self
            .config
            .lastfm_api_key
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("No Last.fm API key"))?;
        let url = format!(
            "https://ws.audioscrobbler.com/2.0/?method=track.gettoptags&artist={}&track={}&api_key={}&format=json",
            urlencoding::encode(artist),
            urlencoding::encode(title),
            api_key
        );

        let text = self.http.get(&url).send().await?.text().await?;
        let resp: serde_json::Value = serde_json::from_str(&text)?;
        let mut tags = Vec::new();

        if let Some(tag_list) = resp["toptags"]["tag"].as_array() {
            for t in tag_list {
                if let Some(name) = t["name"].as_str() {
                    tags.push(name.to_lowercase());
                }
            }
        }

        if tags.is_empty() {
            return Err(anyhow::anyhow!("No Last.fm tags found"));
        }

        Ok(self.tags_to_mood(tags, "lastfm"))
    }

    async fn fetch_musicbrainz(&self, title: &str, artist: &str) -> Result<AcousticMood> {
        let query = format!("recording:\"{}\" AND artist:\"{}\"", title, artist);
        let url = format!(
            "https://musicbrainz.org/ws/2/recording/?query={}&fmt=json&limit=1",
            urlencoding::encode(&query)
        );

        let text = self
            .http
            .get(&url)
            .header(
                "User-Agent",
                "VibeVeil/0.1.0 ( https://github.com/jxoesneon/vibeveil )",
            )
            .send()
            .await?
            .text()
            .await?;
        let resp: serde_json::Value = serde_json::from_str(&text)?;

        let mut tags = Vec::new();
        if let Some(recordings) = resp["recordings"].as_array()
            && let Some(first) = recordings.first()
        {
            if let Some(tag_array) = first["tags"].as_array() {
                for t in tag_array {
                    if let Some(name) = t["name"].as_str() {
                        tags.push(name.to_lowercase());
                    }
                }
            }
            if let Some(genre_array) = first["genres"].as_array() {
                for g in genre_array {
                    if let Some(name) = g["name"].as_str() {
                        tags.push(name.to_lowercase());
                    }
                }
            }
        }

        if tags.is_empty() {
            return Err(anyhow::anyhow!("No MusicBrainz tags found"));
        }

        Ok(self.tags_to_mood(tags, "musicbrainz"))
    }

    pub fn classify_local(&self, title: &str, artist: &str) -> AcousticMood {
        let text = format!("{} {}", title.to_lowercase(), artist.to_lowercase());
        let mut tags = Vec::new();

        let keywords = [
            ("metal", 0.95, 0.4),
            ("rock", 0.85, 0.5),
            ("phonk", 0.90, 0.4),
            ("battle", 0.95, 0.5),
            ("doom", 0.90, 0.3),
            ("epic", 0.85, 0.6),
            ("techno", 0.85, 0.7),
            ("synthwave", 0.75, 0.7),
            ("cyber", 0.75, 0.6),
            ("pop", 0.65, 0.75),
            ("electro", 0.80, 0.7),
            ("lofi", 0.25, 0.6),
            ("lo-fi", 0.25, 0.6),
            ("chill", 0.25, 0.6),
            ("ambient", 0.20, 0.5),
            ("acoustic", 0.30, 0.6),
            ("piano", 0.30, 0.5),
            ("nature", 0.20, 0.6),
            ("relax", 0.20, 0.6),
            ("sleep", 0.15, 0.5),
            ("sad", 0.30, 0.2),
            ("melancholy", 0.25, 0.25),
        ];

        let mut energy_sum = 0.0;
        let mut valence_sum = 0.0;
        let mut match_count = 0;

        for (kw, e, v) in keywords {
            if text.contains(kw) {
                tags.push(kw.to_string());
                energy_sum += e;
                valence_sum += v;
                match_count += 1;
            }
        }

        let (energy, valence) = if match_count > 0 {
            (
                energy_sum / match_count as f32,
                valence_sum / match_count as f32,
            )
        } else {
            (0.5, 0.5)
        };

        AcousticMood {
            energy,
            valence,
            tags,
            provider: "local".into(),
        }
    }

    fn tags_to_mood(&self, tags: Vec<String>, provider: &str) -> AcousticMood {
        let mut energy_sum = 0.5;
        let mut valence_sum = 0.5;
        let mut count = 1;

        for t in &tags {
            let t_lower = t.to_lowercase();
            if t_lower.contains("metal")
                || t_lower.contains("hardcore")
                || t_lower.contains("phonk")
            {
                energy_sum += 0.95;
                valence_sum += 0.4;
                count += 1;
            } else if t_lower.contains("rock")
                || t_lower.contains("punk")
                || t_lower.contains("epic")
            {
                energy_sum += 0.85;
                valence_sum += 0.5;
                count += 1;
            } else if t_lower.contains("synthwave")
                || t_lower.contains("techno")
                || t_lower.contains("dance")
            {
                energy_sum += 0.80;
                valence_sum += 0.75;
                count += 1;
            } else if t_lower.contains("lofi")
                || t_lower.contains("lo-fi")
                || t_lower.contains("chill")
            {
                energy_sum += 0.25;
                valence_sum += 0.6;
                count += 1;
            } else if t_lower.contains("ambient")
                || t_lower.contains("acoustic")
                || t_lower.contains("piano")
            {
                energy_sum += 0.20;
                valence_sum += 0.55;
                count += 1;
            }
        }

        AcousticMood {
            energy: (energy_sum / count as f32).clamp(0.0, 1.0),
            valence: (valence_sum / count as f32).clamp(0.0, 1.0),
            tags,
            provider: provider.into(),
        }
    }
}

// Simple URL percent encoding helper to avoid extra crate dependency
#[allow(dead_code)]
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut encoded = String::new();
        for b in s.bytes() {
            match b {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    encoded.push(b as char);
                }
                b' ' => encoded.push('+'),
                _ => encoded.push_str(&format!("%{:02X}", b)),
            }
        }
        encoded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_classify_local_high_energy() {
        let config = AcousticConfig::default();
        let classifier = AcousticClassifier::new(config);
        let mood = classifier.classify_local("Doom Eternal Metal Battle", "Mick Gordon");
        assert!(mood.energy > 0.7);
        assert!(mood.tags.contains(&"metal".to_string()));
        assert!(mood.tags.contains(&"battle".to_string()));
    }

    #[test]
    fn test_classify_local_chill_ambient() {
        let config = AcousticConfig::default();
        let classifier = AcousticClassifier::new(config);
        let mood = classifier.classify_local("Coffee Lo-fi Chill Relax", "ChilledCow");
        assert!(mood.energy < 0.4);
        assert!(mood.tags.contains(&"chill".to_string()));
    }

    #[test]
    fn test_classify_local_neutral() {
        let config = AcousticConfig::default();
        let classifier = AcousticClassifier::new(config);
        let mood = classifier.classify_local("Unknown Track", "Unknown Artist");
        assert_eq!(mood.energy, 0.5);
        assert_eq!(mood.valence, 0.5);
        assert!(mood.tags.is_empty());
    }

    #[test]
    fn test_urlencoding() {
        assert_eq!(urlencoding::encode("Hello World"), "Hello+World");
        assert_eq!(urlencoding::encode("rock&roll"), "rock%26roll");
    }

    #[test]
    fn test_cache_path_and_tags_to_mood() {
        let tmp = TempDir::new().unwrap();
        let config = AcousticConfig {
            cache_dir: tmp.path().to_path_buf(),
            ..Default::default()
        };
        let classifier = AcousticClassifier::new(config);

        let path = classifier.cache_path("Test Song", "Test Artist");
        assert!(path.starts_with(tmp.path()));
        assert!(path.to_string_lossy().ends_with(".json"));

        let mood = classifier.tags_to_mood(
            vec!["rock".into(), "ambient".into(), "pop".into()],
            "test_provider",
        );
        assert_eq!(mood.provider, "test_provider");
        assert_eq!(mood.tags.len(), 3);
        assert!(mood.energy > 0.0 && mood.energy < 1.0);
    }

    #[tokio::test]
    async fn test_async_classify_with_cache() {
        let tmp = TempDir::new().unwrap();
        let config = AcousticConfig {
            cache_dir: tmp.path().to_path_buf(),
            provider: AcousticProvider::Local,
            ..Default::default()
        };
        let classifier = AcousticClassifier::new(config);

        let mood = classifier
            .classify("Calm Ambient Meditation", "Artist")
            .await;
        assert!(mood.energy < 0.5);

        // Verify that the disk cache was populated and second classify returns cached version
        let cached = classifier
            .classify("Calm Ambient Meditation", "Artist")
            .await;
        assert_eq!(cached.energy, mood.energy);
        assert_eq!(cached.tags, mood.tags);
    }

    #[tokio::test]
    async fn test_acoustic_fallback_branches() {
        let tmp = TempDir::new().unwrap();
        // Provider Lastfm with no API key -> falls back to Local
        let config = AcousticConfig {
            cache_dir: tmp.path().to_path_buf(),
            provider: AcousticProvider::Lastfm,
            lastfm_api_key: None,
            fallback: AcousticProvider::Local,
            ..Default::default()
        };
        let classifier = AcousticClassifier::new(config);
        let mood = classifier.classify("Heavy Battle Doom Metal", "Band").await;
        assert!(mood.energy > 0.6);
        assert_eq!(mood.provider, "local");

        // Provider Musicbrainz with fallback to Lastfm (no key) -> fallback to Local
        let config2 = AcousticConfig {
            cache_dir: tmp.path().to_path_buf(),
            provider: AcousticProvider::Musicbrainz,
            fallback: AcousticProvider::Lastfm,
            lastfm_api_key: None,
            ..Default::default()
        };
        let classifier2 = AcousticClassifier::new(config2);
        let mood2 = classifier2.classify("Lo-fi Calm Relax", "Artist").await;
        assert!(mood2.energy < 0.5);

        // Fallback to Musicbrainz
        let config3 = AcousticConfig {
            cache_dir: tmp.path().to_path_buf(),
            provider: AcousticProvider::Lastfm,
            lastfm_api_key: None,
            fallback: AcousticProvider::Musicbrainz,
            ..Default::default()
        };
        let classifier3 = AcousticClassifier::new(config3);
        let mood3 = classifier3.classify("Random Title", "Random Artist").await;
        assert_eq!(mood3.energy, 0.5);
    }
}
