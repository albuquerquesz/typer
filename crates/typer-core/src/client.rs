use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use anyhow::{Context, bail};

use crate::paths;
use crate::protocol::{Request, Response, State};

fn connect() -> anyhow::Result<UnixStream> {
    let path = paths::socket();
    UnixStream::connect(&path)
        .with_context(|| format!("typerd is not running ({} unavailable)", path.display()))
}

fn send(stream: &mut UnixStream, req: &Request) -> anyhow::Result<()> {
    let mut line = serde_json::to_vec(req)?;
    line.push(b'\n');
    stream.write_all(&line)?;
    Ok(())
}

/// Send one request and wait for its response.
pub fn request(req: &Request) -> anyhow::Result<Response> {
    let mut stream = connect()?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    send(&mut stream, req)?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    if line.is_empty() {
        bail!("typerd closed the connection");
    }
    match serde_json::from_str(&line)? {
        Response::Error { message } => bail!(message),
        res => Ok(res),
    }
}

/// Like [`request`], but expects a `State` back.
pub fn request_state(req: &Request) -> anyhow::Result<State> {
    match request(req)? {
        Response::State(state) => Ok(state),
        other => bail!("unexpected response: {other:?}"),
    }
}

/// Stream state changes until the connection drops. The current state is delivered first.
pub fn subscribe(mut on_state: impl FnMut(State)) -> anyhow::Result<()> {
    let mut stream = connect()?;
    send(&mut stream, &Request::Subscribe)?;
    for line in BufReader::new(stream).lines() {
        if let Response::State(state) = serde_json::from_str(&line?)? {
            on_state(state);
        }
    }
    Ok(())
}

/// Stream pressed-key labels until the connection drops.
pub fn subscribe_keys(mut on_key: impl FnMut(String)) -> anyhow::Result<()> {
    let mut stream = connect()?;
    send(&mut stream, &Request::SubscribeKeys)?;
    for line in BufReader::new(stream).lines() {
        if let Response::Key { label } = serde_json::from_str(&line?)? {
            on_key(label);
        }
    }
    Ok(())
}
