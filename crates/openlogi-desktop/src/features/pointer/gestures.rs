//! Per-device gesture tuning for the pointer-detail column.
//!
//! Two sliders over the selected device's gesture config: **sensitivity**
//! (how long a gesture button must be held, and how far it must travel,
//! before a swipe commits) and **axis bias** (how strongly Left/Right is
//! favored over Up/Down). Each release persists a device override through
//! [`AppState::commit_device_gesture_sensitivity`] /
//! [`AppState::commit_device_gesture_axis_bias`]; a device without one follows
//! the app-wide value from Settings → General. The agent turns both into
//! `SwipeThresholds` for the HID++ and OS-hook gesture paths.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, IntoElement, ParentElement, Render, SharedString, Styled, Subscription, Window, div,
};
use gpui_kit::component::{slider::Slider, v_flex};
use openlogi_core::config::{GestureAxisBias, GestureSensitivity};

use crate::state::{AppState, DeviceRecord, StateEvent, StateEvents};
use crate::ui::commit_slider::{CommitSlider, SliderRange, SliderUnit};
use crate::ui::slider_row::SliderRow;
use crate::ui::{components::control_button, theme};

pub struct GesturePanel {
    /// The selected device's gesture sensitivity (device override, else the
    /// app-wide default).
    sensitivity: CommitSlider<GestureSensitivity>,
    /// The selected device's gesture axis bias (device override, else the
    /// app-wide default).
    axis_bias: CommitSlider<GestureAxisBias>,
    _state_obs: Subscription,
}

impl GesturePanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let sensitivity = CommitSlider::new(
            SliderRange::new(GestureSensitivity::MIN, GestureSensitivity::MAX),
            GestureSensitivity::DEFAULT,
            cx,
            |_, sensitivity, cx| {
                AppState::apply(cx, |state| {
                    state
                        .current_record()
                        .map(DeviceRecord::device_key)
                        .map_or_else(StateEvents::none, |key| {
                            state.commit_device_gesture_sensitivity(&key, sensitivity)
                        })
                });
            },
        );
        let axis_bias = CommitSlider::new(
            SliderRange::new(GestureAxisBias::MIN, GestureAxisBias::MAX),
            GestureAxisBias::DEFAULT,
            cx,
            |_, bias, cx| {
                AppState::apply(cx, |state| {
                    state
                        .current_record()
                        .map(DeviceRecord::device_key)
                        .map_or_else(StateEvents::none, |key| {
                            state.commit_device_gesture_axis_bias(&key, bias)
                        })
                });
            },
        );
        let state_obs = AppState::repaint_on(cx, |event| {
            matches!(
                event,
                StateEvent::DeviceConfigChanged(_) | StateEvent::SettingsChanged
            )
        });
        Self {
            sensitivity,
            axis_bias,
            _state_obs: state_obs,
        }
    }
}

impl Render for GesturePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (sensitivity, axis_bias, global_sensitivity, global_bias, has_override) =
            AppState::try_read(cx)
                .and_then(|state| {
                    let key = &state.current_record()?.config_key;
                    Some((
                        state.device_gesture_sensitivity(key),
                        state.device_gesture_axis_bias(key),
                        state.app_settings().gesture_sensitivity,
                        state.app_settings().gesture_axis_bias,
                        state.has_device_gesture_override(key),
                    ))
                })
                .unwrap_or((
                    GestureSensitivity::DEFAULT,
                    GestureAxisBias::DEFAULT,
                    GestureSensitivity::DEFAULT,
                    GestureAxisBias::DEFAULT,
                    false,
                ));
        let sensitivity_row = slider_row(
            &mut self.sensitivity,
            sensitivity,
            [GestureSensitivity::MIN, GestureSensitivity::MAX],
            tr!("pointer.gesture_sensitivity"),
            tr!("pointer.gesture_sensitivity_description"),
            window,
            cx,
        );
        let axis_bias_row = slider_row(
            &mut self.axis_bias,
            axis_bias,
            [GestureAxisBias::MIN, GestureAxisBias::MAX],
            tr!("pointer.gesture_axis_bias"),
            tr!("pointer.gesture_axis_bias_description"),
            window,
            cx,
        );
        v_flex()
            .gap_4()
            .w_full()
            .child(sensitivity_row)
            .child(axis_bias_row)
            .child(
                div()
                    .text_sm()
                    .text_color(theme::palette(cx).text_muted)
                    .child(tr!(
                        "pointer.gesture_global_defaults",
                        sensitivity = global_sensitivity,
                        bias = global_bias
                    )),
            )
            .when(has_override, |panel| {
                panel.child(
                    control_button("gesture-follow-global")
                        .label(tr!("pointer.gesture_follow_global"))
                        .on_click(|_, _, cx| {
                            AppState::apply(cx, |state| {
                                state
                                    .current_record()
                                    .map(DeviceRecord::device_key)
                                    .map_or_else(StateEvents::none, |key| {
                                        state.reset_device_gesture_settings(&key)
                                    })
                            });
                        }),
                )
            })
    }
}

/// One slider row seated on `committed`, with its live value and a caption.
/// The value column is sized for the wider of `bounds`.
fn slider_row<T: SliderUnit + std::fmt::Display>(
    slider: &mut CommitSlider<T>,
    committed: T,
    bounds: [T; 2],
    label: impl Into<SharedString>,
    description: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut Context<GesturePanel>,
) -> SliderRow {
    slider.sync(committed, window, cx);
    let display = slider.shown(committed);
    SliderRow::new(label, Slider::new(slider.slider()).horizontal())
        .value(display.to_string(), bounds.map(|b| b.to_string()))
        .description(description)
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext as _, TestAppContext};
    use openlogi_core::config::Config;

    use super::*;
    use crate::services::assets::AssetResolver;
    use crate::state::{Sources, tests::direct_inventory};

    #[gpui::test]
    fn inherited_gesture_sliders_follow_global_changes(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(|cx| {
            let resolver = AssetResolver::new();
            let (commands, _receiver) = tokio::sync::mpsc::unbounded_channel();
            let inventories = [direct_inventory([1, 2, 3, 4])];
            let state = cx.new(|_| {
                AppState::new(Sources {
                    inventories: &inventories,
                    ..Sources::in_memory(Config::ephemeral(), &resolver, commands)
                })
            });
            AppState::set_global(state, cx);
        });
        let (panel, cx) = cx.add_window_view(|_, cx| GesturePanel::new(cx));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let repainted = std::rc::Rc::new(std::cell::Cell::new(false));
        let _watch = cx.update(|_, cx| {
            let repainted = repainted.clone();
            cx.observe(&panel, move |_, _| repainted.set(true))
        });
        let sensitivity = GestureSensitivity::MAX;
        let bias = GestureAxisBias::MIN;
        cx.update(|_, cx| {
            AppState::apply(cx, |state| state.commit_gesture_sensitivity(sensitivity));
        });
        cx.update(|_, cx| AppState::apply(cx, |state| state.commit_gesture_axis_bias(bias)));
        cx.run_until_parked();
        assert!(
            repainted.get(),
            "global gesture changes must repaint the open device panel"
        );
        cx.update(|window, cx| window.draw(cx).clear(cx));
        panel.read_with(cx, |panel, cx| {
            assert_eq!(panel.sensitivity.value(cx), sensitivity);
            assert_eq!(panel.axis_bias.value(cx), bias);
        });
        cx.update(|_, cx| {
            AppState::apply(cx, |state| {
                let key = state.current_record().unwrap().device_key();
                state.commit_device_gesture_sensitivity(&key, GestureSensitivity::MIN)
            });
        });
        cx.update(|_, cx| {
            AppState::apply(cx, |state| {
                state.commit_gesture_sensitivity(GestureSensitivity::DEFAULT)
            });
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        panel.read_with(cx, |panel, cx| {
            assert_eq!(
                panel.sensitivity.value(cx),
                GestureSensitivity::MIN,
                "a global change must preserve a device override"
            );
            assert_eq!(AppState::try_read(cx).unwrap().gesture_override_count(), 1);
        });
        cx.update(|_, cx| {
            AppState::apply(cx, |state| {
                let key = state.current_record().unwrap().device_key();
                state.reset_device_gesture_settings(&key)
            });
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        panel.read_with(cx, |panel, cx| {
            assert_eq!(panel.sensitivity.value(cx), GestureSensitivity::DEFAULT);
            assert_eq!(AppState::try_read(cx).unwrap().gesture_override_count(), 0);
        });
    }
}
