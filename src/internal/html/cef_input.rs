//! Browser input is serialized on the app side; native CEF calls stay in its host.
use super::HtmlInputEvent;
use keyboard_types::{Key, KeyState, Location, Modifiers, NamedKey};
use serde_json::{json, Value};

fn modifiers(mods: Modifiers) -> u32 {
    let mut flags = 0;
    for (key, flag) in [(Modifiers::SHIFT, 2), (Modifiers::CONTROL, 4),
        (Modifiers::ALT, 8), (Modifiers::META, 128), (Modifiers::CAPS_LOCK, 1),
        (Modifiers::NUM_LOCK, 256), (Modifiers::ALT_GRAPH, 4096)] {
        if mods.contains(key) { flags |= flag; }
    }
    flags
}

pub(super) fn command(id: u64, generation: u64, event: HtmlInputEvent) -> Value {
    let mut data = match event {
        HtmlInputEvent::MouseMove { x, y } => json!({"type":"move", "x":x, "y":y}),
        HtmlInputEvent::MouseButton { x, y, button, pressed } =>
            json!({"type":"button", "x":x, "y":y, "button":button, "pressed":pressed}),
        HtmlInputEvent::Wheel { x, y, dx, dy } | HtmlInputEvent::Scroll { x, y, dx, dy } =>
            json!({"type":"wheel", "x":x, "y":y, "dx":dx, "dy":dy}),
        HtmlInputEvent::Focus(focused) => json!({"type":"focus", "focused":focused}),
        HtmlInputEvent::Modifiers(mods) => json!({"type":"modifiers", "modifiers":modifiers(mods)}),
        HtmlInputEvent::Key(key) => {
            let mut flags = modifiers(key.modifiers);
            flags |= match key.location { Location::Numpad => 512, Location::Left => 1024,
                Location::Right => 2048, Location::Standard => 0 };
            if key.repeat { flags |= 8192; }
            let text = match &key.key {
                Key::Character(text) => text.as_str(),
                Key::Named(NamedKey::Enter) => "\r",
                Key::Named(_) => "",
            };
            json!({"type":"key", "code":format!("{:?}", key.code),
                "key":key.key.to_string(), "text":text, "pressed":key.state == KeyState::Down,
                "modifiers":flags})
        }
        HtmlInputEvent::Ime(composition) => json!({"type":"ime", "text":composition.data,
            "commit":composition.state == keyboard_types::CompositionState::End}),
        HtmlInputEvent::ImeDismissed => json!({"type":"ime_cancel"}),
    };
    data["op"] = json!("input");
    data["id"] = json!(id);
    data["generation"] = json!(generation);
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_is_addressed_to_the_same_page_and_generation() {
        let click = command(7, 3, HtmlInputEvent::MouseButton { x:40.0, y:50.0, button:0, pressed:false });
        assert_eq!(click["id"], 7);
        assert_eq!(click["generation"], 3);
        assert_eq!(click["type"], "button");
        assert_eq!(click["pressed"], false);
        let flags = command(7, 3, HtmlInputEvent::Modifiers(Modifiers::CONTROL | Modifiers::SHIFT));
        assert_eq!(flags["modifiers"], 6);
    }

    #[test]
    fn unicode_and_key_up_survive_the_transport() {
        let mut key = keyboard_types::KeyboardEvent::default();
        key.key = Key::Character("ą".into());
        key.code = keyboard_types::Code::KeyA;
        key.state = KeyState::Up;
        let event = command(2, 0, HtmlInputEvent::Key(key));
        assert_eq!(event["text"], "ą");
        assert_eq!(event["code"], "KeyA");
        assert_eq!(event["pressed"], false);
    }
}
