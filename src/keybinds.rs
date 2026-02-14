use crate::config::AppConfig;
use winit::keyboard::{Key, ModifiersState, NamedKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingMode {
    Global,
    View,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    Quit,

    // Navigation (Global or View)
    NextImage,
    PrevImage,
    NextMark,
    PrevMark,
    FirstImage,
    LastImage,
    NextFrame,
    PrevFrame,

    // View Mode Specific
    PanLeft,
    PanRight,
    PanUp,
    PanDown,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    FitToWindow,
    BestFit,
    Cover,
    FitWidth,
    FitHeight,
    ResetView,
    RotateCW,
    RotateCCW,
    FlipHorizontal,
    FlipVertical,

    // Grid Mode Specific
    GridMoveLeft,
    GridMoveRight,
    GridMoveUp,
    GridMoveDown,
    GridMovePageUp,
    GridMovePageDown,

    // Global Toggles / Actions
    ToggleGrid,
    ToggleStatusBar,
    ToggleAnimation,
    ToggleSlideshow,
    ToggleMarks,
    UnmarkAll,
    MarkFile,
    RemoveImage,
    ScriptHandlerPrefix,
    FilterMode,
    ToggleAlpha,
    Digit(usize),
}

pub struct Binding {
    pub key: Key,
    pub mods: ModifiersState,
    pub mode: BindingMode,
    pub action: Action,
}

impl Binding {
    pub fn resolve(
        event: &winit::event::KeyEvent,
        bindings: &[Binding],
        current_mods: ModifiersState,
        is_grid: bool,
    ) -> Option<Action> {
        let current_mode = if is_grid {
            BindingMode::Grid
        } else {
            BindingMode::View
        };

        let result = bindings
            .iter()
            .find(|b| {
                let key_matches = b.key == event.logical_key;
                let mods_match = modifiers_match(current_mods, b.mods, &b.key);
                key_matches
                    && mods_match
                    && (b.mode == current_mode || b.mode == BindingMode::Global)
            })
            .map(|b| b.action);

        if result.is_some() {
            return result;
        }

        let has_functional_mods =
            current_mods.control_key() || current_mods.alt_key() || current_mods.super_key();

        if !has_functional_mods {
            if let winit::keyboard::Key::Character(c) = &event.logical_key {
                if let Ok(digit) = c.parse::<usize>() {
                    return Some(Action::Digit(digit));
                }
            }
        }
        None
    }

    pub fn get_all_bindings() -> Vec<Binding> {
        let config = AppConfig::get();
        let mut bindings = Vec::new();
        let add =
            |target: &mut Vec<Binding>, keys: &[String], mode: BindingMode, action: Action| {
                for key_str in keys {
                    if let Some((key, mods)) = parse_keybinding(key_str) {
                        target.push(Binding {
                            key,
                            mods,
                            mode,
                            action,
                        });
                    }
                }
            };

        let k = &config.keybindings;

        add(&mut bindings, &k.quit.0, BindingMode::Global, Action::Quit);
        add(
            &mut bindings,
            &k.handler_prefix.0,
            BindingMode::Global,
            Action::ScriptHandlerPrefix,
        );
        add(
            &mut bindings,
            &k.toggle_status_bar.0,
            BindingMode::Global,
            Action::ToggleStatusBar,
        );
        add(
            &mut bindings,
            &k.toggle_animation.0,
            BindingMode::Global,
            Action::ToggleAnimation,
        );
        add(
            &mut bindings,
            &k.toggle_slideshow.0,
            BindingMode::Global,
            Action::ToggleSlideshow,
        );
        add(
            &mut bindings,
            &k.image_next.0,
            BindingMode::Global,
            Action::NextImage,
        );
        add(
            &mut bindings,
            &k.image_previous.0,
            BindingMode::Global,
            Action::PrevImage,
        );
        add(
            &mut bindings,
            &k.next_mark.0,
            BindingMode::Global,
            Action::NextMark,
        );
        add(
            &mut bindings,
            &k.prev_mark.0,
            BindingMode::Global,
            Action::PrevMark,
        );

        add(
            &mut bindings,
            &k.toggle_grid.0,
            BindingMode::Global,
            Action::ToggleGrid,
        );

        add(
            &mut bindings,
            &k.filter_mode.0,
            BindingMode::Global,
            Action::FilterMode,
        );

        add(
            &mut bindings,
            &k.mark_file.0,
            BindingMode::Global,
            Action::MarkFile,
        );
        add(
            &mut bindings,
            &k.unmark_all.0,
            BindingMode::Global,
            Action::UnmarkAll,
        );
        add(
            &mut bindings,
            &k.remove_image.0,
            BindingMode::Global,
            Action::RemoveImage,
        );
        add(
            &mut bindings,
            &k.mark_all.0,
            BindingMode::Global,
            Action::ToggleMarks,
        );
        add(
            &mut bindings,
            &k.first_image.0,
            BindingMode::Global,
            Action::FirstImage,
        );
        add(
            &mut bindings,
            &k.last_image.0,
            BindingMode::Global,
            Action::LastImage,
        );

        // View Mode
        add(
            &mut bindings,
            &k.zoom_in.0,
            BindingMode::View,
            Action::ZoomIn,
        );
        add(
            &mut bindings,
            &k.zoom_out.0,
            BindingMode::View,
            Action::ZoomOut,
        );
        add(
            &mut bindings,
            &k.zoom_reset.0,
            BindingMode::View,
            Action::ZoomReset,
        );
        add(
            &mut bindings,
            &k.fit_best.0,
            BindingMode::View,
            Action::FitToWindow,
        ); // 'f'
        add(
            &mut bindings,
            &k.fit_best_no_upscale.0,
            BindingMode::View,
            Action::BestFit,
        ); // 'F'
        add(
            &mut bindings,
            &k.fit_cover.0,
            BindingMode::View,
            Action::Cover,
        ); //C
        add(
            &mut bindings,
            &k.fit_width.0,
            BindingMode::View,
            Action::FitWidth,
        );
        add(
            &mut bindings,
            &k.fit_height.0,
            BindingMode::View,
            Action::FitHeight,
        );
        add(
            &mut bindings,
            &k.view_reset_pan.0,
            BindingMode::View,
            Action::ResetView,
        );
        add(
            &mut bindings,
            &k.image_flip_horizontal.0,
            BindingMode::View,
            Action::FlipHorizontal,
        );
        add(
            &mut bindings,
            &k.image_flip_vertical.0,
            BindingMode::View,
            Action::FlipVertical,
        );
        add(
            &mut bindings,
            &k.rotate_cw.0,
            BindingMode::View,
            Action::RotateCW,
        );
        add(
            &mut bindings,
            &k.rotate_ccw.0,
            BindingMode::View,
            Action::RotateCCW,
        );

        // Pan Keys - Dual Mode
        // View Mode: Pan
        add(
            &mut bindings,
            &k.view_pan_left.0,
            BindingMode::View,
            Action::PanLeft,
        );
        add(
            &mut bindings,
            &k.view_pan_right.0,
            BindingMode::View,
            Action::PanRight,
        );
        add(
            &mut bindings,
            &k.view_pan_up.0,
            BindingMode::View,
            Action::PanUp,
        );
        add(
            &mut bindings,
            &k.view_pan_down.0,
            BindingMode::View,
            Action::PanDown,
        );

        // Grid Mode: Move
        add(
            &mut bindings,
            &k.view_pan_left.0,
            BindingMode::Grid,
            Action::GridMoveLeft,
        );
        add(
            &mut bindings,
            &k.view_pan_right.0,
            BindingMode::Grid,
            Action::GridMoveRight,
        );
        add(
            &mut bindings,
            &k.view_pan_up.0,
            BindingMode::Grid,
            Action::GridMoveUp,
        );
        add(
            &mut bindings,
            &k.view_pan_down.0,
            BindingMode::Grid,
            Action::GridMoveDown,
        );

        add(
            &mut bindings,
            &k.grid_page_up.0,
            BindingMode::Grid,
            Action::GridMovePageUp,
        );
        add(
            &mut bindings,
            &k.grid_page_down.0,
            BindingMode::Grid,
            Action::GridMovePageDown,
        );
        add(
            &mut bindings,
            &k.toggle_alpha.0,
            BindingMode::Global,
            Action::ToggleAlpha,
        );
        add(
            &mut bindings,
            &k.next_frame.0,
            BindingMode::View,
            Action::NextFrame,
        );
        add(
            &mut bindings,
            &k.prev_frame.0,
            BindingMode::View,
            Action::PrevFrame,
        );

        bindings
    }
}

fn modifiers_match(current: ModifiersState, required: ModifiersState, key: &Key) -> bool {
    // We want to ensure that 'required' bits are set in 'current'.
    // And that no *other* primary modifiers (Ctrl, Alt, Shift, Super) are set if not required.
    // This prevents "Ctrl+a" from triggering "a".

    // For Key::Character, winit's logical_key usually already accounts for Shift.
    // E.g. Shift + 'g' -> "G".
    // If we enforce exact modifier match, Shift+"g" vs Binding("G", NoMods) will fail.
    // So for Character keys, we ignore the Shift modifier state in the comparison.
    let ignore_shift = matches!(key, Key::Character(_));

    let shift = ignore_shift || (current.shift_key() == required.shift_key());
    let ctrl = current.control_key() == required.control_key();
    let alt = current.alt_key() == required.alt_key();
    let super_key = current.super_key() == required.super_key();

    shift && ctrl && alt && super_key
}

fn parse_keybinding(s: &str) -> Option<(Key, ModifiersState)> {
    let (mods_part, key_part) = if s == "+" {
        ("", "+")
    } else if s.ends_with("++") {
        (&s[..s.len() - 1], "+")
    } else {
        match s.rsplit_once('+') {
            Some((m, k)) => (m, k),
            None => ("", s),
        }
    };

    let mut mods = ModifiersState::default();

    if !mods_part.is_empty() {
        for mod_str in mods_part.split('+') {
            match mod_str.to_lowercase().as_str() {
                "ctrl" | "control" => mods |= ModifiersState::CONTROL,
                "shift" => mods |= ModifiersState::SHIFT,
                "alt" => mods |= ModifiersState::ALT,
                "super" | "meta" => mods |= ModifiersState::SUPER,
                _ => {}
            }
        }
    }

    // Parse Key
    let key = match key_part {
        "Left" => Key::Named(NamedKey::ArrowLeft),
        "Right" => Key::Named(NamedKey::ArrowRight),
        "Up" => Key::Named(NamedKey::ArrowUp),
        "Down" => Key::Named(NamedKey::ArrowDown),
        "Enter" | "Return" => Key::Named(NamedKey::Enter),
        "Space" => Key::Named(NamedKey::Space),
        "Backspace" => Key::Named(NamedKey::Backspace),
        "Tab" => Key::Named(NamedKey::Tab),
        "Escape" | "Esc" => Key::Named(NamedKey::Escape),
        "Home" => Key::Named(NamedKey::Home),
        "End" => Key::Named(NamedKey::End),
        "PageUp" => Key::Named(NamedKey::PageUp),
        "PageDown" => Key::Named(NamedKey::PageDown),
        c if c.chars().count() == 1 => Key::Character(c.into()),
        _ => return None, // Unknown key
    };

    Some((key, mods))
}
