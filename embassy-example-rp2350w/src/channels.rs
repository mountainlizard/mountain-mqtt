use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::pubsub::{PubSubChannel, Publisher, Subscriber};

use crate::action::Action;
use crate::event::Event;

const ACTION_CAP: usize = 16;
const ACTION_SUBS: usize = 4;
const ACTION_PUBS: usize = 4;
pub type ActionChannel = PubSubChannel<NoopRawMutex, Action, ACTION_CAP, ACTION_SUBS, ACTION_PUBS>;
pub type ActionPub = Publisher<'static, NoopRawMutex, Action, ACTION_CAP, ACTION_SUBS, ACTION_PUBS>;
pub type ActionSub =
    Subscriber<'static, NoopRawMutex, Action, ACTION_CAP, ACTION_SUBS, ACTION_PUBS>;

const EVENT_CAP: usize = 16;
const EVENT_SUBS: usize = 4;
const EVENT_PUBS: usize = 2;
pub type EventChannel = PubSubChannel<NoopRawMutex, Event, EVENT_CAP, EVENT_SUBS, EVENT_PUBS>;
pub type EventPub = Publisher<'static, NoopRawMutex, Event, EVENT_CAP, EVENT_SUBS, EVENT_PUBS>;
pub type EventSub = Subscriber<'static, NoopRawMutex, Event, EVENT_CAP, EVENT_SUBS, EVENT_PUBS>;
