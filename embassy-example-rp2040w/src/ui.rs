use crate::action::Action;
use crate::channels::{ActionPub, EventSub};
use crate::event::Event;
use cyw43::Control;
use defmt::info;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::PIN_12;
use embassy_time::{Duration, Ticker};

#[embassy_executor::task]
pub async fn ui_task(
    mut event_sub: EventSub,
    action_pub: ActionPub,
    input: PIN_12,
    mut control: Control<'static>,
) -> ! {
    let mut ticker = Ticker::every(Duration::from_millis(1));
    let mut debounce: u32 = 0;
    let debounce_max = 10;
    let mut pressed = false;
    let input = Input::new(input, Pull::Up);

    loop {
        // Check for new events - if one is received, apply it,
        // and send back any response
        if let Some(message) = event_sub.try_next_message_pure() {
            info!("Event: {}", message);
            match message {
                Event::Led(on) => control.gpio_set(0, on).await,
            }
            // Note: We could reply to events here by publishing to action_pub
        }

        let press = if input.is_low() {
            debounce = debounce_max;
            if !pressed {
                pressed = true;
                Some(true)
            } else {
                None
            }
        } else if debounce > 0 {
            debounce -= 1;
            if debounce == 0 && pressed {
                pressed = false;
                Some(false)
            } else {
                None
            }
        } else {
            None
        };

        // Don't overfill the publisher with button presses, they are the least
        // important item, so just lose them if publisher is getting full.
        if let Some(press) = press {
            if press {
                info!("Button pressed");
            } else {
                info!("Button released");
            }
            if action_pub.free_capacity() > 8 {
                action_pub.publish_immediate(Action::Button(press));
            }
        }

        ticker.next().await;
    }
}
