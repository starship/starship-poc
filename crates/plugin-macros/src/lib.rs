use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{ImplItem, ItemImpl, Type, parse_macro_input};

/// Exports a plugin impl block for WASM.
///
/// The struct must implement `starship_plugin_sdk::Plugin`.
/// Public methods in this impl block become callable from the config. Each
/// takes `&self` and may also take `ctx: &Ctx`.
#[proc_macro_attribute]
pub fn export_plugin(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let impl_block = parse_macro_input!(item as ItemImpl);
    export(impl_block, &quote!(handle_plugin))
}

/// Exports a VCS plugin impl block for WASM.
///
/// The struct must implement `starship_plugin_sdk::VcsPlugin`. Applicability
/// is derived from `detect_depth().is_some()`, so authors don't write a
/// generic gate predicate. `root` and `branch` route to the trait methods;
/// public methods on this impl block (e.g. `jj.change_id`) are callable too.
#[proc_macro_attribute]
pub fn export_vcs_plugin(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let impl_block = parse_macro_input!(item as ItemImpl);
    export(impl_block, &quote!(handle_vcs_plugin))
}

/// Generates the method table and the single `_plugin_handle` export. The
/// request handling itself lives in `starship_plugin_sdk::dispatch`.
fn export(mut impl_block: ItemImpl, handler: &proc_macro2::TokenStream) -> TokenStream {
    // Exported methods must take `&self` so the dispatcher can call them,
    // even when the plugin is stateless.
    impl_block
        .attrs
        .push(syn::parse_quote!(#[allow(clippy::unused_self)]));

    let struct_type = struct_name(&impl_block);
    let methods = public_methods(&impl_block);
    let names: Vec<String> = methods.iter().map(|m| m.name.to_string()).collect();
    let calls = methods.iter().map(|Method { name, takes_ctx }| {
        if *takes_ctx {
            quote!(self.#name(ctx))
        } else {
            quote!(self.#name())
        }
    });

    TokenStream::from(quote! {
        #impl_block

        impl ::starship_plugin_sdk::dispatch::Methods for #struct_type {
            const METHODS: &'static [&'static str] = &[#(#names),*];

            fn call(
                &self,
                method: &str,
                ctx: &::starship_plugin_sdk::Ctx,
            ) -> ::starship_plugin_sdk::serde_json::Value {
                match method {
                    #(#names => ::starship_plugin_sdk::dispatch::to_value(#calls),)*
                    _ => ::starship_plugin_sdk::serde_json::Value::Null,
                }
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn _plugin_handle(packed: u64) -> u64 {
            ::std::thread_local! {
                static PLUGIN: #struct_type = <#struct_type as ::core::default::Default>::default();
            }
            PLUGIN.with(|plugin| {
                ::starship_plugin_sdk::dispatch::handle_packed(packed, |request| {
                    ::starship_plugin_sdk::dispatch::#handler(plugin, request)
                })
            })
        }
    })
}

fn struct_name(impl_block: &ItemImpl) -> Ident {
    match &*impl_block.self_ty {
        Type::Path(type_path) => type_path.path.segments.last().unwrap().ident.clone(),
        _ => panic!("expected struct type"),
    }
}

/// A public method in the exported impl block.
struct Method {
    name: Ident,
    /// Whether it takes `ctx: &Ctx` after `&self`.
    takes_ctx: bool,
}

fn public_methods(impl_block: &ItemImpl) -> Vec<Method> {
    impl_block
        .items
        .iter()
        .filter_map(|item| match item {
            ImplItem::Fn(method) if matches!(method.vis, syn::Visibility::Public(_)) => {
                Some(Method {
                    name: method.sig.ident.clone(),
                    takes_ctx: method.sig.inputs.len() > 1,
                })
            }
            _ => None,
        })
        .collect()
}
