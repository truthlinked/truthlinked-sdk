//! Procedural macros for the TruthLinked SDK.
//!
//! This crate provides derive macros and attribute macros for smart contract development:
//!
//! - `#[derive(Event)]` - Generates event emission code
//! - `#[derive(BytesCodec)]` - Generates variable-length encoding/decoding
//! - `#[derive(Codec32)]` - Generates 32-byte encoding/decoding
//! - `#[derive(Manifest)]` - Generates storage access manifests
//! - `#[error_code]` - Defines error code constants
//! - `#[require]` - Assertion macro with custom error codes

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::{
    parse::Parse, parse::ParseStream, parse_macro_input, spanned::Spanned, Attribute, Data,
    DeriveInput, Expr, Fields, ItemEnum, ItemFn, LitInt, LitStr, Path, Token,
};

/// Derives the `Event` trait for structured logging.
///
/// # Attributes
///
/// - `#[event(name = "EventName")]` - Sets the event name
/// - `#[event(anonymous)]` - Creates an anonymous event (no signature topic)
/// - `#[topic]` - Marks a field as indexed
/// - `#[event(skip)]` - Excludes a field from the event
///
/// # Example
///
/// ```ignore
/// #[derive(Event)]
/// #[event(name = "Transfer")]
/// struct Transfer {
///     #[topic]
///     from: [u8; 32],
///     #[topic]
///     to: [u8; 32],
///     amount: u128,
/// }
/// ```
#[proc_macro_derive(Event, attributes(event, topic))]
pub fn derive_event(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let options = match parse_event_options(&name.to_string(), &input.attrs) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let fields = match event_fields(&input.data) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let topic_count = fields.iter().filter(|f| f.is_topic && !f.skip).count();
    let max_topics = if options.anonymous { 8 } else { 7 };
    if topic_count > max_topics {
        let message = format!(
            "event declares {} indexed fields; maximum is {} ({})",
            topic_count,
            max_topics,
            if options.anonymous {
                "anonymous event"
            } else {
                "topic0 reserved for signature"
            }
        );
        return syn::Error::new(name.span(), message)
            .to_compile_error()
            .into();
    }

    let mut topic_pushes = Vec::new();
    let mut data_pushes = Vec::new();
    let mut type_names = Vec::new();

    for field in fields {
        if field.skip {
            continue;
        }

        let access = field.access;
        if field.is_topic {
            if let Some(path) = field.with_topic {
                topic_pushes.push(quote! {
                    topics.push(#path(&#access));
                });
            } else {
                topic_pushes.push(quote! {
                    topics.push(::truthlinked_sdk::log::EventTopic::to_topic(&#access));
                });
            }
        } else if let Some(path) = field.with_data {
            data_pushes.push(quote! {
                #path(&#access, &mut encoder);
            });
        } else {
            data_pushes.push(quote! {
                ::truthlinked_sdk::log::EventData::encode_event_data(&#access, &mut encoder);
            });
        }

        let ty_name = field
            .signature_type
            .unwrap_or_else(|| normalize_type_name(&field.ty.to_token_stream().to_string()));
        type_names.push(ty_name);
    }

    let signature = options
        .signature
        .unwrap_or_else(|| format!("{}({})", options.name, type_names.join(",")));
    let event_name = options.name;
    let anonymous = options.anonymous;

    let expanded = quote! {
        impl ::truthlinked_sdk::log::Event for #name {
            fn event_name() -> &'static str {
                #event_name
            }

            fn is_anonymous() -> bool {
                #anonymous
            }

            fn event_signature() -> [u8; 32] {
                ::truthlinked_sdk::log::event_signature(#signature)
            }

            fn event_topics(&self) -> ::truthlinked_sdk::log::__private::Vec<[u8; 32]> {
                let mut topics = ::truthlinked_sdk::log::__private::Vec::new();
                if !Self::is_anonymous() {
                    topics.push(Self::event_signature());
                }
                #(#topic_pushes)*
                topics
            }

            fn event_data(&self) -> ::truthlinked_sdk::log::__private::Vec<u8> {
                let mut encoder = ::truthlinked_sdk::codec::Encoder::new();
                #(#data_pushes)*
                encoder.into_vec()
            }
        }
    };

    TokenStream::from(expanded)
}

/// Derives the `Manifest` trait for storage access declarations.
///
/// # Attributes
///
/// - `#[manifest(read)]` - Declares read access to a slot
/// - `#[manifest(write)]` - Declares write access to a slot
/// - `#[manifest(commutative)]` - Declares commutative access
///
/// # Example
///
/// ```ignore
/// #[derive(Manifest)]
/// struct Counter {
///     #[manifest(read, write)]
///     value: Slot,
/// }
/// ```
#[proc_macro_derive(Manifest, attributes(manifest))]
pub fn derive_manifest(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let entries = match parse_manifest_attrs(&input.attrs) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let mut statements = Vec::new();

    for entry in entries {
        match entry {
            ManifestEntry::ReadSlot(slot) => statements.push(quote! {
                manifest.add_read_slot(#slot);
            }),
            ManifestEntry::WriteSlot(slot) => statements.push(quote! {
                manifest.add_write_slot(#slot);
            }),
            ManifestEntry::CommutativeSlot(slot) => statements.push(quote! {
                manifest.add_commutative_key(#slot);
            }),
            ManifestEntry::ReadSlotExpr(path) => statements.push(quote! {
                manifest.add_read_slot(#path);
            }),
            ManifestEntry::WriteSlotExpr(path) => statements.push(quote! {
                manifest.add_write_slot(#path);
            }),
            ManifestEntry::CommutativeSlotExpr(path) => statements.push(quote! {
                manifest.add_commutative_key(#path);
            }),
            ManifestEntry::ReadLabel(label) => statements.push(quote! {
                manifest.add_read_slot(::truthlinked_sdk::storage::Slot::from_label(#label).0);
            }),
            ManifestEntry::WriteLabel(label) => statements.push(quote! {
                manifest.add_write_slot(::truthlinked_sdk::storage::Slot::from_label(#label).0);
            }),
            ManifestEntry::CommutativeLabel(label) => statements.push(quote! {
                manifest.add_commutative_key(::truthlinked_sdk::storage::Slot::from_label(#label).0);
            }),
            ManifestEntry::ReadDerived { namespace, key } => {
                let slot = derived_slot_expr(&namespace, quote! { #key.as_bytes() });
                statements.push(quote! {
                    manifest.add_read_slot(#slot);
                });
            }
            ManifestEntry::WriteDerived { namespace, key } => {
                let slot = derived_slot_expr(&namespace, quote! { #key.as_bytes() });
                statements.push(quote! {
                    manifest.add_write_slot(#slot);
                });
            }
            ManifestEntry::CommutativeDerived { namespace, key } => {
                let slot = derived_slot_expr(&namespace, quote! { #key.as_bytes() });
                statements.push(quote! {
                    manifest.add_commutative_key(#slot);
                });
            }
            ManifestEntry::ReadMap { namespace, key } => {
                let exists = prefixed_slot_expr(&namespace, "map:exists", quote! { #key.as_bytes() });
                let value = prefixed_slot_expr(&namespace, "map:value", quote! { #key.as_bytes() });
                statements.push(quote! {
                    manifest.add_read_slot(#exists);
                    manifest.add_read_slot(#value);
                });
            }
            ManifestEntry::WriteMap { namespace, key } => {
                let exists = prefixed_slot_expr(&namespace, "map:exists", quote! { #key.as_bytes() });
                let value = prefixed_slot_expr(&namespace, "map:value", quote! { #key.as_bytes() });
                statements.push(quote! {
                    manifest.add_write_slot(#exists);
                    manifest.add_write_slot(#value);
                });
            }
            ManifestEntry::ReadVecLen { namespace } => {
                let len_slot = vec_len_slot_expr(&namespace);
                statements.push(quote! {
                    manifest.add_read_slot(#len_slot);
                });
            }
            ManifestEntry::WriteVecLen { namespace } => {
                let len_slot = vec_len_slot_expr(&namespace);
                statements.push(quote! {
                    manifest.add_write_slot(#len_slot);
                });
            }
            ManifestEntry::ReadVecIndex { namespace, index } => {
                let len_slot = vec_len_slot_expr(&namespace);
                let elem_slot = vec_elem_slot_expr(&namespace, index);
                statements.push(quote! {
                    manifest.add_read_slot(#len_slot);
                    manifest.add_read_slot(#elem_slot);
                });
            }
            ManifestEntry::WriteVecIndex { namespace, index } => {
                let len_slot = vec_len_slot_expr(&namespace);
                let elem_slot = vec_elem_slot_expr(&namespace, index);
                statements.push(quote! {
                    manifest.add_read_slot(#len_slot);
                    manifest.add_write_slot(#elem_slot);
                });
            }
            ManifestEntry::ReadBlobChunk { namespace, chunk } => {
                let len_slot = blob_len_slot_expr(&namespace);
                let chunk_slot = blob_chunk_slot_expr(&namespace, chunk);
                statements.push(quote! {
                    manifest.add_read_slot(#len_slot);
                    manifest.add_read_slot(#chunk_slot);
                });
            }
            ManifestEntry::WriteBlobChunk { namespace, chunk } => {
                let len_slot = blob_len_slot_expr(&namespace);
                let chunk_slot = blob_chunk_slot_expr(&namespace, chunk);
                statements.push(quote! {
                    manifest.add_write_slot(#len_slot);
                    manifest.add_write_slot(#chunk_slot);
                });
            }
            ManifestEntry::StorageKeySpec { offset, len } => {
                statements.push(quote! {
                    manifest.add_storage_key_spec(::truthlinked_sdk::manifest::StorageKeySpec { offset: #offset, len: #len });
                });
            }
        }
    }

    let expanded = quote! {
        impl ::truthlinked_sdk::manifest::Manifest for #name {
            fn manifest() -> ::truthlinked_sdk::manifest::ContractManifest {
                let mut manifest = ::truthlinked_sdk::manifest::ContractManifest::new();
                #(#statements)*
                manifest.normalize();
                manifest
            }
        }
    };

    TokenStream::from(expanded)
}

/// Derives the `BytesCodec` trait for variable-length encoding.
///
/// Automatically implements encoding and decoding for structs and enums.
///
/// # Example
///
/// ```ignore
/// #[derive(BytesCodec)]
/// struct Record {
///     id: u64,
///     name: String,
/// }
/// ```
#[proc_macro_derive(BytesCodec)]
pub fn derive_bytes_codec(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let generated = match bytescodec_impl(&name, &input.data) {
        Ok(tokens) => tokens,
        Err(err) => return err.to_compile_error().into(),
    };
    TokenStream::from(generated)
}

/// Derives the `Codec32` trait for 32-byte fixed-size encoding.
///
/// Requires the type to also implement `BytesCodec`.
///
/// # Example
///
/// ```ignore
/// #[derive(BytesCodec, Codec32)]
/// struct SmallRecord {
///     id: u16,
///     enabled: bool,
/// }
/// ```
#[proc_macro_derive(Codec32)]
pub fn derive_codec32(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let generated = quote! {
        impl ::truthlinked_sdk::codec::Codec32 for #name
        where
            #name: ::truthlinked_sdk::codec::BytesCodec,
        {
            fn encode_32(&self) -> [u8; 32] {
                let payload = ::truthlinked_sdk::codec::BytesCodec::encode_bytes(self);
                if payload.len() > 31 {
                    panic!("Codec32 derive overflow: encoded payload > 31 bytes");
                }
                let mut out = [0u8; 32];
                out[0] = payload.len() as u8;
                out[1..1 + payload.len()].copy_from_slice(&payload);
                out
            }

            fn decode_32(bytes: &[u8; 32]) -> ::truthlinked_sdk::Result<Self> {
                let len = bytes[0] as usize;
                if len > 31 {
                    return Err(::truthlinked_sdk::Error::new(::truthlinked_sdk::codec::ERR_CODEC));
                }
                <#name as ::truthlinked_sdk::codec::BytesCodec>::decode_bytes(&bytes[1..1 + len])
            }
        }
    };
    TokenStream::from(generated)
}

/// Attribute macro for defining error code enums.
///
/// # Example
///
/// ```ignore
/// #[error_code(base = -100)]
/// enum MyError {
///     NotFound,      // -100
///     #[code = -50]
///     Custom,        // -50
///     Invalid,       // -101
/// }
/// ```
#[proc_macro_attribute]
pub fn error_code(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as ErrorCodeArgs);
    let enum_item = parse_macro_input!(item as ItemEnum);

    let mut next_auto = args.base;
    let mut mappings = Vec::<(syn::Ident, i32)>::new();

    for variant in &enum_item.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return syn::Error::new_spanned(variant, "#[error_code] only supports unit variants")
                .to_compile_error()
                .into();
        }

        let code = if let Some((_, expr)) = &variant.discriminant {
            match parse_i32_expr(expr) {
                Ok(v) => v,
                Err(err) => return err.to_compile_error().into(),
            }
        } else if let Some(next) = next_auto {
            let current = next;
            next_auto = next.checked_add(1);
            current
        } else {
            return syn::Error::new_spanned(
                variant,
                "missing discriminant; use explicit `= code` or #[error_code(base = N)]",
            )
            .to_compile_error()
            .into();
        };

        mappings.push((variant.ident.clone(), code));
    }

    let enum_name = enum_item.ident.clone();
    let variants = mappings.iter().map(|(ident, _)| ident);
    let codes = mappings.iter().map(|(_, code)| code);

    let expanded = quote! {
        #enum_item

        impl #enum_name {
            pub const fn code(self) -> i32 {
                match self {
                    #(Self::#variants => #codes,)*
                }
            }
        }

        impl From<#enum_name> for ::truthlinked_sdk::Error {
            fn from(value: #enum_name) -> Self {
                ::truthlinked_sdk::Error::new(value.code())
            }
        }

        impl From<#enum_name> for i32 {
            fn from(value: #enum_name) -> Self {
                value.code()
            }
        }
    };

    TokenStream::from(expanded)
}

/// Attribute macro for adding precondition checks to functions.
///
/// # Example
///
/// ```ignore
/// #[require(amount > 0, error = MyError::InvalidAmount)]
/// fn transfer(amount: u64) -> Result<()> {
///     // Function body
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn require(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as RequireArgs);
    let mut function = parse_macro_input!(item as ItemFn);

    let condition = args.condition;
    let error_expr = if let Some(expr) = args.error {
        quote! { #expr }
    } else {
        quote! { ::truthlinked_sdk::Error::new(::truthlinked_sdk::error::ERR_REQUIRE) }
    };

    let guard_stmt: syn::Stmt = match syn::parse2(quote! {
        if !(#condition) {
            return core::result::Result::Err((#error_expr).into());
        }
    }) {
        Ok(stmt) => stmt,
        Err(err) => return err.to_compile_error().into(),
    };

    function.block.stmts.insert(0, guard_stmt);
    TokenStream::from(quote! { #function })
}

struct EventOptions {
    name: String,
    signature: Option<String>,
    anonymous: bool,
}

fn parse_event_options(
    default_name: &str,
    attrs: &[Attribute],
) -> Result<EventOptions, TokenStream> {
    let mut options = EventOptions {
        name: default_name.to_string(),
        signature: None,
        anonymous: false,
    };

    for attr in attrs {
        if !attr.path().is_ident("event") {
            continue;
        }
        let parse_result = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                let value: LitStr = meta.value()?.parse()?;
                options.name = value.value();
                return Ok(());
            }
            if meta.path.is_ident("signature") {
                let value: LitStr = meta.value()?.parse()?;
                options.signature = Some(value.value());
                return Ok(());
            }
            if meta.path.is_ident("anonymous") {
                options.anonymous = true;
                return Ok(());
            }
            Err(meta.error("unsupported event option"))
        });
        if let Err(err) = parse_result {
            return Err(err.to_compile_error().into());
        }
    }

    Ok(options)
}

struct EventField {
    access: proc_macro2::TokenStream,
    ty: syn::Type,
    is_topic: bool,
    skip: bool,
    signature_type: Option<String>,
    with_topic: Option<Path>,
    with_data: Option<Path>,
}

fn event_fields(data: &Data) -> Result<Vec<EventField>, TokenStream> {
    let mut fields_out = Vec::new();
    match data {
        Data::Struct(data_struct) => {
            match &data_struct.fields {
                Fields::Named(named) => {
                    for field in &named.named {
                        let options = match parse_event_field_options(field) {
                            Ok(v) => v,
                            Err(e) => return Err(e.to_compile_error().into()),
                        };
                        let ident = field.ident.clone().expect("named field must have ident");
                        fields_out.push(EventField {
                            access: quote! { self.#ident },
                            ty: field.ty.clone(),
                            is_topic: options.is_topic,
                            skip: options.skip,
                            signature_type: options.signature_type,
                            with_topic: options.with_topic,
                            with_data: options.with_data,
                        });
                    }
                }
                Fields::Unnamed(unnamed) => {
                    for (idx, field) in unnamed.unnamed.iter().enumerate() {
                        let options = match parse_event_field_options(field) {
                            Ok(v) => v,
                            Err(e) => return Err(e.to_compile_error().into()),
                        };
                        let index = syn::Index::from(idx);
                        fields_out.push(EventField {
                            access: quote! { self.#index },
                            ty: field.ty.clone(),
                            is_topic: options.is_topic,
                            skip: options.skip,
                            signature_type: options.signature_type,
                            with_topic: options.with_topic,
                            with_data: options.with_data,
                        });
                    }
                }
                Fields::Unit => {}
            }
            Ok(fields_out)
        }
        _ => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "Event can only be derived for structs",
        )
        .to_compile_error()
        .into()),
    }
}

struct EventFieldOptions {
    is_topic: bool,
    skip: bool,
    signature_type: Option<String>,
    with_topic: Option<Path>,
    with_data: Option<Path>,
}

fn parse_event_field_options(field: &syn::Field) -> syn::Result<EventFieldOptions> {
    let mut out = EventFieldOptions {
        is_topic: false,
        skip: false,
        signature_type: None,
        with_topic: None,
        with_data: None,
    };

    for attr in &field.attrs {
        if attr.path().is_ident("topic") {
            out.is_topic = true;
            continue;
        }

        if !attr.path().is_ident("event") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("topic") || meta.path.is_ident("indexed") {
                out.is_topic = true;
                return Ok(());
            }
            if meta.path.is_ident("skip") {
                out.skip = true;
                return Ok(());
            }
            if meta.path.is_ident("type") {
                let value: LitStr = meta.value()?.parse()?;
                out.signature_type = Some(value.value());
                return Ok(());
            }
            if meta.path.is_ident("with_topic") {
                let value: LitStr = meta.value()?.parse()?;
                let path = syn::parse_str::<Path>(&value.value())
                    .map_err(|_| meta.error("with_topic must be a valid path string"))?;
                out.with_topic = Some(path);
                out.is_topic = true;
                return Ok(());
            }
            if meta.path.is_ident("with_data") {
                let value: LitStr = meta.value()?.parse()?;
                let path = syn::parse_str::<Path>(&value.value())
                    .map_err(|_| meta.error("with_data must be a valid path string"))?;
                out.with_data = Some(path);
                return Ok(());
            }
            Err(meta.error("unsupported field event option"))
        })?;
    }

    if out.skip && out.is_topic {
        return Err(syn::Error::new(
            field.span(),
            "field cannot be both #[topic] and #[event(skip)]",
        ));
    }
    if out.skip && out.with_data.is_some() {
        return Err(syn::Error::new(
            field.span(),
            "field cannot be both #[event(skip)] and #[event(with_data = ...)]",
        ));
    }
    if out.is_topic && out.with_data.is_some() {
        return Err(syn::Error::new(
            field.span(),
            "topic fields cannot define #[event(with_data = ...)]",
        ));
    }

    Ok(out)
}

enum ManifestEntry {
    ReadSlot(proc_macro2::TokenStream),
    WriteSlot(proc_macro2::TokenStream),
    CommutativeSlot(proc_macro2::TokenStream),
    ReadSlotExpr(Path),
    WriteSlotExpr(Path),
    CommutativeSlotExpr(Path),
    ReadLabel(String),
    WriteLabel(String),
    CommutativeLabel(String),
    ReadDerived { namespace: String, key: String },
    WriteDerived { namespace: String, key: String },
    CommutativeDerived { namespace: String, key: String },
    ReadMap { namespace: String, key: String },
    WriteMap { namespace: String, key: String },
    ReadVecLen { namespace: String },
    WriteVecLen { namespace: String },
    ReadVecIndex { namespace: String, index: u64 },
    WriteVecIndex { namespace: String, index: u64 },
    ReadBlobChunk { namespace: String, chunk: u64 },
    WriteBlobChunk { namespace: String, chunk: u64 },
    StorageKeySpec { offset: usize, len: usize },
}

fn parse_manifest_attrs(attrs: &[Attribute]) -> Result<Vec<ManifestEntry>, TokenStream> {
    let mut out = Vec::new();

    for attr in attrs {
        if !attr.path().is_ident("manifest") {
            continue;
        }

        let parse_result = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("read_slot") {
                let value: LitStr = meta.value()?.parse()?;
                let slot = parse_slot_hex_literal(&value)?;
                out.push(ManifestEntry::ReadSlot(slot));
                return Ok(());
            }
            if meta.path.is_ident("write_slot") {
                let value: LitStr = meta.value()?.parse()?;
                let slot = parse_slot_hex_literal(&value)?;
                out.push(ManifestEntry::WriteSlot(slot));
                return Ok(());
            }
            if meta.path.is_ident("commutative_slot") {
                let value: LitStr = meta.value()?.parse()?;
                let slot = parse_slot_hex_literal(&value)?;
                out.push(ManifestEntry::CommutativeSlot(slot));
                return Ok(());
            }

            if meta.path.is_ident("read_slot_expr") {
                let value: LitStr = meta.value()?.parse()?;
                let path = parse_path_literal(&value)?;
                out.push(ManifestEntry::ReadSlotExpr(path));
                return Ok(());
            }
            if meta.path.is_ident("write_slot_expr") {
                let value: LitStr = meta.value()?.parse()?;
                let path = parse_path_literal(&value)?;
                out.push(ManifestEntry::WriteSlotExpr(path));
                return Ok(());
            }
            if meta.path.is_ident("commutative_slot_expr") {
                let value: LitStr = meta.value()?.parse()?;
                let path = parse_path_literal(&value)?;
                out.push(ManifestEntry::CommutativeSlotExpr(path));
                return Ok(());
            }

            if meta.path.is_ident("read_label") {
                let value: LitStr = meta.value()?.parse()?;
                out.push(ManifestEntry::ReadLabel(value.value()));
                return Ok(());
            }
            if meta.path.is_ident("write_label") {
                let value: LitStr = meta.value()?.parse()?;
                out.push(ManifestEntry::WriteLabel(value.value()));
                return Ok(());
            }
            if meta.path.is_ident("commutative_label") {
                let value: LitStr = meta.value()?.parse()?;
                out.push(ManifestEntry::CommutativeLabel(value.value()));
                return Ok(());
            }

            if meta.path.is_ident("read_derived") {
                let (namespace, key) = parse_namespace_key(meta)?;
                out.push(ManifestEntry::ReadDerived { namespace, key });
                return Ok(());
            }
            if meta.path.is_ident("write_derived") {
                let (namespace, key) = parse_namespace_key(meta)?;
                out.push(ManifestEntry::WriteDerived { namespace, key });
                return Ok(());
            }
            if meta.path.is_ident("commutative_derived") {
                let (namespace, key) = parse_namespace_key(meta)?;
                out.push(ManifestEntry::CommutativeDerived { namespace, key });
                return Ok(());
            }

            if meta.path.is_ident("read_map") {
                let (namespace, key) = parse_namespace_key(meta)?;
                out.push(ManifestEntry::ReadMap { namespace, key });
                return Ok(());
            }
            if meta.path.is_ident("write_map") {
                let (namespace, key) = parse_namespace_key(meta)?;
                out.push(ManifestEntry::WriteMap { namespace, key });
                return Ok(());
            }

            if meta.path.is_ident("read_vec_len") {
                let namespace = parse_namespace(meta)?;
                out.push(ManifestEntry::ReadVecLen { namespace });
                return Ok(());
            }
            if meta.path.is_ident("write_vec_len") {
                let namespace = parse_namespace(meta)?;
                out.push(ManifestEntry::WriteVecLen { namespace });
                return Ok(());
            }
            if meta.path.is_ident("read_vec_index") {
                let (namespace, index) = parse_namespace_index(meta, "index")?;
                out.push(ManifestEntry::ReadVecIndex { namespace, index });
                return Ok(());
            }
            if meta.path.is_ident("write_vec_index") {
                let (namespace, index) = parse_namespace_index(meta, "index")?;
                out.push(ManifestEntry::WriteVecIndex { namespace, index });
                return Ok(());
            }

            if meta.path.is_ident("read_blob_chunk") {
                let (namespace, chunk) = parse_namespace_index(meta, "chunk")?;
                out.push(ManifestEntry::ReadBlobChunk { namespace, chunk });
                return Ok(());
            }
            if meta.path.is_ident("write_blob_chunk") {
                let (namespace, chunk) = parse_namespace_index(meta, "chunk")?;
                out.push(ManifestEntry::WriteBlobChunk { namespace, chunk });
                return Ok(());
            }

            if meta.path.is_ident("key_spec") {
                let mut offset = None;
                let mut len = None;
                meta.parse_nested_meta(|nested| {
                    if nested.path.is_ident("offset") {
                        let expr: Expr = nested.value()?.parse()?;
                        offset = Some(expr_to_usize(&expr)?);
                        return Ok(());
                    }
                    if nested.path.is_ident("len") {
                        let expr: Expr = nested.value()?.parse()?;
                        len = Some(expr_to_usize(&expr)?);
                        return Ok(());
                    }
                    Err(nested.error("unsupported key_spec argument"))
                })?;

                let offset = offset.ok_or_else(|| meta.error("key_spec missing offset"))?;
                let len = len.ok_or_else(|| meta.error("key_spec missing len"))?;
                out.push(ManifestEntry::StorageKeySpec { offset, len });
                return Ok(());
            }

            Err(meta.error("unsupported manifest attribute"))
        });

        if let Err(err) = parse_result {
            return Err(err.to_compile_error().into());
        }
    }

    Ok(out)
}

fn parse_namespace(meta: syn::meta::ParseNestedMeta<'_>) -> syn::Result<String> {
    let mut namespace = None;
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("namespace") {
            let value: LitStr = nested.value()?.parse()?;
            namespace = Some(value.value());
            return Ok(());
        }
        Err(nested.error("expected namespace=..."))
    })?;

    namespace.ok_or_else(|| meta.error("missing namespace"))
}

fn parse_namespace_key(meta: syn::meta::ParseNestedMeta<'_>) -> syn::Result<(String, String)> {
    let mut namespace = None;
    let mut key = None;

    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("namespace") {
            let value: LitStr = nested.value()?.parse()?;
            namespace = Some(value.value());
            return Ok(());
        }
        if nested.path.is_ident("key") {
            let value: LitStr = nested.value()?.parse()?;
            key = Some(value.value());
            return Ok(());
        }
        Err(nested.error("expected namespace=... or key=..."))
    })?;

    let namespace = namespace.ok_or_else(|| meta.error("derived spec missing namespace"))?;
    let key = key.ok_or_else(|| meta.error("derived spec missing key"))?;
    Ok((namespace, key))
}

fn parse_namespace_index(
    meta: syn::meta::ParseNestedMeta<'_>,
    key_name: &str,
) -> syn::Result<(String, u64)> {
    let mut namespace = None;
    let mut index = None;

    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("namespace") {
            let value: LitStr = nested.value()?.parse()?;
            namespace = Some(value.value());
            return Ok(());
        }
        if nested.path.is_ident(key_name) {
            let expr: Expr = nested.value()?.parse()?;
            index = Some(expr_to_u64(&expr)?);
            return Ok(());
        }
        Err(nested.error("unexpected argument"))
    })?;

    let namespace = namespace.ok_or_else(|| meta.error("missing namespace"))?;
    let index = index.ok_or_else(|| meta.error(format!("missing {}", key_name)))?;
    Ok((namespace, index))
}

fn expr_to_usize(expr: &Expr) -> syn::Result<usize> {
    if let Expr::Lit(expr_lit) = expr {
        if let syn::Lit::Int(int_lit) = &expr_lit.lit {
            return int_lit.base10_parse::<usize>();
        }
    }
    Err(syn::Error::new_spanned(expr, "expected integer literal"))
}

fn expr_to_u64(expr: &Expr) -> syn::Result<u64> {
    if let Expr::Lit(expr_lit) = expr {
        if let syn::Lit::Int(int_lit) = &expr_lit.lit {
            return int_lit.base10_parse::<u64>();
        }
    }
    Err(syn::Error::new_spanned(expr, "expected integer literal"))
}

fn parse_slot_hex_literal(value: &LitStr) -> syn::Result<proc_macro2::TokenStream> {
    let mut text = value.value();
    if let Some(stripped) = text.strip_prefix("0x") {
        text = stripped.to_string();
    }

    if text.len() != 64 {
        return Err(syn::Error::new_spanned(
            value,
            "slot hex must be exactly 32 bytes (64 hex chars)",
        ));
    }

    let mut bytes = [0u8; 32];
    for i in 0..32 {
        let pair = &text[i * 2..i * 2 + 2];
        bytes[i] = u8::from_str_radix(pair, 16)
            .map_err(|_| syn::Error::new_spanned(value, "slot hex contains non-hex characters"))?;
    }

    let values = bytes.iter();
    Ok(quote! { [#(#values),*] })
}

fn parse_path_literal(value: &LitStr) -> syn::Result<Path> {
    syn::parse_str::<Path>(&value.value())
        .map_err(|_| syn::Error::new_spanned(value, "expected a valid Rust path string"))
}

fn derived_slot_expr(namespace: &str, part: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    quote!({
        let __ns = ::truthlinked_sdk::storage::namespace(#namespace);
        ::truthlinked_sdk::hashing::derive_slot(&__ns, &[#part])
    })
}

fn prefixed_slot_expr(
    namespace: &str,
    prefix: &str,
    part: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let prefix_lit = syn::LitByteStr::new(prefix.as_bytes(), proc_macro2::Span::call_site());
    quote!({
        let __ns = ::truthlinked_sdk::storage::namespace(#namespace);
        ::truthlinked_sdk::hashing::derive_slot(&__ns, &[#prefix_lit, #part])
    })
}

fn vec_len_slot_expr(namespace: &str) -> proc_macro2::TokenStream {
    quote!({
        let __ns = ::truthlinked_sdk::storage::namespace(#namespace);
        ::truthlinked_sdk::hashing::derive_slot(&__ns, &[b"vec:len"])
    })
}

fn vec_elem_slot_expr(namespace: &str, index: u64) -> proc_macro2::TokenStream {
    quote!({
        let __ns = ::truthlinked_sdk::storage::namespace(#namespace);
        let __idx = (#index as u64).to_le_bytes();
        ::truthlinked_sdk::hashing::derive_slot(&__ns, &[b"vec:elem", &__idx])
    })
}

fn blob_len_slot_expr(namespace: &str) -> proc_macro2::TokenStream {
    quote!({
        let __ns = ::truthlinked_sdk::storage::namespace(#namespace);
        ::truthlinked_sdk::hashing::derive_slot(&__ns, &[b"blob:len"])
    })
}

fn blob_chunk_slot_expr(namespace: &str, chunk: u64) -> proc_macro2::TokenStream {
    quote!({
        let __ns = ::truthlinked_sdk::storage::namespace(#namespace);
        let __idx = (#chunk as u64).to_le_bytes();
        ::truthlinked_sdk::hashing::derive_slot(&__ns, &[b"blob:chunk", &__idx])
    })
}

fn normalize_type_name(input: &str) -> String {
    input.chars().filter(|c| !c.is_whitespace()).collect()
}

fn bytescodec_impl(name: &syn::Ident, data: &Data) -> syn::Result<TokenStream2> {
    match data {
        Data::Struct(data_struct) => bytescodec_struct_impl(name, data_struct),
        Data::Enum(data_enum) => bytescodec_enum_impl(name, data_enum),
        _ => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "BytesCodec can only be derived for structs or enums",
        )),
    }
}

fn bytescodec_struct_impl(
    name: &syn::Ident,
    data_struct: &syn::DataStruct,
) -> syn::Result<TokenStream2> {
    match &data_struct.fields {
        Fields::Named(named) => {
            let fields: Vec<_> = named
                .named
                .iter()
                .map(|field| {
                    let ident = field.ident.clone().expect("named field");
                    let ty = field.ty.clone();
                    (ident, ty)
                })
                .collect();

            let encode_fields = fields
                .iter()
                .map(|(ident, _)| encode_field_expr(quote! { &self.#ident }));
            let decode_fields = fields
                .iter()
                .map(|(ident, ty)| decode_field_expr(quote! { #ident }, ty));
            let idents = fields.iter().map(|(ident, _)| ident);

            Ok(quote! {
                impl ::truthlinked_sdk::codec::BytesCodec for #name {
                    fn encode_bytes(&self) -> ::truthlinked_sdk::log::__private::Vec<u8> {
                        let mut __out = ::truthlinked_sdk::log::__private::Vec::new();
                        #(#encode_fields)*
                        __out
                    }

                    fn decode_bytes(bytes: &[u8]) -> ::truthlinked_sdk::Result<Self> {
                        let mut __cursor = 0usize;
                        #(#decode_fields)*
                        if __cursor != bytes.len() {
                            return Err(::truthlinked_sdk::Error::new(::truthlinked_sdk::codec::ERR_CODEC));
                        }
                        Ok(Self { #(#idents),* })
                    }
                }
            })
        }
        Fields::Unnamed(unnamed) => {
            let fields: Vec<_> = unnamed
                .unnamed
                .iter()
                .enumerate()
                .map(|(idx, field)| {
                    (
                        syn::Index::from(idx),
                        field.ty.clone(),
                        syn::Ident::new(&format!("__f{}", idx), field.span()),
                    )
                })
                .collect();

            let encode_fields = fields
                .iter()
                .map(|(index, _, _)| encode_field_expr(quote! { &self.#index }));
            let decode_fields = fields
                .iter()
                .map(|(_, ty, ident)| decode_field_expr(quote! { #ident }, ty));
            let idents = fields.iter().map(|(_, _, ident)| ident);

            Ok(quote! {
                impl ::truthlinked_sdk::codec::BytesCodec for #name {
                    fn encode_bytes(&self) -> ::truthlinked_sdk::log::__private::Vec<u8> {
                        let mut __out = ::truthlinked_sdk::log::__private::Vec::new();
                        #(#encode_fields)*
                        __out
                    }

                    fn decode_bytes(bytes: &[u8]) -> ::truthlinked_sdk::Result<Self> {
                        let mut __cursor = 0usize;
                        #(#decode_fields)*
                        if __cursor != bytes.len() {
                            return Err(::truthlinked_sdk::Error::new(::truthlinked_sdk::codec::ERR_CODEC));
                        }
                        Ok(Self(#(#idents),*))
                    }
                }
            })
        }
        Fields::Unit => Ok(quote! {
            impl ::truthlinked_sdk::codec::BytesCodec for #name {
                fn encode_bytes(&self) -> ::truthlinked_sdk::log::__private::Vec<u8> {
                    ::truthlinked_sdk::log::__private::Vec::new()
                }

                fn decode_bytes(bytes: &[u8]) -> ::truthlinked_sdk::Result<Self> {
                    if !bytes.is_empty() {
                        return Err(::truthlinked_sdk::Error::new(::truthlinked_sdk::codec::ERR_CODEC));
                    }
                    Ok(Self)
                }
            }
        }),
    }
}

fn bytescodec_enum_impl(name: &syn::Ident, data_enum: &syn::DataEnum) -> syn::Result<TokenStream2> {
    let mut encode_arms = Vec::new();
    let mut decode_arms = Vec::new();

    for (variant_idx, variant) in data_enum.variants.iter().enumerate() {
        let discriminant = variant_idx as u32;
        let variant_ident = &variant.ident;

        match &variant.fields {
            Fields::Unit => {
                encode_arms.push(quote! {
                    Self::#variant_ident => {
                        __out.extend_from_slice(&#discriminant.to_le_bytes());
                    }
                });

                decode_arms.push(quote! {
                    #discriminant => {
                        if __cursor != bytes.len() {
                            return Err(::truthlinked_sdk::Error::new(::truthlinked_sdk::codec::ERR_CODEC));
                        }
                        Ok(Self::#variant_ident)
                    }
                });
            }
            Fields::Unnamed(unnamed) => {
                let bindings: Vec<_> = unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(idx, field)| {
                        (
                            syn::Ident::new(&format!("__v{}", idx), field.span()),
                            field.ty.clone(),
                        )
                    })
                    .collect();

                let pattern_idents = bindings.iter().map(|(ident, _)| ident);
                let encode_fields = bindings
                    .iter()
                    .map(|(ident, _)| encode_field_expr(quote! { #ident }));

                let decode_bindings = bindings
                    .iter()
                    .map(|(ident, ty)| decode_field_expr(quote! { #ident }, ty));
                let construct_idents = bindings.iter().map(|(ident, _)| ident);

                encode_arms.push(quote! {
                    Self::#variant_ident(#(#pattern_idents),*) => {
                        __out.extend_from_slice(&#discriminant.to_le_bytes());
                        #(#encode_fields)*
                    }
                });

                decode_arms.push(quote! {
                    #discriminant => {
                        #(#decode_bindings)*
                        if __cursor != bytes.len() {
                            return Err(::truthlinked_sdk::Error::new(::truthlinked_sdk::codec::ERR_CODEC));
                        }
                        Ok(Self::#variant_ident(#(#construct_idents),*))
                    }
                });
            }
            Fields::Named(named) => {
                let bindings: Vec<_> = named
                    .named
                    .iter()
                    .map(|field| (field.ident.clone().expect("named field"), field.ty.clone()))
                    .collect();

                let pattern_idents = bindings.iter().map(|(ident, _)| ident);
                let encode_fields = bindings
                    .iter()
                    .map(|(ident, _)| encode_field_expr(quote! { #ident }));
                let decode_bindings = bindings
                    .iter()
                    .map(|(ident, ty)| decode_field_expr(quote! { #ident }, ty));
                let construct_idents = bindings.iter().map(|(ident, _)| ident);

                encode_arms.push(quote! {
                    Self::#variant_ident { #(#pattern_idents),* } => {
                        __out.extend_from_slice(&#discriminant.to_le_bytes());
                        #(#encode_fields)*
                    }
                });

                decode_arms.push(quote! {
                    #discriminant => {
                        #(#decode_bindings)*
                        if __cursor != bytes.len() {
                            return Err(::truthlinked_sdk::Error::new(::truthlinked_sdk::codec::ERR_CODEC));
                        }
                        Ok(Self::#variant_ident { #(#construct_idents),* })
                    }
                });
            }
        }
    }

    Ok(quote! {
        impl ::truthlinked_sdk::codec::BytesCodec for #name {
            fn encode_bytes(&self) -> ::truthlinked_sdk::log::__private::Vec<u8> {
                let mut __out = ::truthlinked_sdk::log::__private::Vec::new();
                match self {
                    #(#encode_arms),*
                }
                __out
            }

            fn decode_bytes(bytes: &[u8]) -> ::truthlinked_sdk::Result<Self> {
                if bytes.len() < 4 {
                    return Err(::truthlinked_sdk::Error::new(::truthlinked_sdk::codec::ERR_CODEC_EOF));
                }
                let mut __cursor = 0usize;
                let mut __discriminant_raw = [0u8; 4];
                __discriminant_raw.copy_from_slice(&bytes[..4]);
                __cursor = 4;
                let __discriminant = u32::from_le_bytes(__discriminant_raw);
                match __discriminant {
                    #(#decode_arms,)*
                    _ => Err(::truthlinked_sdk::Error::new(::truthlinked_sdk::codec::ERR_CODEC)),
                }
            }
        }
    })
}

fn encode_field_expr(access: TokenStream2) -> TokenStream2 {
    quote! {
        let __field = ::truthlinked_sdk::codec::BytesCodec::encode_bytes(#access);
        let __field_len = (__field.len() as u32).to_le_bytes();
        __out.extend_from_slice(&__field_len);
        __out.extend_from_slice(&__field);
    }
}

fn decode_field_expr(binding: TokenStream2, ty: &syn::Type) -> TokenStream2 {
    quote! {
        let #binding = {
            let __end_len = __cursor
                .checked_add(4)
                .ok_or_else(|| ::truthlinked_sdk::Error::new(::truthlinked_sdk::codec::ERR_CODEC_EOF))?;
            if __end_len > bytes.len() {
                return Err(::truthlinked_sdk::Error::new(::truthlinked_sdk::codec::ERR_CODEC_EOF));
            }
            let mut __len_raw = [0u8; 4];
            __len_raw.copy_from_slice(&bytes[__cursor..__end_len]);
            __cursor = __end_len;
            let __field_len = u32::from_le_bytes(__len_raw) as usize;
            let __end_field = __cursor
                .checked_add(__field_len)
                .ok_or_else(|| ::truthlinked_sdk::Error::new(::truthlinked_sdk::codec::ERR_CODEC_EOF))?;
            if __end_field > bytes.len() {
                return Err(::truthlinked_sdk::Error::new(::truthlinked_sdk::codec::ERR_CODEC_EOF));
            }
            let __field_value = <#ty as ::truthlinked_sdk::codec::BytesCodec>::decode_bytes(
                &bytes[__cursor..__end_field],
            )?;
            __cursor = __end_field;
            __field_value
        };
    }
}

struct ErrorCodeArgs {
    base: Option<i32>,
}

impl Parse for ErrorCodeArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self { base: None });
        }
        let ident: syn::Ident = input.parse()?;
        if ident != "base" {
            return Err(syn::Error::new(ident.span(), "expected `base = <i32>`"));
        }
        input.parse::<Token![=]>()?;
        let base: LitInt = input.parse()?;
        let base_value = base.base10_parse::<i32>()?;
        Ok(Self {
            base: Some(base_value),
        })
    }
}

fn parse_i32_expr(expr: &Expr) -> syn::Result<i32> {
    if let Expr::Lit(expr_lit) = expr {
        if let syn::Lit::Int(int_lit) = &expr_lit.lit {
            return int_lit.base10_parse::<i32>();
        }
    }
    Err(syn::Error::new_spanned(
        expr,
        "error code discriminant must be i32 literal",
    ))
}

struct RequireArgs {
    condition: Expr,
    error: Option<Expr>,
}

impl Parse for RequireArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let condition: Expr = input.parse()?;
        let error = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            Some(input.parse::<Expr>()?)
        } else {
            None
        };
        Ok(Self { condition, error })
    }
}
