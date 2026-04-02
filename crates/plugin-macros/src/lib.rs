use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ImplItem, ItemImpl, Type};

/// Exports a plugin impl block for WASM.
///
/// The struct must implement `starship_plugin_sdk::Plugin`.
/// Public methods in this impl block become callable via `_plugin_call`.
#[proc_macro_attribute]
pub fn export_plugin(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let impl_block = parse_macro_input!(item as ItemImpl);

    let struct_type = match &*impl_block.self_ty {
        Type::Path(type_path) => type_path.path.segments.last().unwrap().ident.clone(),
        _ => panic!("expected struct type"),
    };

    let pub_methods: Vec<_> = impl_block
        .items
        .iter()
        .filter_map(|item| {
            if let ImplItem::Fn(method) = item
                && matches!(method.vis, syn::Visibility::Public(_))
            {
                return Some(method.sig.ident.clone());
            }
            None
        })
        .collect();

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
            starship_plugin_sdk::write_msg(&<#struct_type as starship_plugin_sdk::Plugin>::NAME)
        }
    };

    let is_active_export = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn _plugin_is_active(handle: u32) -> u64 {
            let Some(instance) = instances().get(&handle) else {
                return starship_plugin_sdk::write_msg(&false);
            };
            starship_plugin_sdk::write_msg(&starship_plugin_sdk::Plugin::is_active(instance))
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
        #is_active_export
        #new_export
        #drop_export
        #call_export
    })
}
