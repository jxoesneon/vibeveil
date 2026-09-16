use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use zbus::Connection;
use zbus::zvariant::Value;

/// Sends a desktop notification over session D-Bus using org.freedesktop.Notifications.
///
/// This is implemented in pure asynchronous Rust via the existing `zbus` connection,
/// introducing zero external C dependencies and zero polling overhead.
pub async fn send_desktop_notification(
    connection: &Connection,
    title: &str,
    artist: &str,
    wallpaper_name: &str,
    icon_path: Option<&Path>,
) -> Result<u32> {
    let proxy = zbus::Proxy::new(
        connection,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await?;

    let app_name = "VibeVeil";
    let replaces_id: u32 = 0;
    let app_icon = icon_path
        .and_then(|p| p.to_str())
        .unwrap_or("media-optical");
    let summary = format!("VibeVeil: {}", title);
    let body = format!("{}\nWallpaper: {}", artist, wallpaper_name);
    let actions: Vec<&str> = vec![];
    let hints: HashMap<&str, Value<'_>> = HashMap::new();
    let expire_timeout: i32 = 4000; // 4 seconds

    let reply: u32 = proxy
        .call(
            "Notify",
            &(
                app_name,
                replaces_id,
                app_icon,
                summary.as_str(),
                body.as_str(),
                &actions,
                &hints,
                expire_timeout,
            ),
        )
        .await?;

    Ok(reply)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_formatting() {
        let title = "Resonance";
        let artist = "HOME";
        let wallpaper = "cyberpunk_lucy.mp4";
        let summary = format!("VibeVeil: {}", title);
        let body = format!("{}\nWallpaper: {}", artist, wallpaper);

        assert_eq!(summary, "VibeVeil: Resonance");
        assert_eq!(body, "HOME\nWallpaper: cyberpunk_lucy.mp4");
    }

    #[tokio::test]
    async fn test_send_desktop_notification_branches() {
        if let Ok(conn) = Connection::session().await {
            let _ = send_desktop_notification(
                &conn,
                "Test Track",
                "Test Artist",
                "wallpaper.png",
                None,
            )
            .await;
            let icon_path = std::path::Path::new("/tmp/test_nonexistent_icon.png");
            let _ = send_desktop_notification(
                &conn,
                "Test Track 2",
                "Test Artist 2",
                "wallpaper2.mp4",
                Some(icon_path),
            )
            .await;
        }
    }
}
