use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandardEndpoint {
    Start,
    Stop,
    IsWorking,
    DataValid,
    DataMonitor,
    RawData,
    Inbox,
    Outbox,
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
        ]
    }
}
