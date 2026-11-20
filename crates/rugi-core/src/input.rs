//! Platform-independent input events.
//!
//! Platform layers (winit, the C ABI, test harnesses) translate their native
//! events into [`InputEvent`]s and queue them on the UI context before the
//! frame begins. All positions are logical pixels.

/// A pointer (mouse) button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerButton {
    /// Left / primary button.
    Primary = 0,
    /// Right / secondary button.
    Secondary = 1,
    /// Middle button.
    Middle = 2,
}

/// Keys the UI reacts to. Text entry arrives separately via [`InputEvent::Text`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// Tab.
    Tab,
    /// Enter / Return.
    Enter,
    /// Escape.
    Escape,
    /// Space bar.
    Space,
    /// Backspace.
    Backspace,
    /// Forward delete.
    Delete,
    /// Left arrow.
    ArrowLeft,
    /// Right arrow.
    ArrowRight,
    /// Up arrow.
    ArrowUp,
    /// Down arrow.
    ArrowDown,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// Letter `A`.
    A,
    /// Letter `C`.
    C,
    /// Letter `V`.
    V,
    /// Letter `X`.
    X,
    /// Letter `Y`.
    Y,
    /// Letter `Z`.
    Z,
    /// Letter `I`.
    I,
    /// Letter `K`.
    K,
    /// Function key F12.
    F12,
    /// Letter `B`.
    B,
    /// Letter `D`.
    D,
    /// Letter `E`.
    E,
    /// Letter `F`.
    F,
    /// Letter `G`.
    G,
    /// Letter `H`.
    H,
    /// Letter `J`.
    J,
    /// Letter `L`.
    L,
    /// Letter `M`.
    M,
    /// Letter `N`.
    N,
    /// Letter `O`.
    O,
    /// Letter `P`.
    P,
    /// Letter `Q`.
    Q,
    /// Letter `R`.
    R,
    /// Letter `S`.
    S,
    /// Letter `T`.
    T,
    /// Letter `U`.
    U,
    /// Letter `W`.
    W,
    /// Digit `0` on the main row.
    Num0,
    /// Digit `1` on the main row.
    Num1,
    /// Digit `2` on the main row.
    Num2,
    /// Digit `3` on the main row.
    Num3,
    /// Digit `4` on the main row.
    Num4,
    /// Digit `5` on the main row.
    Num5,
    /// Digit `6` on the main row.
    Num6,
    /// Digit `7` on the main row.
    Num7,
    /// Digit `8` on the main row.
    Num8,
    /// Digit `9` on the main row.
    Num9,
    /// Function key F1.
    F1,
    /// Function key F2.
    F2,
    /// Function key F3.
    F3,
    /// Function key F4.
    F4,
    /// Function key F5.
    F5,
    /// Function key F6.
    F6,
    /// Function key F7.
    F7,
    /// Function key F8.
    F8,
    /// Function key F9.
    F9,
    /// Function key F10.
    F10,
    /// Function key F11.
    F11,
    /// Insert.
    Insert,
    /// Either Shift key.
    Shift,
    /// Either Control key.
    Control,
    /// Either Alt / Option key.
    Alt,
    /// Caps Lock.
    CapsLock,
    /// Context-menu (application) key.
    ContextMenu,
}

impl Key {
    /// Every key, in declaration order.
    pub const ALL: [Key; 68] = [
        Key::Tab,
        Key::Enter,
        Key::Escape,
        Key::Space,
        Key::Backspace,
        Key::Delete,
        Key::ArrowLeft,
        Key::ArrowRight,
        Key::ArrowUp,
        Key::ArrowDown,
        Key::Home,
        Key::End,
        Key::PageUp,
        Key::PageDown,
        Key::A,
        Key::C,
        Key::V,
        Key::X,
        Key::Y,
        Key::Z,
        Key::I,
        Key::K,
        Key::F12,
        Key::B,
        Key::D,
        Key::E,
        Key::F,
        Key::G,
        Key::H,
        Key::J,
        Key::L,
        Key::M,
        Key::N,
        Key::O,
        Key::P,
        Key::Q,
        Key::R,
        Key::S,
        Key::T,
        Key::U,
        Key::W,
        Key::Num0,
        Key::Num1,
        Key::Num2,
        Key::Num3,
        Key::Num4,
        Key::Num5,
        Key::Num6,
        Key::Num7,
        Key::Num8,
        Key::Num9,
        Key::F1,
        Key::F2,
        Key::F3,
        Key::F4,
        Key::F5,
        Key::F6,
        Key::F7,
        Key::F8,
        Key::F9,
        Key::F10,
        Key::F11,
        Key::Insert,
        Key::Shift,
        Key::Control,
        Key::Alt,
        Key::CapsLock,
        Key::ContextMenu,
    ];

    /// Short display name ("A", "F9", "Space", "←").
    pub fn name(self) -> &'static str {
        use Key::*;
        match self {
            Tab => "Tab",
            Enter => "Enter",
            Escape => "Esc",
            Space => "Space",
            Backspace => "Backspace",
            Delete => "Delete",
            ArrowLeft => "Left",
            ArrowRight => "Right",
            ArrowUp => "Up",
            ArrowDown => "Down",
            Home => "Home",
            End => "End",
            PageUp => "PgUp",
            PageDown => "PgDn",
            A => "A",
            B => "B",
            C => "C",
            D => "D",
            E => "E",
            F => "F",
            G => "G",
            H => "H",
            I => "I",
            J => "J",
            K => "K",
            L => "L",
            M => "M",
            N => "N",
            O => "O",
            P => "P",
            Q => "Q",
            R => "R",
            S => "S",
            T => "T",
            U => "U",
            V => "V",
            W => "W",
            X => "X",
            Y => "Y",
            Z => "Z",
            Num0 => "0",
            Num1 => "1",
            Num2 => "2",
            Num3 => "3",
            Num4 => "4",
            Num5 => "5",
            Num6 => "6",
            Num7 => "7",
            Num8 => "8",
            Num9 => "9",
            F1 => "F1",
            F2 => "F2",
            F3 => "F3",
            F4 => "F4",
            F5 => "F5",
            F6 => "F6",
            F7 => "F7",
            F8 => "F8",
            F9 => "F9",
            F10 => "F10",
            F11 => "F11",
            F12 => "F12",
            Insert => "Insert",
            Shift => "Shift",
            Control => "Ctrl",
            Alt => "Alt",
            CapsLock => "Caps",
            ContextMenu => "Menu",
        }
    }
}

/// Keyboard modifier state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    /// Shift held.
    pub shift: bool,
    /// Control held.
    pub ctrl: bool,
    /// Alt / Option held.
    pub alt: bool,
    /// Command (macOS) / Windows / Super key held.
    pub logo: bool,
}

impl Modifiers {
    /// The platform's primary shortcut modifier: Cmd on macOS, Ctrl elsewhere.
    #[inline]
    pub fn command(&self) -> bool {
        if cfg!(target_os = "macos") {
            self.logo
        } else {
            self.ctrl
        }
    }

    /// Word-navigation modifier: Alt on macOS, Ctrl elsewhere.
    #[inline]
    pub fn word(&self) -> bool {
        if cfg!(target_os = "macos") {
            self.alt
        } else {
            self.ctrl
        }
    }
}

/// A single input event.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    /// Pointer moved to a logical position.
    PointerMoved {
        /// X in logical pixels.
        x: f32,
        /// Y in logical pixels.
        y: f32,
    },
    /// Pointer left the window.
    PointerLeft,
    /// A pointer button changed state.
    PointerButton {
        /// Which button.
        button: PointerButton,
        /// True when pressed.
        pressed: bool,
    },
    /// Scroll in logical pixels (positive y = content moves down / scroll up).
    Scroll {
        /// Horizontal delta.
        dx: f32,
        /// Vertical delta.
        dy: f32,
    },
    /// A key changed state.
    Key {
        /// Which key.
        key: Key,
        /// True when pressed (including auto-repeat).
        pressed: bool,
        /// Modifier state at the time of the event.
        modifiers: Modifiers,
    },
    /// Modifier state changed.
    ModifiersChanged(Modifiers),
    /// Committed text input (already composed, never control characters).
    Text(String),
    /// The window gained (`true`) or lost focus.
    WindowFocused(bool),
    /// Text pasted from the clipboard (inserted into the focused text input).
    Paste(String),
}
