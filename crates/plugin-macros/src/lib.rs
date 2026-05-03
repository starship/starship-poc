use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{parse_macro_input, ImplItem, ItemImpl, Type};

fn parse_impl_block(impl_block: &ItemImpl) -> (Ident, Vec<Ident>) {
    let struct_type = match &*impl_block.self_ty {
        Type::Path(type_path) => type_path.path.segments.last().unwrap().ident.clone(),
        _ => panic!("expected struct type"),
    };

    let pub_methods = impl_block
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

    (struct_type, pub_methods)
}

/// Exports a plugin impl block for WASM.
///
/// The struct must implement `starship_plugin_sdk::Plugin`.
/// Public methods in this impl block become callable via `_plugin_call`.
#[proc_macro_attribute]
pub fn export_plugin(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let impl_block = parse_macro_input!(item as ItemImpl);
    let (struct_type, pub_methods) = parse_impl_block(&impl_block);

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

    let kind_export = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn _plugin_kind() -> u64 {
            starship_plugin_sdk::write_msg(&"general")
        }
    };

    let shadows_export = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn _plugin_shadows() -> u64 {
            starship_plugin_sdk::write_msg(&Vec::<&str>::new())
        }
    };

    let is_applicable_export = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn _plugin_is_applicable(handle: u32) -> u64 {
            let Some(instance) = instances().get(&handle) else {
                return starship_plugin_sdk::write_msg(&false);
            };
            starship_plugin_sdk::write_msg(&starship_plugin_sdk::Plugin::is_applicable(instance))
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
        #kind_export
        #shadows_export
        #is_applicable_export
        #new_export
        #drop_export
        #call_export
    })
}

/// Exports a VCS plugin impl block for WASM.
///
/// The struct must implement `starship_plugin_sdk::VcsPlugin`. The macro
/// derives `_plugin_is_applicable` from `detect_depth().is_some()`, so authors
/// don't write a generic gate predicate. `_plugin_call` routes `"root"` and
/// `"branch"` to the trait methods, plus any public methods declared on this
/// inherent impl block (e.g. `jj.change_id`).
#[proc_macro_attribute]
pub fn export_vcs_plugin(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let impl_block = parse_macro_input!(item as ItemImpl);
    let (struct_type, pub_methods) = parse_impl_block(&impl_block);

    let inherent_arms = pub_methods.iter().map(|method| {
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
            starship_plugin_sdk::write_msg(&<#struct_type as starship_plugin_sdk::VcsPlugin>::NAME)
        }
    };

    let kind_export = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn _plugin_kind() -> u64 {
            starship_plugin_sdk::write_msg(&"vcs")
        }
    };

    let shadows_export = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn _plugin_shadows() -> u64 {
            starship_plugin_sdk::write_msg(&<#struct_type as starship_plugin_sdk::VcsPlugin>::SHADOWS)
        }
    };

    let is_applicable_export = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn _plugin_is_applicable(handle: u32) -> u64 {
            let Some(instance) = instances().get(&handle) else {
                return starship_plugin_sdk::write_msg(&false);
            };
            let applicable = starship_plugin_sdk::VcsPlugin::detect_depth(instance).is_some();
            starship_plugin_sdk::write_msg(&applicable)
        }
    };

    let detect_depth_export = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn _plugin_detect_depth(handle: u32) -> u64 {
            let Some(instance) = instances().get(&handle) else {
                return starship_plugin_sdk::write_msg(&Option::<u32>::None);
            };
            starship_plugin_sdk::write_msg(&starship_plugin_sdk::VcsPlugin::detect_depth(instance))
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
                "root" => starship_plugin_sdk::serde_json::to_value(starship_plugin_sdk::VcsPlugin::root(instance)).unwrap_or(starship_plugin_sdk::serde_json::Value::Null),
                "branch" => starship_plugin_sdk::serde_json::to_value(starship_plugin_sdk::VcsPlugin::branch(instance)).unwrap_or(starship_plugin_sdk::serde_json::Value::Null),
                #(#inherent_arms,)*
                _ => starship_plugin_sdk::serde_json::Value::Null,
            };
            starship_plugin_sdk::write_msg(&result)
        }
    };

    TokenStream::from(quote! {
        #impl_block
        #instance_storage
        #name_export
        #kind_export
        #shadows_export
        #is_applicable_export
        #detect_depth_export
        #new_export
        #drop_export
        #call_export
    })
}
