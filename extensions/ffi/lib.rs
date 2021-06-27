// Copyright 2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op_sync;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use dlopen::raw::Library;
use libffi::middle::Cif;
use serde::Deserialize;
use std::borrow::Cow;
use std::ffi::c_void;
use std::path::Path;
use std::rc::Rc;

pub struct Unstable(pub bool);

fn check_unstable(state: &OpState, api_name: &str) {
  let unstable = state.borrow::<Unstable>();

  if !unstable.0 {
    eprintln!(
      "Unstable API '{}'. The --unstable flag must be provided.",
      api_name
    );
    std::process::exit(70);
  }
}

pub trait FfiPermissions {
  fn check(&mut self) -> Result<(), AnyError>;
  fn check_read(&mut self, path: &Path) -> Result<(), AnyError>;
}

pub struct NoFfiPermissions;

impl FfiPermissions for NoFfiPermissions {
  fn check(&mut self) -> Result<(), AnyError> {
    Ok(())
  }

  fn check_read(&mut self, _path: &Path) -> Result<(), AnyError> {
    Ok(())
  }
}

struct DylibResource(Library);

impl Resource for DylibResource {
  fn name(&self) -> Cow<str> {
    "dylib".into()
  }

  fn close(self: Rc<Self>) {
    drop(self)
  }
}

pub fn init<P: FfiPermissions + 'static>(unstable: bool) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:extensions/ffi",
      "00_ffi.js",
    ))
    .ops(vec![
      ("op_dlopen", op_sync(op_dlopen::<P>)),
      ("op_dlcall", op_sync(op_dlcall::<P>)),
    ])
    .state(move |state| {
      // Stolen from deno_webgpu, is there a better option?
      state.put(Unstable(unstable));
      Ok(())
    })
    .build()
}

fn op_dlopen<FP>(
  state: &mut deno_core::OpState,
  path: String,
  _: (),
) -> Result<ResourceId, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.dlopen");
  let permissions = state.borrow_mut::<FP>();
  permissions.check()?;
  permissions.check_read(Path::new(&path))?;

  Ok(
    state
      .resource_table
      .add(DylibResource(Library::open(path)?)),
  )
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FFIArg {
  arg_type: String,
  value: Value,
}

impl From<FFIArg> for libffi::middle::Arg {
  fn from(arg: FFIArg) -> Self {
    match arg.arg_type.clone().into() {
      FFIType::Void => libffi::middle::Arg::new(&()),
      FFIType::U8 => libffi::middle::Arg::new(&(arg.as_u64() as u8)),
      FFIType::I8 => libffi::middle::Arg::new(&(arg.as_i64() as i8)),
      FFIType::U16 => libffi::middle::Arg::new(&(arg.as_u64() as u16)),
      FFIType::I16 => libffi::middle::Arg::new(&(arg.as_i64() as i16)),
      FFIType::U32 => libffi::middle::Arg::new(&(arg.as_u64() as u32)),
      FFIType::I32 => libffi::middle::Arg::new(&(arg.as_i64() as i32)),
      FFIType::U64 => libffi::middle::Arg::new(&arg.as_u64()),
      FFIType::I64 => libffi::middle::Arg::new(&arg.as_i64()),
      FFIType::USize => libffi::middle::Arg::new(&(arg.as_u64() as usize)),
      FFIType::ISize => libffi::middle::Arg::new(&(arg.as_i64() as isize)),
      FFIType::F32 => libffi::middle::Arg::new(&(arg.as_f64() as f32)),
      FFIType::F64 => libffi::middle::Arg::new(&arg.as_f64()),
    }
  }
}

impl FFIArg {
  fn as_u64(&self) -> u64 {
    self
      .value
      .as_u64()
      .expect("Expected ffi arg value to be an unsigned integer")
  }

  fn as_i64(&self) -> i64 {
    self
      .value
      .as_i64()
      .expect("Expected ffi arg value to be a signed integer")
  }

  fn as_f64(&self) -> f64 {
    self
      .value
      .as_f64()
      .expect("Expected ffi arg value to be a float")
  }
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum FFIType {
  Void,
  U8,
  I8,
  U16,
  I16,
  U32,
  I32,
  U64,
  I64,
  USize,
  ISize,
  F32,
  F64,
  //  Ptr,
  //  CStr,
  //  Struct(Vec<FFIType>),
}

impl From<FFIType> for libffi::middle::Type {
  fn from(r#type: FFIType) -> Self {
    match r#type {
      FFIType::Void => libffi::middle::Type::void(),
      FFIType::U8 => libffi::middle::Type::u8(),
      FFIType::I8 => libffi::middle::Type::i8(),
      FFIType::U16 => libffi::middle::Type::u16(),
      FFIType::I16 => libffi::middle::Type::i16(),
      FFIType::U32 => libffi::middle::Type::u32(),
      FFIType::I32 => libffi::middle::Type::i32(),
      FFIType::U64 => libffi::middle::Type::u64(),
      FFIType::I64 => libffi::middle::Type::i64(),
      FFIType::USize => libffi::middle::Type::usize(),
      FFIType::ISize => libffi::middle::Type::isize(),
      FFIType::F32 => libffi::middle::Type::f32(),
      FFIType::F64 => libffi::middle::Type::f64(),
    }
  }
}

impl From<String> for FFIType {
  fn from(string: String) -> Self {
    match string.as_str() {
      "void" => FFIType::Void,
      "u8" => FFIType::U8,
      "i8" => FFIType::I8,
      "u16" => FFIType::U16,
      "i16" => FFIType::I16,
      "u32" => FFIType::U32,
      "i32" => FFIType::I32,
      "u64" => FFIType::U64,
      "i64" => FFIType::I64,
      "usize" => FFIType::USize,
      "isize" => FFIType::ISize,
      "f32" => FFIType::F32,
      "f64" => FFIType::F64,
      _ => unimplemented!(),
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DlcallArgs {
  sym: String,
  args: Vec<FFIArg>,
  return_type: String,
}

fn op_dlcall<FP>(
  state: &mut deno_core::OpState,
  rid: ResourceId,
  dlcall_args: DlcallArgs,
) -> Result<Value, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.dlcall");
  let permissions = state.borrow_mut::<FP>();
  permissions.check()?;

  let library = state
    .resource_table
    .get::<DylibResource>(rid)
    .ok_or_else(bad_resource_id)?;
  let fn_ptr = unsafe { library.0.symbol::<*const c_void>(&dlcall_args.sym) }?;
  let fn_code_ptr = libffi::middle::CodePtr::from_ptr(fn_ptr as _);
  let types = dlcall_args
    .args
    .clone()
    .into_iter()
    .map(|arg| FFIType::from(arg.arg_type).into());
  let return_type = FFIType::from(dlcall_args.return_type);
  let cif = Cif::new(types, return_type.into());
  let args: Vec<libffi::middle::Arg> =
    dlcall_args.args.into_iter().map(|arg| arg.into()).collect();

  Ok(match return_type {
    FFIType::Void => json!(unsafe { cif.call::<()>(fn_code_ptr, &args) }),
    FFIType::U8 => json!(unsafe { cif.call::<u8>(fn_code_ptr, &args) }),
    FFIType::I8 => json!(unsafe { cif.call::<i8>(fn_code_ptr, &args) }),
    FFIType::U16 => json!(unsafe { cif.call::<u16>(fn_code_ptr, &args) }),
    FFIType::I16 => json!(unsafe { cif.call::<i16>(fn_code_ptr, &args) }),
    FFIType::U32 => json!(unsafe { cif.call::<u32>(fn_code_ptr, &args) }),
    FFIType::I32 => json!(unsafe { cif.call::<i32>(fn_code_ptr, &args) }),
    FFIType::U64 => json!(unsafe { cif.call::<u64>(fn_code_ptr, &args) }),
    FFIType::I64 => json!(unsafe { cif.call::<i64>(fn_code_ptr, &args) }),
    FFIType::USize => json!(unsafe { cif.call::<usize>(fn_code_ptr, &args) }),
    FFIType::ISize => json!(unsafe { cif.call::<isize>(fn_code_ptr, &args) }),
    FFIType::F32 => json!(unsafe { cif.call::<f32>(fn_code_ptr, &args) }),
    FFIType::F64 => json!(unsafe { cif.call::<f64>(fn_code_ptr, &args) }),
  })
}
