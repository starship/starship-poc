//! Answers daemon requests on behalf of a plugin.
//!
//! `#[export_plugin]` and `#[export_vcs_plugin]` implement [`Methods`] for the
//! plugin and generate a `_plugin_handle` export that forwards to
//! [`handle_plugin`] or [`handle_vcs_plugin`]. Not part of the public API.

use serde::Serialize;
use serde_json::Value;
use starship_plugin_core::{ABI_VERSION, Manifest, PluginKind, Request, Response};

use crate::{Ctx, Plugin, VcsPlugin};

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
        Request::Describe => {
            Response::Describe(manifest(P::NAME, PluginKind::General, &[], P::METHODS))
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
        Request::Describe => {
            let methods = [&["root", "branch"], P::METHODS].concat();
            Response::Describe(manifest(P::NAME, PluginKind::Vcs, P::SHADOWS, &methods))
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

/// Decodes a request from guest memory, answers it with `handle`, and writes
/// the response back. Called by the generated `_plugin_handle` export.
pub fn handle_packed(packed: u64, handle: impl FnOnce(Request) -> Response) -> u64 {
    // SAFETY: the host writes `packed` with `alloc` before calling `_plugin_handle`.
    let request: Request = unsafe { starship_plugin_core::read_msg(packed) };
    starship_plugin_core::write_msg(&handle(request))
}

fn manifest(name: &str, kind: PluginKind, shadows: &[&str], methods: &[&str]) -> Manifest {
    let owned = |names: &[&str]| names.iter().map(ToString::to_string).collect();
    Manifest {
        abi_version: ABI_VERSION,
        name: name.to_string(),
        kind,
        shadows: owned(shadows),
        methods: owned(methods),
    }
}
