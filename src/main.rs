use std::{
    io::{self, Write},
    os::fd::AsRawFd,
    path::PathBuf,
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use ashpd::desktop::{
    PersistMode,
    screencast::{CursorMode, Screencast, SourceType},
};
use clap::Parser;
use futures_util::StreamExt;
use gst::prelude::*;
use gstreamer as gst;
use tempfile::NamedTempFile;
use tokio::signal::unix::{SignalKind, signal};

#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// GIF to write when recording finishes.
    output: PathBuf,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let Args { output } = Args::parse();
    let output = std::path::absolute(output)?;
    let parent = output.parent().context("expected an output file path")?;

    gst::init()?;
    let pipeline = gst::parse::launch(
        "pipewiresrc name=window do-timestamp=true provide-clock=false keepalive-time=1000 \
         ! videorate skip-to-first=true drop-only=true max-rate=10 ! video/x-raw,framerate=10/1 \
         ! videoconvert ! gifenc ! fdsink name=output sync=false",
    )?
    .downcast::<gst::Pipeline>()
    .expect("multiple linked elements form a pipeline");
    let portal = Screencast::new().await?;
    let session = portal.create_session().await?;
    portal
        .select_sources(
            &session,
            CursorMode::Hidden,
            SourceType::Window.into(),
            false,
            None,
            PersistMode::DoNot,
        )
        .await?
        .response()?;
    let selection = portal.start(&session, None).await?.response()?;
    let [window] = selection.streams() else {
        bail!("expected one selected window")
    };
    let remote = portal.open_pipe_wire_remote(&session).await?;
    let source = pipeline.by_name("window").unwrap();
    source.set_property("fd", remote.as_raw_fd());
    source.set_property("path", window.pipe_wire_node_id().to_string());

    let file = NamedTempFile::new_in(parent)
        .with_context(|| format!("creating a temporary recording in {}", parent.display()))?;
    pipeline
        .by_name("output")
        .unwrap()
        .set_property("fd", file.as_raw_fd());
    let result = record(&pipeline).await;
    pipeline.set_state(gst::State::Null)?;
    result?;
    ensure!(file.as_file().metadata()?.len() > 0, "recording is empty");
    file.persist(&output)
        .with_context(|| format!("saving {}", output.display()))?;
    Ok(())
}

async fn record(pipeline: &gst::Pipeline) -> Result<()> {
    let mut interrupt = signal(SignalKind::interrupt())?;
    let bus = pipeline.bus().unwrap();
    let mut messages = bus.stream();
    pipeline.set_state(gst::State::Playing)?;
    let mut ready = false;
    let mut finish_deadline = None;
    loop {
        tokio::select! {
            _ = interrupt.recv() => {
                ensure!(ready && finish_deadline.is_none(), "recording interrupted");
                ensure!(pipeline.send_event(gst::event::Eos::new()), "could not finish recording");
                finish_deadline = Some(tokio::time::Instant::now() + Duration::from_secs(10));
            }
            message = messages.next() => {
                let message = message.context("recording bus closed")?;
                match message.view() {
                    gst::MessageView::StateChanged(state) if !ready &&
                        state.src() == Some(pipeline.upcast_ref()) && state.current() == gst::State::Playing => {
                        writeln!(io::stdout().lock(), "ready")?;
                        ready = true;
                    }
                    gst::MessageView::Eos(_) => {
                        ensure!(ready, "window stream ended before recording started");
                        return Ok(());
                    }
                    gst::MessageView::Error(err) => bail!("{}", err.error()),
                    _ => {}
                }
            }
            _ = async { tokio::time::sleep_until(finish_deadline.unwrap()).await }, if finish_deadline.is_some() => {
                bail!("timed out finishing recording");
            }
        }
    }
}
