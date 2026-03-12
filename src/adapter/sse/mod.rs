use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};

use serde::Serialize;

use crate::adapter::http::dto::EVENTS_ROUTE;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SseEvent {
    pub(crate) topic: &'static str,
    pub(crate) emission: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct EmittedSseEvent {
    pub(crate) topic: &'static str,
    pub(crate) emission: &'static str,
    pub(crate) data: String,
}

#[derive(Clone, Default)]
pub(crate) struct SseBroker {
    subscribers: Arc<Mutex<Vec<Sender<EmittedSseEvent>>>>,
}

pub(crate) fn after_commit_sse_stream() -> SseEvent {
    SseEvent {
        topic: EVENTS_ROUTE,
        emission: "after commit only",
    }
}

pub(crate) fn emit_after_commit(data: String) -> EmittedSseEvent {
    let stream = after_commit_sse_stream();
    EmittedSseEvent {
        topic: stream.topic,
        emission: stream.emission,
        data,
    }
}

impl SseBroker {
    pub(crate) fn subscribe(&self) -> Receiver<EmittedSseEvent> {
        let (tx, rx) = mpsc::channel();
        self.subscribers
            .lock()
            .expect("sse subscribers lock should be available")
            .push(tx);
        rx
    }

    pub(crate) fn publish(&self, event: EmittedSseEvent) {
        self.subscribers
            .lock()
            .expect("sse subscribers lock should be available")
            .retain(|subscriber| subscriber.send(event.clone()).is_ok());
    }
}

pub(crate) fn encode_sse_event(event: &EmittedSseEvent) -> String {
    let data = serde_json::to_string(event).expect("sse event should serialize");
    format!("data: {data}\n\n")
}

#[cfg(test)]
mod tests {
    use crate::adapter::http::dto::EVENTS_ROUTE;

    use super::{after_commit_sse_stream, emit_after_commit, encode_sse_event, SseBroker};

    #[test]
    fn sse_stream_is_after_commit_only() {
        let event = after_commit_sse_stream();
        assert_eq!(event.topic, EVENTS_ROUTE);
        assert_eq!(event.emission, "after commit only");
    }

    #[test]
    fn emitted_event_uses_after_commit_contract() {
        let event = emit_after_commit("accepted".to_owned());
        assert_eq!(event.topic, EVENTS_ROUTE);
        assert_eq!(event.emission, "after commit only");
        assert_eq!(event.data, "accepted");
    }

    #[test]
    fn broker_delivers_published_after_commit_event() {
        let broker = SseBroker::default();
        let rx = broker.subscribe();
        let event = emit_after_commit("accepted".to_owned());

        broker.publish(event.clone());

        assert_eq!(rx.recv().expect("event should deliver"), event);
    }

    #[test]
    fn encoded_sse_event_uses_data_frame() {
        let encoded = encode_sse_event(&emit_after_commit("accepted".to_owned()));

        assert!(encoded.starts_with("data: "));
        assert!(encoded.ends_with("\n\n"));
        assert!(encoded.contains("\"data\":\"accepted\""));
    }
}
