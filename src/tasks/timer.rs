//! Replaceable, cancellation-responsive transition timer actor.

use embassy_futures::select::{Either, select};
use embassy_sync::channel::{Receiver, Sender};
use embassy_time::{Duration, Instant, Timer};

use crate::app::AppRawMutex;
use crate::domain::{ControllerEvent, TimerCommand};

type TimerReceiver = Receiver<'static, AppRawMutex, TimerCommand, 4>;
type EventSender = Sender<'static, AppRawMutex, ControllerEvent, 16>;

#[derive(Clone, Copy)]
struct ActiveTimer {
    generation: u32,
    deadline: Instant,
}

#[embassy_executor::task]
pub async fn run(commands: TimerReceiver, events: EventSender) {
    let mut active: Option<ActiveTimer> = None;

    loop {
        match active {
            None => apply_command(&mut active, commands.receive().await),
            Some(timer) => match select(commands.receive(), Timer::at(timer.deadline)).await {
                Either::First(command) => apply_command(&mut active, command),
                Either::Second(()) => {
                    active = None;
                    events
                        .send(ControllerEvent::TransitionElapsed {
                            generation: timer.generation,
                        })
                        .await;
                }
            },
        }
    }
}

fn apply_command(active: &mut Option<ActiveTimer>, command: TimerCommand) {
    match command {
        TimerCommand::Start {
            generation,
            duration_ms,
        } => {
            *active = Some(ActiveTimer {
                generation,
                deadline: Instant::now() + Duration::from_millis(duration_ms as u64),
            });
        }
        TimerCommand::Cancel { generation }
            if active
                .as_ref()
                .is_some_and(|timer| timer.generation == generation) =>
        {
            *active = None;
        }
        TimerCommand::Cancel { .. } => {}
    }
}
