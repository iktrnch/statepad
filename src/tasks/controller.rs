//! Controller actor: the sole owner of mutable automaton state.

use embassy_futures::select::{Either, select};
use embassy_rp::rom_data::reset_to_usb_boot;
use embassy_sync::channel::{Receiver, Sender};
use embassy_sync::watch;
use embassy_time::{Duration, Instant, Timer};

use crate::app::AppRawMutex;
use crate::domain::{
    AutomatonRuntime, ControllerEvent, Decision, DisplayModel, FirmwareError, HidCommand, Profile,
    SystemStatus,
};

type EventReceiver = Receiver<'static, AppRawMutex, ControllerEvent, 16>;
type HidSender = Sender<'static, AppRawMutex, HidCommand, 8>;
type DisplaySender = watch::Sender<'static, AppRawMutex, DisplayModel, 1>;

const BOOTLOADER_RELEASE_TIMEOUT: Duration = Duration::from_millis(2_500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingAction {
    Fatal { request_id: u32 },
    BootloaderDisplay { request_id: u32 },
    BootloaderHid { request_id: u32, deadline: Instant },
}

struct Controller {
    events: EventReceiver,
    hid: HidSender,
    display: DisplaySender,
    profiles: &'static [Profile],
    automaton: AutomatonRuntime,
    next_system_request_id: u32,
    pending_action: Option<PendingAction>,
}

impl Controller {
    async fn run(mut self) -> ! {
        if self.profiles.is_empty() {
            self.begin_fatal(FirmwareError::NoProfilesConfigured);
        } else if !self.profiles.iter().all(Profile::is_valid) {
            self.begin_fatal(FirmwareError::InvalidProfile);
        } else {
            let initial = self.automaton.initial_decision(self.profiles);
            self.forward(initial);
        }

        loop {
            let event = match self.pending_action {
                Some(PendingAction::BootloaderHid { deadline, .. }) => {
                    match select(self.events.receive(), Timer::at(deadline)).await {
                        Either::First(event) => event,
                        Either::Second(()) => {
                            reset_to_usb_boot(0, 0);
                            core::future::pending::<ControllerEvent>().await
                        }
                    }
                }
                _ => self.events.receive().await,
            };
            self.dispatch(event).await;
        }
    }

    async fn dispatch(&mut self, event: ControllerEvent) {
        match event {
            ControllerEvent::FatalError(error) => {
                if matches!(
                    self.pending_action,
                    Some(PendingAction::BootloaderDisplay { .. })
                        | Some(PendingAction::BootloaderHid { .. })
                ) {
                    return;
                }
                self.begin_fatal(error);
                return;
            }
            ControllerEvent::DisplayPresented { request_id } => {
                match self.pending_action {
                    Some(PendingAction::Fatal {
                        request_id: pending,
                    }) if request_id == pending => {
                        crate::tasks::display::preserve_screen_on_panic();
                        panic!("firmware halted after displaying a fatal error");
                    }
                    Some(PendingAction::BootloaderDisplay {
                        request_id: pending,
                    }) if request_id == pending => {
                        self.pending_action = Some(PendingAction::BootloaderHid {
                            request_id,
                            deadline: Instant::now() + BOOTLOADER_RELEASE_TIMEOUT,
                        });
                        if self
                            .hid
                            .try_send(HidCommand::ReleaseForBootloader { request_id })
                            .is_err()
                        {
                            // The OLED already confirms BOOTSEL. A saturated HID queue
                            // must not prevent entering the ROM bootloader.
                            Timer::after(Duration::from_millis(30)).await;
                            reset_to_usb_boot(0, 0);
                        }
                    }
                    _ => {}
                }
                return;
            }
            ControllerEvent::HidReleasedForBootloader { request_id } => {
                if let Some(PendingAction::BootloaderHid {
                    request_id: pending,
                    ..
                }) = self.pending_action
                    && request_id == pending
                {
                    // Keep USB alive briefly after the acknowledged empty reports reach the writer.
                    Timer::after(Duration::from_millis(30)).await;
                    reset_to_usb_boot(0, 0);
                }
            }
            _ => {}
        }

        if self.pending_action.is_some() {
            return;
        }

        let decision = self.automaton.handle_event(event, self.profiles);
        self.forward(decision);
    }

    fn forward(&mut self, decision: Decision) {
        // Ordering is intentional: release/update HID before publishing its matching frame.
        if decision.bootloader {
            self.begin_bootloader();
            return;
        }

        if let Some(hid) = decision.hid
            && self.hid.try_send(hid).is_err()
        {
            self.begin_fatal(FirmwareError::HidCommandQueueFull);
            return;
        }
        if let Some(display) = decision.display {
            self.display.send(display);
        }
    }

    fn begin_fatal(&mut self, error: FirmwareError) {
        if matches!(self.pending_action, Some(PendingAction::Fatal { .. })) {
            return;
        }

        let request_id = self.allocate_system_request_id();
        self.pending_action = Some(PendingAction::Fatal { request_id });
        self.display.send(DisplayModel::System {
            request_id,
            status: SystemStatus::Fatal(error),
        });
    }

    fn begin_bootloader(&mut self) {
        let request_id = self.allocate_system_request_id();
        self.pending_action = Some(PendingAction::BootloaderDisplay { request_id });
        self.display.send(DisplayModel::System {
            request_id,
            status: SystemStatus::Bootloader,
        });
    }

    fn allocate_system_request_id(&mut self) -> u32 {
        let request_id = self.next_system_request_id;
        self.next_system_request_id = self.next_system_request_id.wrapping_add(1);
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
        next_system_request_id: 1,
        pending_action: None,
    }
    .run()
    .await;
}
