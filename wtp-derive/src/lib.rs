//! Procedural macros for wtp

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Meta};

/// Derive macro to add group information to subcommand enums
///
/// Usage:
/// ```ignore
/// #[derive(GroupedSubcommand, Subcommand)]
/// pub enum Commands {
///     #[group("Workspace Management")]
///     Cd(CdArgs),
///     #[group("Repository Operations")]
///     Import(ImportArgs),
/// }
/// ```
#[proc_macro_derive(GroupedSubcommand, attributes(cmd_group))]
pub fn derive_grouped_subcommand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;
    let data = match &input.data {
        Data::Enum(data) => data,
        _ => {
            return syn::Error::new_spanned(
                input,
                "GroupedSubcommand can only be derived for enums",
            )
            .to_compile_error()
            .into();
        }
    };

    // Parse all variants and extract group info
    let mut entries = Vec::new();
    for variant in &data.variants {
        let variant_name = &variant.ident;
        let variant_name_str = variant_name.to_string().to_lowercase();
        
        // Get the about text from doc comments
        let about = variant
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("doc"))
            .filter_map(|attr| {
                let meta = attr.meta.require_name_value().ok()?;
                let expr = &meta.value;
                let lit: syn::LitStr = match expr {
                    syn::Expr::Lit(expr_lit) => {
                        if let syn::Lit::Str(lit) = &expr_lit.lit {
                            lit.clone()
                        } else {
                            return None;
                        }
                    }
                    _ => return None,
                };
                Some(lit.value().trim().to_string())
            })
            .next()
            .unwrap_or_default();

        // Parse #[cmd_group("...")] attribute
        let group = variant
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("cmd_group"))
            .and_then(|attr| {
                // Try to parse as list: group("...")
                if let Ok(meta_list) = attr.parse_args::<syn::LitStr>() {
                    return Some(meta_list.value());
                }
                None
            })
            .unwrap_or_else(|| "Other".to_string());

        entries.push((variant_name_str, about, group));
    }

    // Generate match arm for group() method
    let group_arms = data.variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let group = variant
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("cmd_group"))
            .and_then(|attr| {
                if let Ok(meta_list) = attr.parse_args::<syn::LitStr>() {
                    return Some(meta_list.value());
                }
                None
            })
            .unwrap_or_else(|| "Other".to_string());

        let pattern = match &variant.fields {
            Fields::Unit => quote! { #name::#variant_name },
            Fields::Named(_) => quote! { #name::#variant_name { .. } },
            Fields::Unnamed(_) => quote! { #name::#variant_name(..) },
        };

        quote! {
            #pattern => #group
        }
    });

    // Collect unique groups in order of appearance
    let mut seen_groups = std::collections::HashSet::new();
    let mut ordered_groups = Vec::new();
    for (_, _, group) in &entries {
        if seen_groups.insert(group.clone()) {
            ordered_groups.push(group.clone());
        }
    }

    // Generate help text printing
    let print_help_body = ordered_groups.iter().map(|group| {
        let group_entries: Vec<_> = entries
            .iter()
            .filter(|(_, _, g)| g == group)
            .collect();
        
        let max_name_len = group_entries
            .iter()
            .map(|(name, _, _)| name.len())
            .max()
            .unwrap_or(0);

        let entry_lines = group_entries.iter().map(|(name, about, _)| {
            let padding = " ".repeat(max_name_len - name.len());
            quote! {
                println!("  {}{}  {}", #name.green(), #padding, #about);
            }
        });

        quote! {
            println!("{}:", #group.green().bold());
            #(#entry_lines)*
            println!();
        }
    });

    let expanded = quote! {
        impl #name {
            /// Get the group name for this subcommand variant
            pub fn group(&self) -> &'static str {
                match self {
                    #(#group_arms,)*
                }
            }

            /// Print custom help with grouped subcommands
            pub fn print_help(app_name: &str, version: &str, about: &str) {
                use colored::Colorize;

                println!("{} {}", app_name.green().bold(), version);
                println!("{}", about);
                println!();
                println!("{}: {} {}", "Usage".green().bold(), app_name.green(), "[OPTIONS] <COMMAND>".cyan());
                println!();
                
                println!("Options:");
                println!("  {}  Enable verbose output", "-v, --verbose".yellow());
                println!("  {}  Print version", "-V, --version".yellow());
                println!("  {}     Print help", "-h, --help".yellow());
                println!();
                
                #(#print_help_body)*
                
                println!("Use {} {} for more information on a specific command.", 
                         format!("{} help", app_name).green(),
                         "<command>".cyan());
            }
        }
    };

    TokenStream::from(expanded)
}
