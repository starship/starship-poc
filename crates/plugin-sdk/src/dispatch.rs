//! Answers daemon requests on behalf of a plugin.
//!
//! `#[export_plugin]` and `#[export_vcs_plugin]` implement [`Methods`] for the
//! plugin and generate a `main` that runs [`serve`] with [`handle_plugin`] or
//! [`handle_vcs_plugin`]. Not part of the public API.

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use serde_json::Value;
use starship_plugin_core::{
    ABI_VERSION, Manifest, Message, PluginKind, RenderContext, Request, Response,
};

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

/// The renders a plugin has begun and not yet ended, by render ID.
#[derive(Default)]
pub struct Renders(Mutex<HashMap<u64, Arc<Ctx>>>);

impl Renders {
    /// Remembers a render's context until [`end`](Self::end).
    fn begin(&self, render_id: u64, context: RenderContext) -> Arc<Ctx> {
        let ctx = Arc::new(Ctx::new(context));
        self.lock().insert(render_id, Arc::clone(&ctx));
        ctx
    }

    /// Runs a method in a begun render. A render the plugin doesn't know
    /// answers `Null`.
    fn call(&self, render_id: u64, method: impl FnOnce(&Ctx) -> Value) -> Response {
        let ctx = self.lock().get(&render_id).cloned();
        let value = if let Some(ctx) = ctx {
            method(&ctx)
        } else {
            eprintln!("call for unknown render {render_id}");
            Value::Null
        };
        Response::Call(value)
    }

    fn end(&self, render_id: u64) {
        self.lock().remove(&render_id);
    }

    /// A poisoned lock only means another request panicked; the map itself
    /// is still usable.
    fn lock(&self) -> MutexGuard<'_, HashMap<u64, Arc<Ctx>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Answers one request for a general plugin. `EndRender` gets no response.
pub fn handle_plugin<P: Plugin + Methods>(
    plugin: &P,
    renders: &Renders,
    request: Request,
) -> Option<Response> {
    Some(match request {
        Request::Describe { cache_dir } => {
            describe(cache_dir, P::NAME, PluginKind::General, &[], P::METHODS)
        }
        Request::BeginRender { render_id, context } => {
            let ctx = renders.begin(render_id, context);
            Response::BeginRender {
                applicable: plugin.is_applicable(&ctx),
                depth: None,
            }
        }
        Request::Call { render_id, method } => {
            renders.call(render_id, |ctx| plugin.call(&method, ctx))
        }
        Request::EndRender { render_id } => {
            renders.end(render_id);
            return None;
        }
    })
}

/// Answers one request for a VCS plugin. Applicability is derived from
/// `detect_depth`, and `root` and `branch` route to the trait methods.
pub fn handle_vcs_plugin<P: VcsPlugin + Methods>(
    plugin: &P,
    renders: &Renders,
    request: Request,
) -> Option<Response> {
    Some(match request {
        Request::Describe { cache_dir } => {
            let methods = [&["root", "branch"], P::METHODS].concat();
            describe(cache_dir, P::NAME, PluginKind::Vcs, P::SHADOWS, &methods)
        }
        Request::BeginRender { render_id, context } => {
            let ctx = renders.begin(render_id, context);
            let depth = plugin.detect_depth(&ctx);
            Response::BeginRender {
                applicable: depth.is_some(),
                depth,
            }
        }
        Request::Call { render_id, method } => {
            renders.call(render_id, |ctx| match method.as_str() {
                "root" => to_value(plugin.root(ctx)),
                "branch" => to_value(plugin.branch(ctx)),
                _ => plugin.call(&method, ctx),
            })
        }
        Request::EndRender { render_id } => {
            renders.end(render_id);
            return None;
        }
    })
}

/// Answers requests from stdin on stdout until the daemon closes stdin.
/// Called by the generated `main`.
pub fn serve(mut handle: impl FnMut(Request) -> Option<Response>) {
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
        let Some(body) = handle(request.body) else {
            continue;
        };
        let response = Message {
            id: request.id,
            body,
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
