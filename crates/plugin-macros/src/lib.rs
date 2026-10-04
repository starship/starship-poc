use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{ImplItem, ItemImpl, Type, parse_macro_input};

/// Exports a plugin impl block for WASM.
///
/// The struct must implement `starship_plugin_sdk::Plugin`.
/// Public methods in this impl block become callable from the config.
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
    let names: Vec<String> = methods.iter().map(ToString::to_string).collect();

    TokenStream::from(quote! {
        #impl_block

        impl ::starship_plugin_sdk::dispatch::Methods for #struct_type {
            const METHODS: &'static [&'static str] = &[#(#names),*];

            fn call(&self, method: &str) -> ::starship_plugin_sdk::serde_json::Value {
                match method {
                    #(#names => ::starship_plugin_sdk::dispatch::to_value(self.#methods()),)*
                    _ => ::starship_plugin_sdk::serde_json::Value::Null,
                }
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn _plugin_handle(packed: u64) -> u64 {
            ::std::thread_local! {
                static PLUGIN: #struct_type = <#struct_type as ::core::default::Default>::default();
            }
            PLUGIN.with(|plugin| ::starship_plugin_sdk::dispatch::#handler(plugin, packed))
        }
    })
}

fn struct_name(impl_block: &ItemImpl) -> Ident {
    match &*impl_block.self_ty {
        Type::Path(type_path) => type_path.path.segments.last().unwrap().ident.clone(),
        _ => panic!("expected struct type"),
    }
}

fn public_methods(impl_block: &ItemImpl) -> Vec<Ident> {
    impl_block
        .items
        .iter()
        .filter_map(|item| match item {
            ImplItem::Fn(method) if matches!(method.vis, syn::Visibility::Public(_)) => {
                Some(method.sig.ident.clone())
            }
            _ => None,
        })
        .collect()
}
