use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ImplItem, ItemImpl, Type};

/// Exports a plugin impl block for WASM.
///
/// Requires `const NAME: &str` in the impl block.
#[proc_macro_attribute]
pub fn export_plugin(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let impl_block = parse_macro_input!(item as ItemImpl);

    let struct_type = match &*impl_block.self_ty {
        Type::Path(type_path) => type_path.path.segments.last().unwrap().ident.clone(),
        _ => panic!("expected struct type"),
    };

    let has_name_const = impl_block.items.iter().any(|item| {
        if let ImplItem::Const(c) = item {
            c.ident == "NAME"
        } else {
            false
        }
    });

    if !has_name_const {
        return syn::Error::new_spanned(
            &impl_block.self_ty,
            "#[export_plugin] requires `const NAME: &str` in the impl block",
        )
        .to_compile_error()
        .into();
    }

    let pub_methods: Vec<_> = impl_block
        .items
        .iter()
        .filter_map(|item| {
            if let ImplItem::Fn(method) = item {
                if matches!(method.vis, syn::Visibility::Public(_)) {
                    return Some(method.sig.ident.clone());
                }
            }
            None
        })
        .collect();

    let method_names: Vec<String> = pub_methods.iter().map(|m| m.to_string()).collect();
    let method_count = method_names.len();

    let match_arms = pub_methods.iter().map(|method| {
        let name = method.to_string();
        quote! {
            #name => starship_plugin_sdk::serde_json::to_value(instance.#method()).unwrap_or(starship_plugin_sdk::serde_json::Value::Null)
        }
    });

    let instance_storage = quote! {
        static mut INSTANCES: Option<std::collections::HashMap<u32, #struct_type>> = None;
        static mut NEXT_HANDLE: u32 = 1;

        fn instances() -> &'static mut std::collections::HashMap<u32, #struct_type> {
            unsafe {
                INSTANCES.get_or_insert_with(std::collections::HashMap::new)
            }
        }
    };

    let name_export = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn _plugin_name() -> u64 {
            starship_plugin_sdk::write_msg(&#struct_type::NAME)
        }
    };

    let version_export = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn _plugin_version() -> u64 {
            starship_plugin_sdk::write_msg(&env!("CARGO_PKG_VERSION"))
        }
    };

    let methods_export = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn _plugin_methods() -> u64 {
            let methods: [&str; #method_count] = [#(#method_names),*];
            starship_plugin_sdk::write_msg(&methods)
        }
    };

    let new_export = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn _plugin_new() -> u32 {
            let handle = unsafe { NEXT_HANDLE };
            unsafe { NEXT_HANDLE += 1; }
            instances().insert(handle, #struct_type::default());
            handle
        }
    };

    let drop_export = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn _plugin_drop(handle: u32) {
            instances().remove(&handle);
        }
    };

    let call_export = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn _plugin_call(handle: u32, method_packed: u64) -> u64 {
            let method: String = unsafe { starship_plugin_sdk::read_msg(method_packed) };
            let Some(instance) = instances().get(&handle) else {
                return starship_plugin_sdk::write_msg(&starship_plugin_sdk::serde_json::Value::Null);
            };
            let result: starship_plugin_sdk::serde_json::Value = match method.as_str() {
                #(#match_arms,)*
                _ => starship_plugin_sdk::serde_json::Value::Null,
            };
            starship_plugin_sdk::write_msg(&result)
        }
    };

    TokenStream::from(quote! {
        #impl_block
        #instance_storage
        #name_export
        #version_export
        #methods_export
        #new_export
        #drop_export
        #call_export
    })
}
