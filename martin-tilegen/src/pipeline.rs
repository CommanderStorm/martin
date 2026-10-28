//! Ordered parallel map: the caller produces batches in tile order, worker threads encode them in any
//! order, and one writer thread consumes the results in production order.

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, Scope, ScopedJoinHandle};

use hotpath::wrap::flume::Sender as ProfiledSender;

use crate::{TileGenError, TileGenResult};

type Done<O> = (TileGenResult<O>, usize);
type Pending<O> = flume::Receiver<Done<O>>;
type Work<I, O> = (I, flume::Sender<Done<O>>);

/// How far the encoders may run ahead of the writer.
#[derive(Clone, Copy, Debug)]
pub struct Window {
    /// Batches handed out and not yet written, besides the one being written.
    pub batches: usize,
    /// Bytes of encoded batches waiting for the writer past which no new batch is handed out.
    pub bytes: usize,
}

/// Bench only: the most bytes that waited for the writer.
static PEAK: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Bytes of encoded batches waiting for the writer.
#[derive(Debug, Default)]
struct Unwritten {
    /// The bytes, and whether the writer stopped.
    state: Mutex<(usize, bool)>,
    /// Notified when the writer takes a batch or stops.
    taken: Condvar,
}

impl Unwritten {
    /// Counts an encoded batch of `bytes`.
    fn add(&self, bytes: usize) {
        let mut state = self.lock();
        state.0 += bytes;
        PEAK.fetch_max(state.0, std::sync::atomic::Ordering::Relaxed);
    }

    /// Uncounts a batch of `bytes` the writer took.
    fn take(&self, bytes: usize) {
        self.lock().0 -= bytes;
        self.taken.notify_one();
    }

    /// Marks the writer stopped, so nothing waits for it.
    fn stop(&self) {
        self.lock().1 = true;
        self.taken.notify_one();
    }

    /// Waits until at most `limit` bytes wait for the writer, or it stopped.
    fn wait(&self, limit: usize) {
        let state = self.lock();
        drop(
            self.taken
                .wait_while(state, |&mut (bytes, stopped)| bytes > limit && !stopped)
                .unwrap_or_else(PoisonError::into_inner),
        );
    }

    /// The state, also after a panic elsewhere.
    fn lock(&self) -> MutexGuard<'_, (usize, bool)> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Marks the writer stopped when dropped, also when it panics.
struct StopOnDrop<'a>(&'a Unwritten);

impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        self.0.stop();
    }
}

pub(crate) fn spawn_named<'scope, T: Send + 'scope>(
    scope: &'scope Scope<'scope, '_>,
    role: &str,
    index: usize,
    task: impl FnOnce() -> T + Send + 'scope,
) -> ScopedJoinHandle<'scope, T> {
    thread::Builder::new()
        .name(format!("{role}-{index}"))
        .spawn_scoped(scope, task)
        .expect("spawning a pipeline thread")
}

/// Hands batches to the encoders, blocking while the [`Window`] is full.
pub struct BatchSender<'a, I, O> {
    work: ProfiledSender<Work<I, O>>,
    order: ProfiledSender<Pending<O>>,
    unwritten: &'a Unwritten,
    bytes: usize,
}

impl<I, O> BatchSender<'_, I, O> {
    /// Fails only when the writer stopped early; its own error is what [`run`] returns then.
    pub fn send(&self, batch: I) -> TileGenResult<()> {
        hotpath::measure_block!({ "submit_wait_written" }, self.unwritten.wait(self.bytes));
        let (result_tx, result_rx) = flume::bounded(1);
        hotpath::measure_block!(
            { "submit_order_send" },
            self.order
                .send(result_rx)
                .map_err(|_closed| TileGenError::WriterStopped)
        )?;
        self.work
            .send((batch, result_tx))
            .map_err(|_closed| TileGenError::WriterStopped)
    }

    fn fail(&self, err: TileGenError) {
        let (result_tx, result_rx) = flume::bounded(1);
        let _ = result_tx.send((Err(err), 0));
        let _ = self.order.send(result_rx);
    }
}

/// Runs `produce` on the calling thread, `encode` on `threads` workers (each with its own state from
/// `init`, returned at the end, e.g. for statistics) and `write` on its own thread.
/// The [`Window`] bounds the batches waiting to be written, and the bytes `weigh` gives the encoded ones,
/// which bounds memory even when one batch is slow. A producer or encoder error reaches the writer in
/// order as an `Err` item, so a sink never finishes a partial output; the first root-cause error is
/// returned.
pub fn run<I: Send + 'static, O: Send + 'static, S: Send, W: Send>(
    threads: usize,
    window: Window,
    produce: impl FnOnce(&BatchSender<'_, I, O>) -> TileGenResult<()>,
    init: impl Fn() -> S + Sync,
    encode: impl Fn(&mut S, I) -> TileGenResult<O> + Sync,
    weigh: impl Fn(&O) -> usize + Sync,
    write: impl FnOnce(&mut dyn Iterator<Item = TileGenResult<O>>) -> TileGenResult<W> + Send,
) -> TileGenResult<(W, Vec<S>)> {
    let (work_tx, work_rx) = hotpath::channel!(flume::bounded::<Work<I, O>>(threads.max(1)), label = { "encode_work" });
    let (order_tx, order_rx) =
        hotpath::channel!(flume::bounded::<Pending<O>>(window.batches), label = { "encode_order" });
    let unwritten = &Unwritten::default();
    thread::scope(|scope| {
        let encoders: Vec<_> = (0..threads.max(1))
            .map(|i| {
                let (work_rx, init, encode, weigh) = (work_rx.clone(), &init, &encode, &weigh);
                spawn_named(scope, "encoder", i, move || {
                    let mut state = init();
                    for (batch, result_tx) in work_rx {
                        let result = encode(&mut state, batch);
                        let bytes = result.as_ref().map_or(0, weigh);
                        unwritten.add(bytes);
                        let _ = result_tx.send((result, bytes));
                    }
                    state
                })
            })
            .collect();
        drop(work_rx);
        let writer = spawn_named(scope, "writer", 0, move || {
            let _stop = StopOnDrop(unwritten);
            write(&mut order_rx.into_iter().map_while(|pending| {
                let (result, bytes) = hotpath::measure_block!({ "writer_wait_result" }, pending.recv().ok())?;
                unwritten.take(bytes);
                Some(result)
            }))
        });

        let sender = BatchSender {
            work: work_tx,
            order: order_tx,
            unwritten,
            bytes: window.bytes,
        };
        if let Err(err) = produce(&sender)
            && !matches!(err, TileGenError::WriterStopped)
        {
            sender.fail(err);
        }
        drop(sender);
        eprintln!("BENCH peak unwritten bytes {}", PEAK.load(std::sync::atomic::Ordering::Relaxed));
        let written = writer.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic))?;
        let states = encoders
            .into_iter()
            .map(|e| e.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)))
            .collect();
        Ok((written, states))
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;

    fn produce_n(n: usize) -> impl FnOnce(&BatchSender<'_, usize, usize>) -> TileGenResult<()> {
        move |submit| (0..n).try_for_each(|i| submit.send(i))
    }

    /// Later batches finish first, so only the reordering makes the output sorted.
    fn slow_early(i: usize) -> usize {
        thread::sleep(Duration::from_micros(((50 - i % 50) * 20) as u64));
        i * 10
    }

    #[test]
    fn writes_in_production_order() {
        let (out, counts) = run(
            4,
            Window {
                batches: 8,
                bytes: usize::MAX,
            },
            produce_n(500),
            || 0,
            |count: &mut usize, i| {
                *count += 1;
                Ok(slow_early(i))
            },
            |_| 0,
            |results| results.collect::<TileGenResult<Vec<_>>>(),
        )
        .unwrap();
        assert_eq!(out, (0..500).map(|i| i * 10).collect::<Vec<_>>());
        assert_eq!(counts.len(), 4);
        assert_eq!(counts.iter().sum::<usize>(), 500);
    }

    #[test]
    fn encoder_error_stops_the_writer_in_order() {
        let encode = |(): &mut (), i| {
            if i == 7 {
                Err(TileGenError::RecordTooLarge(i))
            } else {
                Ok(slow_early(i))
            }
        };
        let mut seen = 0;
        let err = run(
            4,
            Window {
                batches: 8,
                bytes: usize::MAX,
            },
            produce_n(100),
            || (),
            encode,
            |_| 0,
            |results| {
                for result in results {
                    result?;
                    seen += 1;
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert!(matches!(err, TileGenError::RecordTooLarge(7)));
        assert_eq!(seen, 7);
    }

    #[test]
    fn producer_error_reaches_the_writer_after_submitted_batches() {
        let produce = |submit: &BatchSender<'_, usize, usize>| {
            (0..5).try_for_each(|i| submit.send(i))?;
            Err(TileGenError::InvalidTileId(99))
        };
        let mut seen = Vec::new();
        let err = run(
            2,
            Window {
                batches: 3,
                bytes: usize::MAX,
            },
            produce,
            || (),
            |(), i| Ok(i),
            |_| 0,
            |results| {
                for result in results {
                    seen.push(result?);
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert!(matches!(err, TileGenError::InvalidTileId(99)));
        assert_eq!(seen, [0, 1, 2, 3, 4]);
    }

    #[test]
    fn writer_error_stops_the_producer() {
        let sent = AtomicUsize::new(0);
        let produce = |submit: &BatchSender<'_, usize, usize>| {
            (0..1_000_000).try_for_each(|i| {
                sent.fetch_add(1, Ordering::Relaxed);
                submit.send(i)
            })
        };

        let err = run(
            2,
            Window {
                batches: 2,
                bytes: usize::MAX,
            },
            produce,
            || (),
            |(), i| Ok(i),
            |_| 0,
            |results| {
                results.take(3).try_for_each(|r| r.map(drop))?;
                Err::<(), _>(TileGenError::InvalidTileId(1))
            },
        )
        .unwrap_err();

        assert!(matches!(err, TileGenError::InvalidTileId(1)));
        assert!(sent.into_inner() < 1_000_000);
    }

    #[test]
    fn encoded_bytes_waiting_for_the_writer_hold_back_the_producer() {
        let written = AtomicUsize::new(0);
        let lead = AtomicUsize::new(0);
        let produce = |submit: &BatchSender<'_, usize, usize>| {
            (0..500).try_for_each(|i| {
                submit.send(i)?;
                lead.fetch_max(i + 1 - written.load(Ordering::Relaxed), Ordering::Relaxed);
                Ok(())
            })
        };
        run(
            4,
            Window {
                batches: 1000,
                bytes: 10,
            },
            produce,
            || (),
            |(), i| Ok(i),
            |_| 1,
            |results| {
                for (i, result) in results.enumerate() {
                    result?;
                    if i == 0 {
                        thread::sleep(Duration::from_millis(50));
                    }
                    written.fetch_add(1, Ordering::Relaxed);
                }
                Ok(())
            },
        )
        .unwrap();
        assert!(lead.into_inner() <= 10 + 2 * 4 + 2);
    }

    #[test]
    fn writer_error_stops_a_producer_waiting_for_written_bytes() {
        let err = run(
            2,
            Window {
                batches: 1000,
                bytes: 0,
            },
            produce_n(1_000_000),
            || (),
            |(), i| Ok(i),
            |_| 1,
            |results| {
                results.take(3).try_for_each(|r| r.map(drop))?;
                Err::<(), _>(TileGenError::InvalidTileId(1))
            },
        )
        .unwrap_err();
        assert!(matches!(err, TileGenError::InvalidTileId(1)));
    }
}
