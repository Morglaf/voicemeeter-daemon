pub fn show_scene_toast(scene_name: &str) {
    show_toast("Voicemeeter Controller", &format!("Scène : {scene_name}"));
}

pub fn show_info_toast(message: &str) {
    show_toast("Voicemeeter Controller", message);
}

pub fn show_error_toast(message: &str) {
    show_toast("Voicemeeter Controller", message);
}

fn show_toast(title: &str, body: &str) {
    if try_winrt_toast(title.to_string(), body.to_string()).is_err() {
        eprintln!("{title}: {body}");
    }
}

fn try_winrt_toast(title: String, body: String) -> Result<(), ()> {
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

    let xml_string = format!(
        r#"<toast duration="short"><visual><binding template="ToastText02"><text id="1">{}</text><text id="2">{}</text></binding></visual></toast>"#,
        xml_escape(&title),
        xml_escape(&body)
    );

    let xml = XmlDocument::new().map_err(|_| ())?;
    xml.LoadXml(&windows::core::HSTRING::from(&xml_string))
        .map_err(|_| ())?;

    let toast = ToastNotification::CreateToastNotification(&xml).map_err(|_| ())?;
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(
        &windows::core::HSTRING::from("VoicemeeterController"),
    )
    .map_err(|_| ())?;

    notifier.Show(&toast).map_err(|_| ())?;
    Ok(())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
