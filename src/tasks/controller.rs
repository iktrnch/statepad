//! Controller actor: the sole owner of mutable automaton state.

use embassy_rp::rom_data::reset_to_usb_boot;
use embassy_sync::channel::{Receiver, Sender};
use embassy_sync::watch;
use embassy_time::{Duration, Timer};

use crate::app::AppRawMutex;
use crate::domain::{
    AutomatonRuntime, ControllerEvent, Decision, DisplayModel, HidCommand, Profile,
};

type EventReceiver = Receiver<'static, AppRawMutex, ControllerEvent, 16>;
type HidSender = Sender<'static, AppRawMutex, HidCommand, 8>;
type DisplaySender = watch::Sender<'static, AppRawMutex, DisplayModel, 1>;

struct Controller {
    events: EventReceiver,
    hid: HidSender,
    display: DisplaySender,
    profiles: &'static [Profile],
    automaton: AutomatonRuntime,
    next_bootloader_request_id: u32,
    pending_bootloader_request: Option<u32>,
}

impl Controller {
    async fn run(mut self) -> ! {
        let initial = self.automaton.initial_decision(self.profiles);
        self.forward(initial).await;

        loop {
            let event = self.events.receive().await;
            self.dispatch(event).await;
        }
    }

    async fn dispatch(&mut self, event: ControllerEvent) {
        if let ControllerEvent::HidReleasedForBootloader { request_id } = event {
            if self.pending_bootloader_request == Some(request_id) {
                // Keep USB alive briefly after the acknowledged empty reports reach the writer.
                Timer::after(Duration::from_millis(30)).await;
                reset_to_usb_boot(0, 0);
            }
            return;
        }

        if self.pending_bootloader_request.is_some() {
            return;
        }

        let decision = self.automaton.handle_event(event, self.profiles);
        self.forward(decision).await;
    }

    async fn forward(&mut self, decision: Decision) {
        // Ordering is intentional: release/update HID before publishing its matching frame.
        if decision.bootloader {
            let request_id = self.allocate_bootloader_request_id();
            self.pending_bootloader_request = Some(request_id);
            self.hid
                .send(HidCommand::ReleaseForBootloader { request_id })
                .await;
            if let Some(display) = decision.display {
                self.display.send(display);
            }
            return;
        }

        if let Some(hid) = decision.hid {
            self.hid.send(hid).await;
        }
        if let Some(display) = decision.display {
            self.display.send(display);
        }
    }

    fn allocate_bootloader_request_id(&mut self) -> u32 {
        let request_id = self.next_bootloader_request_id;
        self.next_bootloader_request_id = self.next_bootloader_request_id.wrapping_add(1);
        request_id
    }
}

#[embassy_executor::task]
pub async fn run(
    events: EventReceiver,
    hid: HidSender,
    display: DisplaySender,
    profiles: &'static [Profile],
) {
    Controller {
        events,
        hid,
        display,
        profiles,
        automaton: AutomatonRuntime::new(),
        next_bootloader_request_id: 1,
        pending_bootloader_request: None,
    }
    .run()
    .await;
}
