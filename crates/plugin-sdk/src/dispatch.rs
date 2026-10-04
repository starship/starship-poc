//! Answers daemon requests on behalf of a plugin.
//!
//! `#[export_plugin]` and `#[export_vcs_plugin]` implement [`Methods`] for the
//! plugin and generate a `main` that runs [`serve`] with [`handle_plugin`] or
//! [`handle_vcs_plugin`]. Not part of the public API.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;
use starship_plugin_core::{ABI_VERSION, Manifest, Message, PluginKind, Request, Response};

use crate::{Ctx, Plugin, VcsPlugin, exec_cache};

/// The plugin-specific methods declared in an `#[export_plugin]` or
/// `#[export_vcs_plugin]` impl block.
pub trait Methods {
    const METHODS: &'static [&'static str];

    /// Calls the named method. Returns `Null` for unknown names.
    fn call(&self, method: &str, ctx: &Ctx) -> Value;
}

/// Converts a method's return value for the daemon. Values that fail to
/// serialize become `Null`.
pub fn to_value<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// Answers one request for a general plugin.
pub fn handle_plugin<P: Plugin + Methods>(plugin: &P, request: Request) -> Response {
    match request {
        Request::Describe { cache_dir } => {
            describe(cache_dir, P::NAME, PluginKind::General, &[], P::METHODS)
        }
        Request::IsApplicable { context } => {
            Response::IsApplicable(plugin.is_applicable(&Ctx::new(context)))
        }
        Request::DetectDepth { .. } => Response::DetectDepth(None),
        Request::Call { method, context } => {
            Response::Call(plugin.call(&method, &Ctx::new(context)))
        }
    }
}

/// Answers one request for a VCS plugin. Applicability is derived from
/// `detect_depth`, and `root` and `branch` route to the trait methods.
pub fn handle_vcs_plugin<P: VcsPlugin + Methods>(plugin: &P, request: Request) -> Response {
    match request {
        Request::Describe { cache_dir } => {
            let methods = [&["root", "branch"], P::METHODS].concat();
            describe(cache_dir, P::NAME, PluginKind::Vcs, P::SHADOWS, &methods)
        }
        Request::IsApplicable { context } => {
            Response::IsApplicable(plugin.detect_depth(&Ctx::new(context)).is_some())
        }
        Request::DetectDepth { context } => {
            Response::DetectDepth(plugin.detect_depth(&Ctx::new(context)))
        }
        Request::Call { method, context } => {
            let ctx = Ctx::new(context);
            Response::Call(match method.as_str() {
                "root" => to_value(plugin.root(&ctx)),
                "branch" => to_value(plugin.branch(&ctx)),
                _ => plugin.call(&method, &ctx),
            })
        }
    }
}

/// Answers requests from stdin on stdout until the daemon closes stdin.
/// Called by the generated `main`.
pub fn serve(mut handle: impl FnMut(Request) -> Response) {
    let mut stdout = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else {
            break;
        };
        let request: Message<Request> = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(error) => {
                eprintln!("ignoring malformed request: {error}");
                continue;
            }
        };
        let response = Message {
            id: request.id,
            body: handle(request.body),
        };
        if write_line(&mut stdout, &response).is_err() {
            break;
        }
    }
}

fn write_line(out: &mut impl Write, message: &Message<Response>) -> io::Result<()> {
    serde_json::to_writer(&mut *out, message)?;
    out.write_all(b"\n")?;
    out.flush()
}

/// Starts the exec cache under the daemon's cache directory and reports the
/// plugin's manifest.
fn describe(
    cache_dir: Option<PathBuf>,
    name: &str,
    kind: PluginKind,
    shadows: &[&str],
    methods: &[&str],
) -> Response {
    if let Some(dir) = cache_dir {
        exec_cache::init(dir.join(name).join("exec_cache.json"));
    }
    let owned = |names: &[&str]| names.iter().map(ToString::to_string).collect();
    Response::Describe(Manifest {
        abi_version: ABI_VERSION,
        name: name.to_string(),
        kind,
        shadows: owned(shadows),
        methods: owned(methods),
    })
}
