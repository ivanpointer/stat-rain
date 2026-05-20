use crate::effect::{MessageOverlay, MessageTiming};
use crate::message::MessageClass;
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedMessage {
    pub text: String,
    pub class: MessageClass,
    pub ttl_frames: Option<u64>,
}

impl QueuedMessage {
    pub fn new(text: String, class: MessageClass, ttl_frames: Option<u64>) -> Self {
        Self {
            text,
            class,
            ttl_frames,
        }
    }

    fn same_identity(&self, text: &str, class: MessageClass) -> bool {
        self.text == text && self.class == class
    }
}

#[derive(Debug, Default, Clone)]
pub struct MessageQueue {
    pending: VecDeque<QueuedMessage>,
}

impl MessageQueue {
    pub fn enqueue_or_refresh(
        &mut self,
        active: &mut Option<MessageOverlay>,
        message: QueuedMessage,
        timing: MessageTiming,
    ) {
        if let Some(active) = active.as_mut() {
            if active.text == message.text && active.class == message.class {
                if let Some(ttl_frames) = message.ttl_frames {
                    active.refresh_stay(timing.stay_frames_for_ttl(ttl_frames));
                }
                return;
            }
        }

        if let Some(existing) = self
            .pending
            .iter_mut()
            .find(|queued| queued.same_identity(&message.text, message.class))
        {
            existing.ttl_frames = max_ttl(existing.ttl_frames, message.ttl_frames);
            return;
        }

        self.pending.push_back(message);
    }

    pub fn pop_next(&mut self) -> Option<QueuedMessage> {
        self.pending.pop_front()
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

fn max_ttl(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

pub fn frames_from_ms(ms: u64, frame_delay_ms: u64) -> u64 {
    if ms == 0 {
        return 0;
    }
    let frame_delay_ms = frame_delay_ms.max(1);
    ms.div_ceil(frame_delay_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timing() -> MessageTiming {
        MessageTiming {
            fade_in: 1,
            stay: 10,
            fade_out: 1,
            frame_delay: 33,
        }
    }

    #[test]
    fn pops_messages_fifo() {
        let mut queue = MessageQueue::default();
        let mut active = None;

        queue.enqueue_or_refresh(
            &mut active,
            QueuedMessage::new("ONE".to_string(), MessageClass::Info, None),
            timing(),
        );
        queue.enqueue_or_refresh(
            &mut active,
            QueuedMessage::new("TWO".to_string(), MessageClass::Info, None),
            timing(),
        );

        assert_eq!(queue.pop_next().unwrap().text, "ONE");
        assert_eq!(queue.pop_next().unwrap().text, "TWO");
    }

    #[test]
    fn coalesces_queued_duplicates_by_text_and_class() {
        let mut queue = MessageQueue::default();
        let mut active = None;

        queue.enqueue_or_refresh(
            &mut active,
            QueuedMessage::new("BUILD".to_string(), MessageClass::Warning, Some(1_000)),
            timing(),
        );
        queue.enqueue_or_refresh(
            &mut active,
            QueuedMessage::new("BUILD".to_string(), MessageClass::Warning, Some(2_000)),
            timing(),
        );

        assert_eq!(queue.len(), 1);
        assert_eq!(queue.pop_next().unwrap().ttl_frames, Some(2_000));
    }

    #[test]
    fn does_not_coalesce_same_text_with_different_class() {
        let mut queue = MessageQueue::default();
        let mut active = None;

        queue.enqueue_or_refresh(
            &mut active,
            QueuedMessage::new("BUILD".to_string(), MessageClass::Warning, None),
            timing(),
        );
        queue.enqueue_or_refresh(
            &mut active,
            QueuedMessage::new("BUILD".to_string(), MessageClass::Error, None),
            timing(),
        );

        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn refreshes_active_duplicate_stay() {
        let mut active = Some(MessageOverlay::new("BUILD".to_string(), 1, 3, 1, 7));
        active.as_mut().unwrap().class = MessageClass::Warning;
        let mut queue = MessageQueue::default();
        let timing = MessageTiming {
            fade_in: 1,
            stay: 20,
            fade_out: 1,
            frame_delay: 50,
        };

        queue.enqueue_or_refresh(
            &mut active,
            QueuedMessage::new("BUILD".to_string(), MessageClass::Warning, Some(20)),
            timing,
        );

        assert_eq!(active.unwrap().stay, 20);
        assert!(queue.is_empty());
    }

    #[test]
    fn converts_milliseconds_to_frame_count_with_ceiling() {
        assert_eq!(frames_from_ms(100, 33), 4);
        assert_eq!(frames_from_ms(0, 33), 0);
        assert_eq!(frames_from_ms(100, 0), 100);
    }
}
