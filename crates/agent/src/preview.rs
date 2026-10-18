//! The existing desktop bridge owns capture/completion; workers own snapshots only.
use crate::{Connection, Failure, PendingRequest};
use plotium_core::{automation::*, state::PlotiumApp};
use std::thread::JoinHandle;

pub(crate) struct PreviewFlight {
    pub(super) worker: JoinHandle<Result<ToolResult, AutomationError>>,
    pub(super) run: PreparedToolRun,
    pub(super) request: PendingRequest,
}
impl Connection {
    /// Dispatch all requests through one desktop entry point. Existing tools
    /// keep their synchronous contract; image work does not block GUI frames.
    pub fn dispatch_request(
        &mut self,
        app: &mut PlotiumApp,
        request: PendingRequest,
        blocked: bool,
    ) -> Result<(), Failure> {
        if request.is_cancelled() {
            return Ok(());
        }
        if request.name != "plotium_render_preview" {
            let result = self.session.call_when_available(
                app,
                &request.name,
                request.arguments.clone(),
                blocked,
            );
            return respond(request, result);
        }
        if self.preview.is_some() {
            return respond(
                request,
                Err(Failure::new(
                    "busy",
                    "A preview is still rendering; retry after it finishes",
                )),
            );
        }
        let prepared = self
            .session
            .prepare_preview(app, request.arguments.clone(), blocked);
        match prepared {
            Err(error) => respond(request, Err(error)),
            Ok((snapshot, run)) => {
                let cancellation = request.cancellation.clone();
                let active = self.active.clone();
                let worker = std::thread::Builder::new()
                    .name("plotium-preview".into())
                    .spawn(move || {
                        if cancellation.is_cancelled()
                            || !active.load(std::sync::atomic::Ordering::Acquire)
                        {
                            return Err(AutomationError::PreviewCancelled);
                        }
                        snapshot.render()
                    });
                match worker {
                    Ok(worker) => {
                        self.preview = Some(PreviewFlight {
                            worker,
                            run,
                            request,
                        });
                        Ok(())
                    }
                    Err(error) => {
                        let error = AutomationError::Execution(format!(
                            "Could not start preview worker: {error}"
                        ));
                        let failure = Failure::from(&error);
                        let failure = match run.finish(app, &Err(error)) {
                            Ok(()) => failure,
                            Err(error) => error.into(),
                        };
                        respond(request, Err(failure))
                    }
                }
            }
        }
    }
    pub fn poll_preview(&mut self, app: &mut PlotiumApp) -> Result<(), Failure> {
        if !self
            .preview
            .as_ref()
            .is_some_and(|p| p.worker.is_finished())
        {
            return Ok(());
        }
        let Some(flight) = self.preview.take() else {
            return Ok(());
        };
        let cancelled = flight.request.is_cancelled();
        let outcome = match flight.worker.join() {
            Ok(_) if cancelled => Err(AutomationError::PreviewCancelled),
            Ok(outcome) => outcome,
            Err(_) => Err(AutomationError::Execution(
                "preview worker failed; retry or reopen the project".into(),
            )),
        };
        if let Err(error) = flight.run.finish(app, &outcome) {
            return respond(flight.request, Err(error.into()));
        }
        let result = if cancelled {
            Err(Failure::new(
                "request_cancelled",
                "Preview request cancelled",
            ))
        } else {
            outcome.map(|v| serde_json::json!(v)).map_err(Failure::from)
        };
        respond(flight.request, result)
    }
}
fn respond(
    request: PendingRequest,
    result: Result<serde_json::Value, Failure>,
) -> Result<(), Failure> {
    let status = result.as_ref().map(|_| ()).map_err(Clone::clone);
    if request.response.send(result).is_err() {
        // Disconnection is normal; no live document mutation is waiting to commit.
    }
    status
}
