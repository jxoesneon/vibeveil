use anyhow::Result;
use std::collections::HashMap;
use std::ops::Deref;
use zbus::Connection;
use zbus::zvariant::Value;

#[derive(Debug, Clone)]
pub struct MprisTrack {
    pub player: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art_url: Option<String>,
    pub status: PlaybackStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

pub struct MprisClient {
    connection: Connection,
}

impl MprisClient {
    pub async fn new() -> Result<Self> {
        let connection = Connection::session().await?;
        Ok(Self { connection })
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub async fn find_active_player(&self) -> Result<Option<String>> {
        let dbus_proxy = zbus::fdo::DBusProxy::new(&self.connection).await?;
        let names = dbus_proxy.list_names().await?;

        // Prioritize Spotify, then any other MPRIS player
        let mut best = None;
        for name in names {
            let name_str = name.as_str();
            if name_str.starts_with("org.mpris.MediaPlayer2.") {
                if name_str.contains("spotify") {
                    return Ok(Some(name_str.to_string()));
                }
                if best.is_none() {
                    best = Some(name_str.to_string());
                }
            }
        }

        Ok(best)
    }

    pub async fn get_current_track(&self, player: &str) -> Result<Option<MprisTrack>> {
        let proxy = zbus::Proxy::new(
            &self.connection,
            player,
            "/org/mpris/MediaPlayer2",
            "org.mpris.MediaPlayer2.Player",
        )
        .await?;

        let status_str: String = proxy
            .get_property("PlaybackStatus")
            .await
            .unwrap_or_else(|_| "Stopped".into());
        let status = parse_playback_status(&status_str);

        let metadata_val: Result<HashMap<String, zbus::zvariant::OwnedValue>, _> =
            proxy.get_property("Metadata").await;
        let metadata = match metadata_val {
            Ok(m) => m,
            Err(_) => return Ok(None),
        };

        Ok(Some(parse_track_from_metadata(player, status, &metadata)))
    }

    pub async fn listen_for_properties_changed(
        &self,
        player: &str,
    ) -> Result<impl futures_util::stream::Stream<Item = zbus::fdo::PropertiesChanged>> {
        let proxy = zbus::fdo::PropertiesProxy::builder(&self.connection)
            .destination(player)?
            .path("/org/mpris/MediaPlayer2")?
            .build()
            .await?;

        proxy.receive_properties_changed().await.map_err(Into::into)
    }
}

pub fn parse_playback_status(status_str: &str) -> PlaybackStatus {
    match status_str {
        "Playing" => PlaybackStatus::Playing,
        "Paused" => PlaybackStatus::Paused,
        _ => PlaybackStatus::Stopped,
    }
}

pub fn parse_track_from_metadata(
    player: &str,
    status: PlaybackStatus,
    metadata: &HashMap<String, zbus::zvariant::OwnedValue>,
) -> MprisTrack {
    let title = metadata
        .get("xesam:title")
        .and_then(|v| match v.deref() {
            Value::Str(s) => Some(s.to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "Unknown Title".into());

    let artist = metadata
        .get("xesam:artist")
        .and_then(|v| match v.deref() {
            Value::Array(arr) => {
                let artists: Vec<String> = arr
                    .iter()
                    .filter_map(|item| match item {
                        Value::Str(s) => Some(s.to_string()),
                        Value::Value(inner) => match inner.deref() {
                            Value::Str(s) => Some(s.to_string()),
                            _ => None,
                        },
                        _ => None,
                    })
                    .collect();
                if artists.is_empty() {
                    None
                } else {
                    Some(artists.join(", "))
                }
            }
            Value::Str(s) => Some(s.to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "Unknown Artist".into());

    let album = metadata
        .get("xesam:album")
        .and_then(|v| match v.deref() {
            Value::Str(s) => Some(s.to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "Unknown Album".into());

    let art_url = metadata.get("mpris:artUrl").and_then(|v| match v.deref() {
        Value::Str(s) => Some(s.to_string()),
        _ => None,
    });

    MprisTrack {
        player: player.to_string(),
        title,
        artist,
        album,
        art_url,
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- PlaybackStatus ---

    #[test]
    fn playback_status_all_variants_distinct() {
        assert_ne!(PlaybackStatus::Playing, PlaybackStatus::Paused);
        assert_ne!(PlaybackStatus::Paused, PlaybackStatus::Stopped);
        assert_ne!(PlaybackStatus::Playing, PlaybackStatus::Stopped);
    }

    #[test]
    fn playback_status_is_copy() {
        let s = PlaybackStatus::Playing;
        let s2 = s; // copy
        assert_eq!(s, s2);
    }

    #[test]
    fn playback_status_is_eq() {
        assert_eq!(PlaybackStatus::Playing, PlaybackStatus::Playing);
        assert_eq!(PlaybackStatus::Paused, PlaybackStatus::Paused);
        assert_eq!(PlaybackStatus::Stopped, PlaybackStatus::Stopped);
    }

    #[test]
    fn playback_status_debug() {
        let s = format!("{:?}", PlaybackStatus::Playing);
        assert!(s.contains("Playing"));
        let s = format!("{:?}", PlaybackStatus::Paused);
        assert!(s.contains("Paused"));
        let s = format!("{:?}", PlaybackStatus::Stopped);
        assert!(s.contains("Stopped"));
    }

    // --- MprisTrack ---

    #[test]
    fn mpris_track_clone_preserves_fields() {
        let t = MprisTrack {
            player: "org.mpris.MediaPlayer2.spotify".to_string(),
            title: "Blinding Lights".to_string(),
            artist: "The Weeknd".to_string(),
            album: "After Hours".to_string(),
            art_url: Some("https://example.com/art.jpg".to_string()),
            status: PlaybackStatus::Playing,
        };
        let c = t.clone();
        assert_eq!(c.player, t.player);
        assert_eq!(c.title, t.title);
        assert_eq!(c.artist, t.artist);
        assert_eq!(c.album, t.album);
        assert_eq!(c.art_url, t.art_url);
        assert_eq!(c.status, t.status);
    }

    #[test]
    fn mpris_track_optional_art_url_none() {
        let t = MprisTrack {
            player: "org.mpris.MediaPlayer2.vlc".to_string(),
            title: "Track".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            art_url: None,
            status: PlaybackStatus::Stopped,
        };
        assert!(t.art_url.is_none());
        assert_eq!(t.status, PlaybackStatus::Stopped);
    }

    #[test]
    fn mpris_track_debug() {
        let t = MprisTrack {
            player: "player".to_string(),
            title: "title".to_string(),
            artist: "artist".to_string(),
            album: "album".to_string(),
            art_url: None,
            status: PlaybackStatus::Paused,
        };
        let dbg = format!("{:?}", t);
        assert!(dbg.contains("title"));
        assert!(dbg.contains("Paused"));
    }

    // --- Parsing helper tests ---

    #[test]
    fn parse_playback_status_cases() {
        assert_eq!(parse_playback_status("Playing"), PlaybackStatus::Playing);
        assert_eq!(parse_playback_status("Paused"), PlaybackStatus::Paused);
        assert_eq!(parse_playback_status("Stopped"), PlaybackStatus::Stopped);
        assert_eq!(
            parse_playback_status("UnknownValue"),
            PlaybackStatus::Stopped
        );
    }

    #[test]
    fn parse_track_from_metadata_full_array_artist() {
        let mut map = HashMap::new();
        map.insert(
            "xesam:title".into(),
            Value::from("Starboy").try_to_owned().unwrap(),
        );
        let artists: Vec<&str> = vec!["The Weeknd", "Daft Punk"];
        map.insert(
            "xesam:artist".into(),
            Value::from(artists).try_to_owned().unwrap(),
        );
        map.insert(
            "xesam:album".into(),
            Value::from("Starboy").try_to_owned().unwrap(),
        );
        map.insert(
            "mpris:artUrl".into(),
            Value::from("https://example.com/starboy.jpg")
                .try_to_owned()
                .unwrap(),
        );

        let track = parse_track_from_metadata("spotify", PlaybackStatus::Playing, &map);
        assert_eq!(track.player, "spotify");
        assert_eq!(track.title, "Starboy");
        assert_eq!(track.artist, "The Weeknd, Daft Punk");
        assert_eq!(track.album, "Starboy");
        assert_eq!(
            track.art_url.as_deref(),
            Some("https://example.com/starboy.jpg")
        );
        assert_eq!(track.status, PlaybackStatus::Playing);
    }

    #[test]
    fn parse_track_from_metadata_single_artist_str() {
        let mut map = HashMap::new();
        map.insert(
            "xesam:title".into(),
            Value::from("Levitating").try_to_owned().unwrap(),
        );
        map.insert(
            "xesam:artist".into(),
            Value::from("Dua Lipa").try_to_owned().unwrap(),
        );
        map.insert(
            "xesam:album".into(),
            Value::from("Future Nostalgia").try_to_owned().unwrap(),
        );

        let track = parse_track_from_metadata("vlc", PlaybackStatus::Paused, &map);
        assert_eq!(track.artist, "Dua Lipa");
        assert_eq!(track.title, "Levitating");
        assert_eq!(track.album, "Future Nostalgia");
        assert!(track.art_url.is_none());
        assert_eq!(track.status, PlaybackStatus::Paused);
    }

    #[test]
    fn parse_track_from_metadata_empty_or_missing() {
        let map = HashMap::new();
        let track = parse_track_from_metadata("player", PlaybackStatus::Stopped, &map);
        assert_eq!(track.title, "Unknown Title");
        assert_eq!(track.artist, "Unknown Artist");
        assert_eq!(track.album, "Unknown Album");
        assert!(track.art_url.is_none());
    }

    #[test]
    fn parse_track_from_metadata_empty_artist_array() {
        let mut map = HashMap::new();
        let empty_arr: Vec<Value> = vec![];
        map.insert(
            "xesam:artist".into(),
            Value::Array(empty_arr.into()).try_to_owned().unwrap(),
        );
        let track = parse_track_from_metadata("player", PlaybackStatus::Playing, &map);
        assert_eq!(track.artist, "Unknown Artist");
    }

    // --- D-Bus session integration tests ---

    #[tokio::test]
    async fn mpris_client_live_bus_methods() {
        if let Ok(client) = MprisClient::new().await
            && let Ok(player) = client.find_active_player().await
            && let Some(ref p) = player
        {
            let _ = client.get_current_track(p).await;
            let _ = client.listen_for_properties_changed(p).await;
        }
    }
}
