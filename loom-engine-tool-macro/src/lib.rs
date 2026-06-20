use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{
    parse_macro_input, ItemFn, FnArg, Pat, ReturnType, 
    Attribute, LitStr, Ident, Token, parse::Parse, Type, Expr, Lit
};
use std::collections::HashMap;

struct ToolAttrs {
    result_doc: String,
    params: Vec<ParamAttr>,
    embedding_doc: Option<String>,
}

struct ParamAttr {
    name: Ident,
    description: String,
    default: Option<String>,
}

impl Parse for ToolAttrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut result_doc = String::new();
        let mut params = Vec::new();
        let mut embedding_doc = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;

            if key == "result_doc" {
                input.parse::<Token![=]>()?;
                let lit: LitStr = input.parse()?;
                result_doc = lit.value();
            } else if key == "param" {
                input.parse::<Token![=]>()?;
                let content;
                syn::parenthesized!(content in input);

                let name: Ident = content.parse()?;
                content.parse::<Token![,]>()?;
                let desc_lit: LitStr = content.parse()?;
                let description = desc_lit.value();

                let mut default = None;
                if content.peek(Token![,]) {
                    content.parse::<Token![,]>()?;
                    let default_key: Ident = content.parse()?;
                    if default_key == "default" {
                        content.parse::<Token![=]>()?;
                        let default_lit: LitStr = content.parse()?;
                        default = Some(default_lit.value());
                    }
                }

                params.push(ParamAttr { name, description, default });
            } else if key == "embedding_doc" {
                input.parse::<Token![=]>()?;
                let lit: LitStr = input.parse()?;
                embedding_doc = Some(lit.value());
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(ToolAttrs { result_doc, params, embedding_doc })
    }
}

struct Param {
    name: Ident,
    ty: Box<Type>,
    description: String,
    default: Option<String>,
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
            }
        })
        .collect()
}

#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = parse_macro_input!(attr as ToolAttrs);
    let result_template = attrs.result_doc;
    
    if result_template.is_empty() {
        return syn::Error::new_spanned(
            proc_macro2::TokenStream::new(),
            "tool macro requires result_doc attribute, e.g., #[tool(result_doc = \"...\")]"
        ).to_compile_error().into();
    }
    
    let input = parse_macro_input!(item as ItemFn);
    let fn_name = &input.sig.ident;
    let fn_name_str = fn_name.to_string();
    let fn_description = extract_doc_comment(&input.attrs);
    
    // 结构体使用大驼峰命名
    let struct_name = format_ident!("{}", to_pascal_case(&fn_name_str));
    let params_struct = format_ident!("{}Params", struct_name);
    
    let param_attr_map: HashMap<String, (String, Option<String>)> = attrs.params
        .into_iter()
        .map(|p| (p.name.to_string(), (p.description, p.default)))
        .collect();
    
    let impl_fn_name = format_ident!("__{}_impl", fn_name);
    
    // 创建 impl_fn，保持原始签名不变
    let mut impl_fn = input.clone();
    impl_fn.sig.ident = impl_fn_name.clone();
    
    let mut params = Vec::new();
    for (idx, arg) in input.sig.inputs.iter().enumerate() {
        match parse_param(arg, idx, &param_attr_map) {
            Ok(param) => params.push(param),
            Err(e) => return e.to_compile_error().into(),
        }
    }
    
    let (_ok_type, is_result) = match &input.sig.output {
        ReturnType::Default => (quote! { () }, false),
        ReturnType::Type(_, ty) => {
            if let Type::Path(type_path) = ty.as_ref() {
                let path = &type_path.path;
                if let Some(seg) = path.segments.last() {
                    if seg.ident == "Result" {
                        if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                            if let Some(syn::GenericArgument::Type(ok_ty)) = args.args.first() {
                                (quote! { #ok_ty }, true)
                            } else {
                                (quote! { #ty }, false)
                            }
                        } else {
                            (quote! { #ty }, false)
                        }
                    } else {
                        (quote! { #ty }, false)
                    }
                } else {
                    (quote! { #ty }, false)
                }
            } else {
                (quote! { #ty }, false)
            }
        }
    };
    
    let param_names: Vec<_> = params.iter().map(|p| &p.name).collect();
    
    let param_fields = params.iter().map(|p| {
        let name = &p.name;
        let ty = &p.ty;
        let desc = &p.description;
        
        let default_attr = p.default.as_ref().map(|_| {
            quote! { #[serde(default)] }
        });
        
        quote! {
            #[schemars(description = #desc)]
            #default_attr
            pub #name: #ty
        }
    });
    
    let default_impl = {
        let default_fields = params.iter().map(|p| {
            let name = &p.name;
            if let Some(ref val) = p.default {
                quote! { #name: #val.parse().unwrap_or_default() }
            } else {
                quote! { #name: Default::default() }
            }
        });
        
        quote! {
            impl Default for #params_struct {
                fn default() -> Self {
                    Self {
                        #(#default_fields,)*
                    }
                }
            }
        }
    };
    
    let required_params: Vec<_> = params.iter()
        .filter(|p| p.default.is_none())
        .map(|p| {
            let name_str = p.name.to_string();
            quote! { #name_str }
        })
        .collect();
    
    let execute_call = if is_result {
        quote! {
            // 先克隆参数以便后续模板使用
            #(let #param_names = params.#param_names.clone();)*
            // 调用 impl_fn，传递克隆的参数
            let result = #impl_fn_name(#(#param_names.clone()),*)
                .map_err(|e| crate::llm::tool::ToolError::Execution(e.to_string()))?;
            // 创建一个包含返回值和所有参数的对象，以便模板可以使用任意变量
            let mut ctx = serde_json::Map::new();
            ctx.insert("value".to_string(), serde_json::to_value(&result)
                .map_err(|e| crate::llm::tool::ToolError::Serialization(e))?);
            #(ctx.insert(stringify!(#param_names).to_string(), serde_json::to_value(&#param_names)
                .map_err(|e| crate::llm::tool::ToolError::Serialization(e))?);)*
            let ctx_value = serde_json::Value::Object(ctx);
            let description = crate::llm::tool::format_template(&ctx_value, #result_template);
            Ok(crate::llm::tool::ToolOutput {
                tool_name: #fn_name_str.to_string(),
                value: serde_json::to_value(&result)
                    .map_err(|e| crate::llm::tool::ToolError::Serialization(e))?,
                description,
            })
        }
    } else {
        quote! {
            // 先克隆参数以便后续模板使用
            #(let #param_names = params.#param_names.clone();)*
            // 调用 impl_fn，传递克隆的参数
            let result = #impl_fn_name(#(#param_names.clone()),*);
            // 创建一个包含返回值和所有参数的对象，以便模板可以使用任意变量
            let mut ctx = serde_json::Map::new();
            ctx.insert("value".to_string(), serde_json::to_value(&result)
                .map_err(|e| crate::llm::tool::ToolError::Serialization(e))?);
            #(ctx.insert(stringify!(#param_names).to_string(), serde_json::to_value(&#param_names)
                .map_err(|e| crate::llm::tool::ToolError::Serialization(e))?);)*
            let ctx_value = serde_json::Value::Object(ctx);
            let description = crate::llm::tool::format_template(&ctx_value, #result_template);
            Ok(crate::llm::tool::ToolOutput {
                tool_name: #fn_name_str.to_string(),
                value: serde_json::to_value(&result)
                    .map_err(|e| crate::llm::tool::ToolError::Serialization(e))?,
                description,
            })
        }
    };

    let (rig_impl, rig_registry) = if let Some(doc) = attrs.embedding_doc {
        let rig_registry_static = format_ident!("__rig_tool_registry_{}", fn_name_str);
        (
            Some(quote! {
                impl rig::tool::Tool for #struct_name {
                    const NAME: &'static str = #fn_name_str;
                    type Error = crate::llm::tool::ToolError;
                    type Args = serde_json::Value;
                    type Output = crate::llm::tool::ToolOutput;

                    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
                        let def = Self::definition();
                        rig::completion::ToolDefinition {
                            name: def.function.name.to_string(),
                            description: def.function.description.to_string(),
                            parameters: def.function.parameters,
                        }
                    }

                    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
                        crate::llm::tool::Tool::execute(self, args)
                    }
                }

                impl rig::tool::ToolEmbedding for #struct_name {
                    type InitError = crate::llm::tool::ToolError;
                    type Context = ();
                    type State = ();

                    fn init(_state: Self::State, _context: Self::Context) -> Result<Self, Self::InitError> {
                        Ok(#struct_name)
                    }

                    fn embedding_docs(&self) -> Vec<String> {
                        vec![#doc.to_string()]
                    }

                    fn context(&self) -> Self::Context {}
                }
            }),
            Some(quote! {
                #[::linkme::distributed_slice(crate::llm::tool::RIG_TOOLS)]
                static #rig_registry_static: fn() -> ::std::boxed::Box<dyn ::rig::tool::ToolDyn> = || {
                    ::std::boxed::Box::new(#struct_name)
                };
            })
        )
    } else {
        (None, None)
    };

    let expanded = quote! {
        #impl_fn

        #[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
        struct #params_struct {
            #(#param_fields,)*
        }

        #default_impl

        pub struct #struct_name;

        impl #struct_name {
            pub fn definition() -> crate::llm::tool::ToolDefinition {
                use schemars::JsonSchema;
                let schema = schemars::schema_for!(#params_struct);
                let mut schema_value = serde_json::to_value(schema).unwrap_or_default();
                if let Some(obj) = schema_value.as_object_mut() {
                    let required: Vec<String> = vec![#(#required_params.to_string()),*];
                    if !required.is_empty() {
                        obj.insert("required".to_string(),
                            serde_json::to_value(&required).unwrap());
                    }
                }
                crate::llm::tool::ToolDefinition {
                    r#type: "function",
                    function: crate::llm::tool::FunctionDefinition {
                        name: #fn_name_str,
                        description: #fn_description,
                        parameters: schema_value,
                        result_description: Some(#result_template),
                    },
                }
            }
        }

        impl crate::llm::tool::Tool for #struct_name {
            fn name(&self) -> &'static str { #fn_name_str }
            fn description(&self) -> &'static str { #fn_description }
            fn parameters_schema(&self) -> serde_json::Value {
                Self::definition().function.parameters
            }
            fn result_description(&self) -> Option<&'static str> {
                Some(#result_template)
            }
            fn execute(
                &self,
                args: serde_json::Value
            ) -> Result<crate::llm::tool::ToolOutput, crate::llm::tool::ToolError> {
                let params: #params_struct = serde_json::from_value(args)
                    .map_err(|e| crate::llm::tool::ToolError::InvalidParams(e.to_string()))?;
                #execute_call
            }
        }

        #rig_impl
        #rig_registry
    };

    expanded.into()
}

fn parse_param(
    arg: &FnArg, 
    idx: usize, 
    attr_map: &HashMap<String, (String, Option<String>)>
) -> Result<Param, syn::Error> {
    match arg {
        FnArg::Typed(pat_type) => {
            let name = match pat_type.pat.as_ref() {
                Pat::Ident(id) => id.ident.clone(),
                _ => format_ident!("arg{}", idx),
            };
            
            let name_str = name.to_string();
            let (description, default) = attr_map
                .get(&name_str)
                .cloned()
                .unwrap_or_else(|| (format!("Parameter {}", name_str), None));
            
            Ok(Param { 
                name, 
                ty: pat_type.ty.clone(), 
                description,
                default,
            })
        }
        _ => Err(syn::Error::new_spanned(arg, "self parameters are not supported")),
    }
}

fn extract_doc_comment(attrs: &[Attribute]) -> String {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .filter_map(|a| {
            let meta = a.meta.require_name_value().ok()?;
            match &meta.value {
                Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }) => Some(s.value()),
                _ => None,
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}