use crate::{api::TrackItem, ipc::IpcServer, AuthManager, Player, SHUTDOWN};
use dbus::{
    arg::messageitem::MessageItem,
    blocking::Connection,
    channel::MatchingReceiver,
    message::MatchRule,
    strings::{Interface, Member, Path},
};
use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

const PATH: &str = "/org/mpris/MediaPlayer2";
const ROOT: &str = "org.mpris.MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const PROPS: &str = "org.freedesktop.DBus.Properties";

const XML: &str = "<node><interface name=\"org.freedesktop.DBus.Introspectable\"><method name=\"Introspect\"><arg name=\"xml\" type=\"s\" direction=\"out\"/></method></interface><interface name=\"org.freedesktop.DBus.Properties\"><method name=\"Get\"><arg type=\"s\" direction=\"in\"/><arg type=\"s\" direction=\"in\"/><arg type=\"v\" direction=\"out\"/></method><method name=\"GetAll\"><arg type=\"s\" direction=\"in\"/><arg type=\"a{sv}\" direction=\"out\"/></method><method name=\"Set\"><arg type=\"s\" direction=\"in\"/><arg type=\"s\" direction=\"in\"/><arg type=\"v\" direction=\"in\"/></method><signal name=\"PropertiesChanged\"><arg type=\"s\"/><arg type=\"a{sv}\"/><arg type=\"as\"/></signal></interface><interface name=\"org.mpris.MediaPlayer2\"><method name=\"Quit\"/><property name=\"Identity\" type=\"s\" access=\"read\"/><property name=\"CanQuit\" type=\"b\" access=\"read\"/><property name=\"CanRaise\" type=\"b\" access=\"read\"/><property name=\"HasTrackList\" type=\"b\" access=\"read\"/><property name=\"SupportedUriSchemes\" type=\"as\" access=\"read\"/><property name=\"SupportedMimeTypes\" type=\"as\" access=\"read\"/></interface><interface name=\"org.mpris.MediaPlayer2.Player\"><method name=\"Play\"/><method name=\"Pause\"/><method name=\"PlayPause\"/><method name=\"Stop\"/><method name=\"Next\"/><method name=\"Previous\"/><method name=\"Seek\"><arg type=\"x\" direction=\"in\"/></method><method name=\"SetPosition\"><arg type=\"o\" direction=\"in\"/><arg type=\"x\" direction=\"in\"/></method><signal name=\"Seeked\"><arg type=\"x\"/></signal><property name=\"PlaybackStatus\" type=\"s\" access=\"read\"/><property name=\"Metadata\" type=\"a{sv}\" access=\"read\"/><property name=\"Position\" type=\"x\" access=\"read\"/><property name=\"TidalAudioQuality\" type=\"s\" access=\"read\"/><property name=\"TidalPreferredAudioQuality\" type=\"s\" access=\"read\"/><property name=\"CanControl\" type=\"b\" access=\"read\"/><property name=\"CanPlay\" type=\"b\" access=\"read\"/><property name=\"CanPause\" type=\"b\" access=\"read\"/><property name=\"CanSeek\" type=\"b\" access=\"read\"/><property name=\"CanGoNext\" type=\"b\" access=\"read\"/><property name=\"CanGoPrevious\" type=\"b\" access=\"read\"/></interface></node>";

fn array_strings(values: &[&str]) -> MessageItem {
    MessageItem::Array(
        dbus::arg::messageitem::MessageItemArray::new(
            values
                .iter()
                .map(|s| MessageItem::Str((*s).into()))
                .collect(),
            "as".into(),
        )
        .unwrap(),
    )
}
fn variant(value: MessageItem) -> MessageItem {
    MessageItem::Variant(Box::new(value))
}
fn map(values: Vec<(&str, MessageItem)>) -> MessageItem {
    MessageItem::from_dict(
        values
            .into_iter()
            .map(|(k, v)| Ok::<_, ()>((k.to_string(), v)))
            .map(|r| r)
            .collect::<Vec<_>>()
            .into_iter(),
    )
    .unwrap()
}
fn track_metadata(track: Option<&TrackItem>) -> MessageItem {
    let Some(t) = track else { return map(vec![]) };
    let path = format!("{PATH}/Track/{}", t.id);
    map(vec![
        ("mpris:trackid", MessageItem::ObjectPath(path.into())),
        ("xesam:title", MessageItem::Str(t.title.clone())),
        ("xesam:artist", array_strings(&[t.artist_name()])),
        ("xesam:album", MessageItem::Str(t.album_title().to_string())),
        (
            "xesam:artUrl",
            MessageItem::Str(crate::api::cover_url(t.cover())),
        ),
        (
            "mpris:length",
            MessageItem::Int64((t.duration as i64).saturating_mul(1_000_000)),
        ),
    ])
}
fn properties(iface: &str, player: &Player) -> Vec<(&'static str, MessageItem)> {
    if iface == ROOT {
        return vec![
            ("Identity", MessageItem::Str("Tidal".into())),
            ("CanQuit", MessageItem::Bool(true)),
            ("CanRaise", MessageItem::Bool(false)),
            ("HasTrackList", MessageItem::Bool(false)),
            ("SupportedUriSchemes", array_strings(&["tidal"])),
            (
                "SupportedMimeTypes",
                array_strings(&["audio/flac", "audio/aac"]),
            ),
        ];
    }
    let idle = player
        .engine
        .property("idle-active")
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let paused = player
        .engine
        .property("pause")
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let status = if idle || player.current.is_none() {
        "Stopped"
    } else if paused {
        "Paused"
    } else {
        "Playing"
    };
    let pos = player.engine.position().unwrap_or(0.0);
    vec![
        ("PlaybackStatus", MessageItem::Str(status.into())),
        ("Metadata", track_metadata(player.current.as_ref())),
        ("Position", MessageItem::Int64((pos * 1_000_000.0) as i64)),
        (
            "TidalAudioQuality",
            MessageItem::Str(player.quality.clone()),
        ),
        (
            "TidalPreferredAudioQuality",
            MessageItem::Str(player.preferred_quality.clone()),
        ),
        ("CanControl", MessageItem::Bool(true)),
        ("CanPlay", MessageItem::Bool(true)),
        ("CanPause", MessageItem::Bool(true)),
        ("CanSeek", MessageItem::Bool(true)),
        ("CanGoNext", MessageItem::Bool(true)),
        ("CanGoPrevious", MessageItem::Bool(true)),
    ]
}
fn dict(props: Vec<(&str, MessageItem)>) -> MessageItem {
    map(props)
}
fn parse_seek_args(
    member: &str,
    args: &[MessageItem],
) -> Result<(Option<String>, i64), &'static str> {
    let (track, position) = if member == "Seek" {
        (None, args.first())
    } else {
        let track = args.first().and_then(|v| {
            if let MessageItem::ObjectPath(path) = v {
                Some(path.to_string())
            } else {
                None
            }
        });
        (track, args.get(1))
    };
    let position = match position {
        Some(MessageItem::Int64(value)) => *value,
        _ => return Err("Expected an int64 position"),
    };
    if member == "SetPosition" && track.is_none() {
        return Err("Expected an object path track ID");
    }
    Ok((track, position))
}
fn reply(conn: &Connection, msg: &dbus::Message, items: Vec<MessageItem>) {
    if let Some(mut out) = dbus::Message::new_method_return(msg) {
        out.append_items(&items);
        let _ = conn.channel().send(out);
    }
}
fn error(conn: &Connection, msg: &dbus::Message, text: &str) {
    let name: dbus::strings::ErrorName = "org.freedesktop.DBus.Error.InvalidArgs".into();
    let c = std::ffi::CString::new(text).unwrap_or_default();
    let _ = conn.channel().send(msg.error(&name, &c));
}
fn send_changed(conn: &Connection, iface: &str, changed: Vec<(&str, MessageItem)>) {
    if let Some(mut sig) = dbus::Message::signal(
        &Path::new(PATH).unwrap(),
        &Interface::new(PROPS).unwrap(),
        &Member::new("PropertiesChanged").unwrap(),
    )
    .into()
    {
        sig.append_items(&[
            MessageItem::Str(iface.into()),
            map(changed),
            array_strings(&[]),
        ]);
        let _ = conn.channel().send(sig);
    }
}

pub fn start(auth: Arc<AuthManager>, player: Arc<Mutex<Player>>, ipc: IpcServer) {
    thread::spawn(move || {
        let conn = match Connection::new_session() {
            Ok(c) => c,
            Err(e) => {
                crate::log::write(&format!("MPRIS session bus unavailable: {e}"));
                return;
            }
        };
        if let Err(e) = conn.request_name("org.mpris.MediaPlayer2.Tidal", false, true, true) {
            crate::log::write(&format!("MPRIS registration failed: {e}"));
            return;
        }
        let auth_cb = Arc::clone(&auth);
        let player_cb = Arc::clone(&player);
        let ipc_cb = ipc.clone();
        conn.start_receive(
            MatchRule::new_method_call(),
            Box::new(move |msg, conn| {
                if msg.path().map(|p| p.to_string()) != Some(PATH.into()) {
                    return true;
                }
                let iface = msg.interface().map(|i| i.to_string()).unwrap_or_default();
                let member = msg.member().map(|m| m.to_string()).unwrap_or_default();
                let args = msg.get_items();
                if iface == "org.freedesktop.DBus.Introspectable" && member == "Introspect" {
                    reply(conn, &msg, vec![MessageItem::Str(XML.into())]);
                    return true;
                }
                if iface == PROPS {
                    let target = args
                        .first()
                        .and_then(|x| {
                            if let MessageItem::Str(s) = x {
                                Some(s.as_str())
                            } else {
                                None
                            }
                        })
                        .unwrap_or("");
                    if member == "Get" {
                        let name = args
                            .get(1)
                            .and_then(|x| {
                                if let MessageItem::Str(s) = x {
                                    Some(s.as_str())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or("");
                        let value = player_cb.lock().ok().and_then(|p| {
                            properties(target, &p)
                                .into_iter()
                                .find(|(n, _)| *n == name)
                                .map(|(_, v)| v)
                        });
                        if let Some(v) = value {
                            reply(conn, &msg, vec![variant(v)]);
                        } else {
                            error(conn, &msg, "Unknown property");
                        }
                    } else if member == "GetAll" {
                        if let Ok(p) = player_cb.lock() {
                            reply(conn, &msg, vec![dict(properties(target, &p))]);
                        }
                    } else {
                        error(conn, &msg, "Properties are read-only");
                    }
                    return true;
                }
                let mut result: Result<(), String> = Ok(());
                match (iface.as_str(), member.as_str()) {
                    (ROOT, "Quit") => {
                        SHUTDOWN.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    (PLAYER, "Play") | (PLAYER, "PlayPause") => {
                        result = crate::play_or_resume(
                            &auth_cb,
                            &player_cb,
                            &ipc_cb,
                            member == "PlayPause",
                        );
                    }
                    (PLAYER, "Pause") | (PLAYER, "Stop") => {
                        result = player_cb
                            .lock()
                            .map_err(|e| e.to_string())
                            .and_then(|mut p| match member.as_str() {
                                "Pause" => p.engine.set_pause(true),
                                "PlayPause" => p.engine.toggle_pause(),
                                _ => p.engine.stop(),
                            });
                        if member == "Stop" {
                            if let Ok(mut p) = player_cb.lock() {
                                p.current = None;
                            }
                        }
                    }
                    (PLAYER, "Next") | (PLAYER, "Previous") => {
                        result = crate::navigate(&auth_cb, &player_cb, &ipc_cb, member == "Next")
                    }
                    (PLAYER, "Seek") | (PLAYER, "SetPosition") => {
                        let (track_id, micros) = match parse_seek_args(&member, &args) {
                            Ok(parsed) => parsed,
                            Err(e) => {
                                error(conn, &msg, e);
                                return true;
                            }
                        };
                        if member == "SetPosition" {
                            let expected = player_cb
                                .lock()
                                .ok()
                                .and_then(|p| p.current.as_ref().map(|t| t.id));
                            let actual = track_id;
                            if expected.map(|id| format!("{PATH}/Track/{id}")) != actual {
                                error(conn, &msg, "Track ID does not match current track");
                                return true;
                            }
                        }
                        let current = player_cb
                            .lock()
                            .ok()
                            .and_then(|p| p.engine.position().ok())
                            .unwrap_or(0.0);
                        let seconds = if member == "Seek" {
                            (current + micros as f64 / 1_000_000.0).max(0.0)
                        } else {
                            micros.max(0) as f64 / 1_000_000.0
                        };
                        result = player_cb
                            .lock()
                            .map_err(|e| e.to_string())
                            .and_then(|p| p.engine.seek(seconds));
                        if result.is_ok() {
                            let sig = dbus::Message::signal(
                                &Path::new(PATH).unwrap(),
                                &Interface::new(PLAYER).unwrap(),
                                &Member::new("Seeked").unwrap(),
                            )
                            .append1((seconds * 1_000_000.0) as i64);
                            let _ = conn.channel().send(sig);
                        }
                    }
                    _ => {
                        error(conn, &msg, "Unknown method");
                        return true;
                    }
                }
                if let Err(e) = result {
                    error(conn, &msg, &e);
                } else {
                    reply(conn, &msg, vec![]);
                }
                true
            }),
        );
        let mut previous = String::new();
        while !SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = conn.process(Duration::from_millis(100));
            if let Ok(p) = player.lock() {
                let props = properties(PLAYER, &p);
                let status = props
                    .iter()
                    .find(|(n, _)| *n == "PlaybackStatus")
                    .map(|(_, v)| format!("{v:?}"))
                    .unwrap_or_default();
                let track = p.current.as_ref().map(|t| t.id).unwrap_or(0);
                let state = format!("{status}:{track}:{}:{}", p.quality, p.preferred_quality);
                if state != previous {
                    send_changed(
                        &conn,
                        PLAYER,
                        vec![
                            ("PlaybackStatus", props[0].1.clone()),
                            ("Metadata", props[1].1.clone()),
                            ("TidalAudioQuality", props[3].1.clone()),
                            ("TidalPreferredAudioQuality", props[4].1.clone()),
                        ],
                    );
                    previous = state;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mpris_schema_has_required_methods_and_properties() {
        assert!(XML.contains("name=\"SetPosition\""));
        assert!(XML.contains("name=\"PropertiesChanged\""));
        assert!(XML.contains("name=\"PlaybackStatus\""));
    }
    #[test]
    fn metadata_is_variant_dictionary() {
        let t: TrackItem = serde_json::from_str(r#"{"id":42,"title":"Song","duration":180,"artists":[{"name":"Artist"}],"album":{"title":"Album","cover":"ab-cd"}}"#).unwrap();
        let x = track_metadata(Some(&t));
        assert_eq!(x.signature().to_string(), "a{sv}");
        let MessageItem::Dict(entries) = x else {
            panic!("metadata must be a dictionary")
        };
        assert_eq!(entries.len(), 6);
    }
    #[test]
    fn seek_message_arguments_parse_with_expected_dbus_types() {
        assert_eq!(
            parse_seek_args("Seek", &[MessageItem::Int64(-5)]),
            Ok((None, -5))
        );
        assert_eq!(
            parse_seek_args(
                "SetPosition",
                &[
                    MessageItem::ObjectPath(format!("{PATH}/Track/42").into()),
                    MessageItem::Int64(15)
                ]
            ),
            Ok((Some(format!("{PATH}/Track/42")), 15))
        );
        assert!(parse_seek_args("Seek", &[MessageItem::Str("15".into())]).is_err());
        assert!(parse_seek_args(
            "SetPosition",
            &[MessageItem::Str("bad path".into()), MessageItem::Int64(15)]
        )
        .is_err());
    }
    #[test]
    fn root_property_values_match_mpris() {
        let props = properties(ROOT, &Player::new());
        assert_eq!(
            props.iter().find(|(n, _)| *n == "Identity").unwrap().1,
            MessageItem::Str("Tidal".into())
        );
        assert_eq!(
            props.iter().find(|(n, _)| *n == "CanQuit").unwrap().1,
            MessageItem::Bool(true)
        );
        assert_eq!(
            props.iter().find(|(n, _)| *n == "CanRaise").unwrap().1,
            MessageItem::Bool(false)
        );
        assert_eq!(dict(props).signature().to_string(), "a{sv}");
    }
}
