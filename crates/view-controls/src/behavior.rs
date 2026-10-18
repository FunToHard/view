use crate::ControlError;
use view_core::{
    Key, KeyPhase, KeyboardEvent, NamedKey, PointerButton, PointerEvent, PointerPhase,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Interaction {
    pub disabled: bool,
    pub focused: bool,
    pub hovered: bool,
    pub pressed: bool,
}
/// Activates on primary pointer release inside, Space release, or first Enter
/// down. Repeat, cancellation, focus loss and disabling never double-activate.
#[derive(Clone, Debug, Default)]
pub struct Pressable {
    state: Interaction,
    source: Option<PressSource>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PressSource {
    Pointer,
    Space,
}
impl Pressable {
    pub fn state(&self) -> Interaction {
        self.state
    }
    pub fn set_disabled(&mut self, disabled: bool) {
        self.state.disabled = disabled;
        if disabled {
            self.set_focused(false);
            self.cancel();
        }
    }
    pub fn set_focused(&mut self, focused: bool) {
        self.state.focused = focused && !self.state.disabled;
        if !self.state.focused {
            self.cancel();
        }
    }
    pub fn cancel(&mut self) {
        self.source = None;
        self.state.pressed = false;
    }
    pub fn pointer(&mut self, event: PointerEvent, inside: bool) -> bool {
        self.state.hovered = inside;
        if self.state.disabled {
            self.cancel();
            return false;
        }
        match event.phase {
            PointerPhase::Down if event.button == Some(PointerButton::Primary) && inside => {
                self.source = Some(PressSource::Pointer);
                self.state.pressed = true;
            }
            PointerPhase::Move if self.source == Some(PressSource::Pointer) => {
                self.state.pressed = inside
            }
            PointerPhase::Up
                if event.button == Some(PointerButton::Primary)
                    && self.source == Some(PressSource::Pointer) =>
            {
                self.cancel();
                return inside;
            }
            PointerPhase::Cancel => self.cancel(),
            _ => {}
        }
        false
    }
    pub fn key(&mut self, event: &KeyboardEvent) -> bool {
        if self.state.disabled
            || !self.state.focused
            || event.modifiers.ctrl
            || event.modifiers.alt
            || event.modifiers.meta
        {
            return false;
        }
        match (&event.key, event.phase) {
            (Key::Named(NamedKey::Enter), KeyPhase::Down) => true,
            (Key::Named(NamedKey::Space), KeyPhase::Down) => {
                self.source = Some(PressSource::Space);
                self.state.pressed = true;
                false
            }
            (Key::Named(NamedKey::Space), KeyPhase::Up)
                if self.source == Some(PressSource::Space) =>
            {
                self.cancel();
                true
            }
            (Key::Named(NamedKey::Escape), _) => {
                self.cancel();
                false
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeSpec {
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub page: f64,
    pub decimals: u8,
}
impl RangeSpec {
    pub fn validate(self) -> Result<Self, ControlError> {
        if [self.min, self.max, self.step, self.page]
            .iter()
            .any(|v| !v.is_finite())
            || self.min >= self.max
            || !(self.max - self.min).is_finite()
            || self.step <= 0.0
            || self.page <= 0.0
            || self.decimals > 12
        {
            Err(ControlError::Invalid("range"))
        } else {
            Ok(self)
        }
    }
    pub fn clamp(self, value: f64) -> Result<f64, ControlError> {
        self.validate()?;
        if !value.is_finite() {
            return Err(ControlError::Invalid("range value"));
        }
        let factor = 10f64.powi(self.decimals as i32);
        let rounded = (value.clamp(self.min, self.max) * factor).round() / factor;
        if !rounded.is_finite() {
            return Err(ControlError::Invalid("range precision overflow"));
        }
        Ok(rounded.clamp(self.min, self.max))
    }
}
#[derive(Clone, Debug)]
pub struct RangeBehavior {
    spec: RangeSpec,
    value: f64,
    pub disabled: bool,
}
impl RangeBehavior {
    pub fn new(spec: RangeSpec, value: f64) -> Result<Self, ControlError> {
        Ok(Self {
            spec: spec.validate()?,
            value: spec.clamp(value)?,
            disabled: false,
        })
    }
    pub fn spec(&self) -> RangeSpec {
        self.spec
    }
    pub fn value(&self) -> f64 {
        self.value
    }
    pub fn set(&mut self, value: f64) -> Result<bool, ControlError> {
        if self.disabled {
            return Err(ControlError::Disabled);
        }
        let value = self.spec.clamp(value)?;
        let changed = value != self.value;
        self.value = value;
        Ok(changed)
    }
    pub fn fraction(&self) -> f64 {
        (self.value - self.spec.min) / (self.spec.max - self.spec.min)
    }
    pub fn set_fraction(&mut self, f: f64) -> Result<bool, ControlError> {
        if !f.is_finite() {
            return Err(ControlError::Invalid("range fraction"));
        }
        let raw = self.spec.min + f.clamp(0.0, 1.0) * (self.spec.max - self.spec.min);
        let stepped =
            self.spec.min + ((raw - self.spec.min) / self.spec.step).round() * self.spec.step;
        self.set(stepped)
    }
    pub fn key(&mut self, event: &KeyboardEvent) -> Result<bool, ControlError> {
        if event.phase == KeyPhase::Up || self.disabled {
            return Ok(false);
        }
        let value = match event.key {
            Key::Named(NamedKey::ArrowRight | NamedKey::ArrowUp) => self.value + self.spec.step,
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowDown) => self.value - self.spec.step,
            Key::Named(NamedKey::PageUp) => self.value + self.spec.page,
            Key::Named(NamedKey::PageDown) => self.value - self.spec.page,
            Key::Named(NamedKey::Home) => self.spec.min,
            Key::Named(NamedKey::End) => self.spec.max,
            _ => return Ok(false),
        };
        self.set(value)
    }
}
