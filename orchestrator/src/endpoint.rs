use std::fmt;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandardEndpoint {
    Start = 0,
    Stop = 1,
    IsWorking = 2,
    DataValid = 3,
    DataMonitor = 4,
    RawData = 5,
    Inbox = 6,
    Outbox = 7,
    GetSubscriptions = 8,
}

impl fmt::Display for StandardEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StandardEndpoint::Start => write!(f, "start"),
            StandardEndpoint::Stop => write!(f, "stop"),
            StandardEndpoint::IsWorking => write!(f, "is_working"),
            StandardEndpoint::DataValid => write!(f, "data_valid"),
            StandardEndpoint::DataMonitor => write!(f, "data_monitor"),
            StandardEndpoint::RawData => write!(f, "raw_data"),
            StandardEndpoint::Inbox => write!(f, "inbox"),
            StandardEndpoint::Outbox => write!(f, "outbox"),
            StandardEndpoint::GetSubscriptions => write!(f, "get_subscriptions"),
        }
    }
}

impl StandardEndpoint {
    pub fn all() -> Vec<StandardEndpoint> {
        vec![
            StandardEndpoint::Start,
            StandardEndpoint::Stop,
            StandardEndpoint::IsWorking,
            StandardEndpoint::DataValid,
            StandardEndpoint::DataMonitor,
            StandardEndpoint::RawData,
            StandardEndpoint::Inbox,
            StandardEndpoint::Outbox,
            StandardEndpoint::GetSubscriptions,
        ]
    }
}
