//! Command queue processing for the VISCA runtime.

use tracing::{debug, error};

use super::{scheduler::Scheduler, tx::handle_tx_item};
use crate::{
    error::Result,
    transport::{buffer::BufferManager, envelope::TransportEnvelope, AsyncTransport},
};

/// Process commands from the priority queue.
pub async fn process_command_queue<T: AsyncTransport + Send, E: crate::executor::Executor>(
    transport: &mut T,
    scheduler: &mut Scheduler,
    executor: &E,
    allow_retry_defer: bool,
    envelope: &TransportEnvelope,
    buffer_manager: &BufferManager,
) -> Result<()> {
    // Process commands from the priority queue while we can send more
    // But check if there's a higher priority retry ready first
    while scheduler.can_send_command() && !scheduler.is_queue_empty() {
        // Check if there's a retry ready that has higher or equal priority than the next queued command
        let now = executor.now();
        if allow_retry_defer {
            if let Some(next_queue_priority) = scheduler.peek_queue_priority() {
                if let Some(retry_priority) = scheduler.peek_ready_retry_priority(now) {
                    // If retry has higher or equal priority, don't process queue yet
                    if retry_priority >= next_queue_priority {
                        debug!(
                            "Deferring queue processing - retry with priority {retry_priority:?} waiting (queue has {queue_priority:?})",
                            retry_priority = retry_priority, queue_priority = next_queue_priority
                        );
                        return Ok(());
                    }
                }
            }
        }

        if let Some(item) = scheduler.dequeue_command() {
            debug!(
                "Processing queued command from priority queue (remaining: {remaining})",
                remaining = scheduler.queue_size()
            );
            // Process the dequeued command
            if let Err(e) = handle_tx_item(
                transport,
                scheduler,
                item,
                executor,
                envelope,
                buffer_manager,
            )
            .await
            {
                error!("Error processing queued command: {e}");
            }
        }
    }
    Ok(())
}
