//! Thin cancellable JSON Lines adapter. Platform resources remain in the engine.
use lumiere_capture_contract::{
    CaptureMode, CaptureOutcome, Failure, Operation, Request, Response,
};
use lumiere_capture_windows::{Cancellation, CaptureEngine};
use std::{io, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    sync::{Mutex, mpsc, watch},
};

struct Active {
    id: String,
    mode: CaptureMode,
    cancel: Cancellation,
    done: watch::Receiver<bool>,
}

pub async fn serve<R, W, E>(input: R, mut output: W, engine: Arc<E>) -> io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + Send,
    E: CaptureEngine,
{
    let active: Arc<Mutex<Option<Active>>> = Arc::new(Mutex::new(None));
    let (send, mut receive) = mpsc::channel::<Response>(32);
    let writer = async move {
        while let Some(response) = receive.recv().await {
            let mut bytes = serde_json::to_vec(&response).map_err(io::Error::other)?;
            bytes.push(b'\n');
            output.write_all(&bytes).await?;
            output.flush().await?;
        }
        Ok::<(), io::Error>(())
    };
    let reader = async {
        let mut lines = BufReader::new(input).lines();
        let mut controls = tokio::task::JoinSet::new();
        let mut captures = tokio::task::JoinSet::new();
        let read_result = loop {
            let next = tokio::select! {
                next = lines.next_line() => next,
                _ = send.closed() => break Ok(()),
            };
            let line = match next {
                Ok(Some(line)) => line,
                Ok(None) => break Ok(()),
                Err(error) => break Err(error),
            };
            let request = match Request::parse(&line) {
                Ok(request) => request,
                Err(response) => {
                    if send.send(response).await.is_err() {
                        break Ok(());
                    }
                    continue;
                }
            };
            let Request {
                version,
                id,
                operation,
            } = request;
            match operation {
                Operation::Capabilities => {
                    if send
                        .send(Response::result(version, id, engine.capabilities(version)))
                        .await
                        .is_err()
                    {
                        break Ok(());
                    }
                }
                Operation::CancelRegion { request_id } => {
                    let done = {
                        let guard = active.lock().await;
                        guard
                            .as_ref()
                            .filter(|capture| {
                                capture.id == request_id && capture.mode == CaptureMode::Region
                            })
                            .map(|capture| {
                                capture.cancel.cancel();
                                capture.done.clone()
                            })
                    };
                    let send = send.clone();
                    controls.spawn(async move {
                        if let Some(mut done) = done {
                            let _ = done.wait_for(|finished| *finished).await;
                        }
                        let _ = send.send(Response::released(version, id)).await;
                    });
                }
                Operation::Capture { mode, params } => {
                    let cancel = Cancellation::default();
                    let (finished, done) = watch::channel(false);
                    {
                        let mut guard = active.lock().await;
                        if guard.is_some() {
                            drop(guard);
                            let result = CaptureOutcome::Failed {
                                failure: Failure::capture("A capture is already in progress."),
                            };
                            if send
                                .send(Response::result(version, id, result))
                                .await
                                .is_err()
                            {
                                break Ok(());
                            }
                            continue;
                        }
                        // Reserve before dispatch so an immediately following cancellation sees it.
                        *guard = Some(Active {
                            id: id.clone(),
                            mode,
                            cancel: cancel.clone(),
                            done: done.clone(),
                        });
                    }
                    let active = active.clone();
                    let engine = engine.clone();
                    let send = send.clone();
                    captures.spawn(async move {
                        let deadline_cancel = cancel.clone();
                        let mut capture = tokio::task::spawn_blocking(move || engine.capture(mode, params, cancel));
                        let result = if mode == CaptureMode::Region {
                            tokio::select! {
                                result = &mut capture => result,
                                _ = tokio::time::sleep(std::time::Duration::from_secs(lumiere_capture_contract::REGION_LEASE_SECONDS)) => {
                                    deadline_cancel.cancel();
                                    // The engine must finish releasing before this request can complete.
                                    capture.await
                                }
                            }
                        } else { capture.await };
                        let result = result.unwrap_or_else(|_| CaptureOutcome::Failed {failure: Failure::unexpected("Native capture task failed.")});
                        *active.lock().await = None;
                        let _ = finished.send(true);
                        let _ = send.send(Response::result(version, id, result)).await;
                    });
                }
            }
            while controls.try_join_next().is_some() {}
            while captures.try_join_next().is_some() {}
        };
        if let Some(capture) = active.lock().await.as_ref() {
            capture.cancel.cancel();
        }
        while captures.join_next().await.is_some() {}
        while controls.join_next().await.is_some() {}
        drop(send);
        read_result
    };
    // A broken output pipe must not drop the reader's native cleanup path.
    let (read_result, write_result) = tokio::join!(reader, writer);
    read_result?;
    write_result?;
    Ok(())
}
