use serde_json::{json, Value}; // JSON 构造
use windows::Win32::System::Ole::{SafeArrayCreateVector, SafeArrayPutElement}; // SAFEARRAY
use std::thread::sleep; // COM_BUSY 退避
use std::time::Duration; // 退避间隔
use windows::Win32::Foundation::VARIANT_BOOL; // 布尔变体类型
use windows::Win32::System::Com::{CLSIDFromProgID, CoCreateInstance, DISPPARAMS, DISPATCH_FLAGS, DISPATCH_METHOD, DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT, EXCEPINFO, IDispatch, CLSCTX_LOCAL_SERVER}; // COM 核心
use windows::Win32::System::Variant::{VariantClear, VariantCopy, VARIANT, VT_ARRAY, VT_BOOL, VT_BSTR, VT_BYREF, VT_DISPATCH, VT_EMPTY, VT_I4, VT_I8, VT_NULL, VT_R4, VT_R8, VT_VARIANT}; // VARIANT 类型
use windows::core::{BSTR, GUID, Interface, IUnknown, PCWSTR}; // COM 基础类型
use crate::com::constants::{DISPID_PROPERTYPUT, LOCALE_USER_DEFAULT, WORD_PROGID}; // 常量
use crate::word::report::ToolError; // 错误类型
pub type ComResult<T> = std::result::Result<T, ToolError>; // COM 结果别名
#[derive(Clone)] // COM 对象包装（Clone 复用同一 IDispatch 引用）
pub struct ComObj { // IDispatch 包装
    disp: IDispatch, // 底层接口
}
impl std::fmt::Debug for ComObj { // Debug 实现（IDispatch 无 Debug）
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("ComObj") } // 简写
}
impl ComObj { // 包装操作
    pub fn new(disp: IDispatch) -> Self { Self { disp } } // 构造
    pub fn from_iunknown(unknown: windows::core::IUnknown) -> ComResult<Self> { // 从 IUnknown 查询 IDispatch
        let disp = unknown.cast::<IDispatch>().map_err(|e| com_error("QUERY_INTERFACE", e.code().0))?; // QueryInterface
        Ok(Self { disp }) // 返回
    }
    pub fn raw(&self) -> &IDispatch { &self.disp } // 暴露底层接口
    pub fn leaked_ptr(&self) -> usize { let clone = self.clone(); let raw = clone.raw().as_raw() as usize; std::mem::forget(clone); raw } // 登记用裸指针（持有引用）
    pub fn from_raw_ptr(ptr: usize) -> Option<ComObj> { let raw = ptr as *mut std::ffi::c_void; let disp = unsafe { IDispatch::from_raw_borrowed(&raw) }?; Some(Self::new(disp.clone())) } // 从登记指针重建
    pub unsafe fn release_raw(ptr: usize) { let raw = ptr as *mut std::ffi::c_void; drop(unsafe { IUnknown::from_raw(raw) }); } // 释放登记引用
}
pub fn app_clsid(progid: &str) -> ComResult<GUID> { // 取任意 Office ProgID 的 CLSID
    let wide: Vec<u16> = progid.encode_utf16().chain(std::iter::once(0)).collect(); // ProgID 转 UTF-16
    unsafe { CLSIDFromProgID(PCWSTR::from_raw(wide.as_ptr())) }.map_err(|e| com_error("CLSIDFromProgID", e.code().0)) // 映射错误
}
pub fn word_clsid() -> ComResult<GUID> { app_clsid(WORD_PROGID) } // Word CLSID（兼容旧接口）
pub fn create_app(progid: &str) -> ComResult<ComObj> { // 新建任意 Office Application 实例
    let clsid = app_clsid(progid)?; // CLSID
    let disp: IDispatch = unsafe { CoCreateInstance(&clsid, None, CLSCTX_LOCAL_SERVER) }.map_err(|e| com_error("CoCreateInstance", e.code().0))?; // 创建实例
    Ok(ComObj::new(disp)) // 返回
}
pub fn create_word() -> ComResult<ComObj> { create_app(WORD_PROGID) } // 新建 Word（兼容旧接口）
pub fn attach_app(progid: &str) -> ComResult<ComObj> { // 附着到已运行的 Office Application
    use windows::Win32::System::Ole::GetActiveObject; // 活动对象表
    let clsid = app_clsid(progid)?; // CLSID
    let mut unknown: Option<windows::core::IUnknown> = None; // 接收指针
    unsafe { GetActiveObject(&clsid, None, &mut unknown) }.map_err(|e| com_error("GetActiveObject", e.code().0))?; // 取活动对象
    let unknown = unknown.ok_or_else(|| ToolError::new("APP_NOT_INSTALLED", format!("没有运行中的 {progid} 实例")))?; // 空指针
    ComObj::from_iunknown(unknown) // 查询 IDispatch
}
pub fn attach_word() -> ComResult<ComObj> { attach_app(WORD_PROGID) } // 附着 Word（兼容旧接口）
#[derive(Clone, Copy)] // 调用标志
pub enum Flags { // Invoke 类型
    Get, // 属性读
    Put, // 属性写
    Method, // 方法
}
impl Flags { // 标志转换
    fn value(self) -> DISPATCH_FLAGS { // 转 DISPATCH_FLAGS
        match self { // 分派
            Flags::Get => DISPATCH_PROPERTYGET, // 读
            Flags::Put => DISPATCH_PROPERTYPUT, // 写
            Flags::Method => DISPATCH_METHOD, // 方法
        }
    }
}
#[derive(Clone, Debug)] // VARIANT 值（JSON ↔ COM 桥）
pub enum Variant { // 支持的变体类型
    Empty, // 空
    I4(i32), // 4 字节整数
    I8(i64), // 8 字节整数
    R4(f32), // 单精度
    R8(f64), // 双精度
    Bool(bool), // 布尔
    Str(String), // 字符串
    Dispatch(ComObj), // COM 对象
    Array(Vec<String>), // 字符串数组（SAFEARRAY BSTR）
    R8Array(Vec<f64>), // 双精度数组（SAFEARRAY R8，图表 Values 用）
}
impl Variant { // 变体构造
    pub fn from_json(value: &Value) -> ComResult<Self> { // JSON 转 VARIANT
        match value { // 按类型分派
            Value::Null => Ok(Variant::Empty), // null → 空
            Value::Bool(flag) => Ok(Variant::Bool(*flag)), // 布尔
            Value::Number(num) => { // 数字
                if let Some(i) = num.as_i64() { // 整数
                    if i >= i32::MIN as i64 && i <= i32::MAX as i64 { Ok(Variant::I4(i as i32)) } else { Ok(Variant::I8(i)) } // 按范围选 I4/I8
                } else { // 浮点
                    Ok(Variant::R8(num.as_f64().unwrap_or(0.0))) // 双精度
                }
            }
            Value::String(text) => Ok(Variant::Str(text.clone())), // 字符串
            Value::Object(_) | Value::Array(_) => Ok(Variant::Str(value.to_string())), // 复合类型退化为 JSON 字符串
        }
    }
    fn as_raw(&self) -> VARIANT { // 构造原生 VARIANT
        let mut raw = VARIANT::default(); // 零初始化
        unsafe { // 写 union 字段
            let inner = &mut raw.Anonymous.Anonymous; // VARIANT_0_0
            match self { // 按类型填充
                Variant::Empty => { inner.vt = VT_EMPTY; } // 空
                Variant::I4(value) => { inner.vt = VT_I4; inner.Anonymous.lVal = *value; } // I4
                Variant::I8(value) => { inner.vt = VT_I8; inner.Anonymous.llVal = *value; } // I8
                Variant::R4(value) => { inner.vt = VT_R4; inner.Anonymous.fltVal = *value; } // R4
                Variant::R8(value) => { inner.vt = VT_R8; inner.Anonymous.dblVal = *value; } // R8
                Variant::Bool(flag) => { inner.vt = VT_BOOL; inner.Anonymous.boolVal = VARIANT_BOOL(if *flag { -1 } else { 0 }); } // BOOL
                Variant::Str(text) => { inner.vt = VT_BSTR; inner.Anonymous.bstrVal = std::mem::ManuallyDrop::new(BSTR::from(text.as_str())); } // BSTR
                Variant::Dispatch(obj) => { inner.vt = VT_DISPATCH; inner.Anonymous.pdispVal = std::mem::ManuallyDrop::new(Some(obj.disp.clone())); } // DISPATCH
                Variant::Array(items) => { // SAFEARRAY(BSTR)
                    let array = SafeArrayCreateVector(VT_BSTR, 0, items.len() as u32); // 一维数组
                    for (index, item) in items.iter().enumerate() { // 逐元素
                        let text = BSTR::from(item.as_str()); // BSTR
                        let _ = SafeArrayPutElement(array, &(index as i32), text.as_ptr() as *const std::ffi::c_void); // 写入（数组接管释放）
                        std::mem::forget(text); // 交由 SafeArrayDestroy 释放
                    }
                    inner.vt = VT_ARRAY | VT_BSTR; // 数组类型
                    inner.Anonymous.parray = array; // 绑定
                }
                Variant::R8Array(items) => { // SAFEARRAY(R8)
                    let array = SafeArrayCreateVector(VT_R8, 0, items.len() as u32); // 一维数组
                    for (index, item) in items.iter().enumerate() { // 逐元素
                        let _ = SafeArrayPutElement(array, &(index as i32), item as *const f64 as *const std::ffi::c_void); // 写入 double
                    }
                    inner.vt = VT_ARRAY | VT_R8; // 数组类型
                    inner.Anonymous.parray = array; // 绑定
                }
            }
        }
        raw // 返回
    }
    fn from_raw(raw: &VARIANT) -> ComResult<Self> { // 原生 VARIANT 转回
        unsafe { // 读 union 字段
            let inner = &raw.Anonymous.Anonymous; // VARIANT_0_0
            let vt = inner.vt; // 类型标记
            if vt.0 & VT_BYREF.0 != 0 { // BYREF：优先直接解引用，失败再 VariantCopy
                let base = vt.0 & 0x0FFF; // 去掉 BYREF 标志
                if base == VT_R8.0 { let ptr = inner.Anonymous.pdblVal; if !ptr.is_null() { return Ok(Variant::R8(*ptr)); } } // R8
                if base == VT_R4.0 { let ptr = inner.Anonymous.pfltVal; if !ptr.is_null() { return Ok(Variant::R4(*ptr)); } } // R4
                if base == VT_I4.0 { let ptr = inner.Anonymous.plVal; if !ptr.is_null() { return Ok(Variant::I4(*ptr)); } } // I4
                if base == VT_I8.0 { let ptr = inner.Anonymous.pllVal; if !ptr.is_null() { return Ok(Variant::I8(*ptr)); } } // I8
                if base == VT_BOOL.0 { let ptr = inner.Anonymous.pboolVal; if !ptr.is_null() { return Ok(Variant::Bool((*ptr).0 != 0)); } } // BOOL
                if base == VT_BSTR.0 { let ptr = inner.Anonymous.pbstrVal; if !ptr.is_null() { return Ok(Variant::Str((*ptr).to_string())); } } // BSTR
                if base == VT_VARIANT.0 { let ptr = inner.Anonymous.pvarVal; if !ptr.is_null() { return Self::from_raw(&*ptr); } } // VARIANT
                let mut copy = VARIANT::default(); // 复制目标
                if VariantCopy(&mut copy, raw).is_ok() { // 回退 VariantCopy
                    let value = Self::from_raw(&copy); // 递归解析
                    let _ = VariantClear(&mut copy); // 释放副本
                    return value; // 返回
                }
            }
            if vt == VT_EMPTY || vt == VT_NULL { return Ok(Variant::Empty); } // 空值
            if vt == VT_I4 { return Ok(Variant::I4(inner.Anonymous.lVal)); } // I4
            if vt == VT_I8 { return Ok(Variant::I8(inner.Anonymous.llVal)); } // I8
            if vt == VT_R4 { return Ok(Variant::R4(inner.Anonymous.fltVal)); } // R4
            if vt == VT_R8 { return Ok(Variant::R8(inner.Anonymous.dblVal)); } // R8
            if vt == VT_BOOL { return Ok(Variant::Bool(inner.Anonymous.boolVal.0 != 0)); } // BOOL
            if vt == VT_BSTR { // BSTR
                let text = inner.Anonymous.bstrVal.to_string(); // 转字符串
                return Ok(Variant::Str(text)); // 返回
            }
            if vt == VT_DISPATCH { // DISPATCH
                let slot = inner.Anonymous.pdispVal.as_ref(); // ManuallyDrop<Option<IDispatch>>
                if let Some(disp) = slot.as_ref() { return Ok(Variant::Dispatch(ComObj::new((*disp).clone()))); } // 包装（clone 已 AddRef）
                return Ok(Variant::Empty); // 空接口
            }
            Ok(Variant::Empty) // 未支持类型按空处理
        }
    }
    pub fn as_string(&self) -> String { // 取字符串
        match self { // 分派
            Variant::Str(text) => text.clone(), // 字符串
            Variant::Empty => String::new(), // 空
            Variant::I4(value) => value.to_string(), // 整数
            Variant::I8(value) => value.to_string(), // 长整数
            Variant::R4(value) => value.to_string(), // 单精度
            Variant::R8(value) => value.to_string(), // 浮点
            Variant::Bool(flag) => flag.to_string(), // 布尔
            Variant::Dispatch(_) => String::new(), // 对象
            Variant::Array(_) => String::new(), // 数组
            Variant::R8Array(_) => String::new(), // 数值数组
        } // 结束
    } // 结束
    pub fn as_bool(&self) -> bool { // 取布尔
        match self { // 分派
            Variant::Bool(flag) => *flag, // 布尔
            Variant::I4(value) => *value != 0, // 整数
            Variant::I8(value) => *value != 0, // 长整数
            _ => false, // 其余
        }
    }
    pub fn as_i32(&self) -> i32 { // 取整数
        match self { // 分派
            Variant::I4(value) => *value, // I4
            Variant::I8(value) => *value as i32, // I8
            Variant::R4(value) => *value as i32, // R4
            Variant::R8(value) => *value as i32, // R8
            Variant::Bool(flag) => if *flag { -1 } else { 0 }, // BOOL
            Variant::Str(text) => text.parse().unwrap_or(0), // 字符串解析
            _ => 0, // 其余
        }
    }
    pub fn as_isize(&self) -> isize { // 取指针大小整数（窗口句柄 HWND）
        match self {
            Variant::I4(value) => *value as isize,
            Variant::I8(value) => *value as isize,
            Variant::R4(value) => *value as isize,
            Variant::R8(value) => *value as isize,
            Variant::Str(text) => text.trim().parse().unwrap_or(0),
            _ => 0,
        }
    }
    pub fn as_f64(&self) -> f64 { // 取浮点
        match self { // 分派
            Variant::R8(value) => *value, // R8
            Variant::R4(value) => *value as f64, // R4
            Variant::I4(value) => *value as f64, // I4
            Variant::I8(value) => *value as f64, // I8
            Variant::Str(text) => text.parse().unwrap_or(0.0), // 字符串解析
            _ => 0.0, // 其余
        }
    }
    pub fn as_dispatch(&self) -> ComResult<ComObj> { // 取 COM 对象
        match self { // 分派
            Variant::Dispatch(obj) => Ok(obj.clone()), // 对象
            _ => Err(ToolError::new("COM_ERROR", "期望 COM 对象但收到其它类型")), // 类型不匹配
        }
    }
    pub fn to_json(&self) -> Value { // 转 JSON
        match self { // 分派
            Variant::Empty => Value::Null, // 空
            Variant::I4(value) => json!(value), // I4
            Variant::I8(value) => json!(value), // I8
            Variant::R4(value) => json!(value), // R4
            Variant::R8(value) => json!(value), // R8
            Variant::Bool(flag) => json!(flag), // 布尔
            Variant::Str(text) => json!(text), // 字符串
            Variant::Dispatch(_) => json!(null), // 对象不可序列化
            Variant::Array(items) => json!(items), // 数组
            Variant::R8Array(items) => json!(items), // 数值数组
        }
    }
}
pub fn dispatch_command(cmd: &str, args: &serde_json::Map<String, Value>) -> Value { // 工具总入口（STA 线程执行）
    match crate::tools::dispatch(cmd, args) { // 分派到 12 工具
        Some(value) => value, // 命中
        None => crate::word::report::err_json(&ToolError::new("NOT_IMPLEMENTED", format!("未知工具 {cmd}"))), // 未知
    }
}
pub fn get(obj: &ComObj, name: &str) -> ComResult<Variant> { // 属性读
    invoke(obj, name, Flags::Get, &[]) // 委托
}
pub fn set(obj: &ComObj, name: &str, value: Variant) -> ComResult<()> { // 属性写
    invoke(obj, name, Flags::Put, &[value]).map(|_| ()) // 委托
}
pub fn get_args(obj: &ComObj, name: &str, args: &[Variant]) -> ComResult<Variant> { // 参数化属性读取（Range/Item/Cell）
    invoke(obj, name, Flags::Get, args) // 委托
}
pub fn range(doc: &ComObj, start: i32, end: i32) -> ComResult<ComObj> { // 用 Content 偏移构造 Range（规避 Document.Range 异常）
    let content = get(doc, "Content")?.as_dispatch()?; // 正文
    let max_end = get(&content, "End")?.as_i32().max(1); // 末尾位置
    let start = start.clamp(0, max_end); // 起点
    let end = end.clamp(start, max_end); // 终点
    set(&content, "Start", Variant::I4(start))?; // 设置起点
    set(&content, "End", Variant::I4(end))?; // 设置终点
    Ok(content) // 返回
}
pub fn call_named(obj: &ComObj, name: &str, named: &[(&str, Variant)], args: &[Variant]) -> ComResult<Variant> { // 命名参数方法调用
    invoke_named(obj, name, Flags::Method, named, args) // 委托
}
fn invoke_named(obj: &ComObj, name: &str, flags: Flags, named: &[(&str, Variant)], args: &[Variant]) -> ComResult<Variant> { // 命名参数调用（含 COM_BUSY 退避）
    let mut backoff = [100u64, 300, 800]; // 退避序列
    for attempt in 0..4 { // 最多 4 次
        match invoke_once_named(obj, name, flags, named, args) { // 单次
            Ok(value) => return Ok(value), // 成功
            Err(err) => { // 失败
                if err.code == "COM_BUSY" && attempt < 3 { sleep(Duration::from_millis(backoff[attempt])); continue; } // 忙碌重试
                return Err(err); // 其它失败
            }
        }
    }
    Err(ToolError::new("COM_BUSY", "COM 多次忙碌仍无法完成")) // 兜底
}
fn invoke_once_named(obj: &ComObj, name: &str, flags: Flags, named: &[(&str, Variant)], args: &[Variant]) -> ComResult<Variant> { // 单次命名参数调用
    let dispid = get_id(obj, name)?; // 方法 DISPID
    let mut named_ids: Vec<i32> = Vec::with_capacity(named.len()); // 命名参数 DISPID
    let mut raw_named: Vec<VARIANT> = Vec::with_capacity(named.len()); // 命名参数（位于 rgvarg 前部）
    for (key, value) in named { named_ids.push(get_id(obj, key)?); raw_named.push(value.as_raw()); } // 逐个解析
    let mut raw_pos: Vec<VARIANT> = args.iter().rev().map(|arg| arg.as_raw()).collect(); // 位置参数逆序
    raw_named.append(&mut raw_pos); // 命名参数在前，位置参数在后
    let mut params = DISPPARAMS { // 参数描述
        rgvarg: raw_named.as_ptr() as *mut VARIANT, // 参数数组
        rgdispidNamedArgs: named_ids.as_mut_ptr(), // 命名参数 DISPID
        cArgs: raw_named.len() as u32, // 参数数量
        cNamedArgs: named_ids.len() as u32, // 命名参数数量
    };
    let iid_null = GUID::zeroed(); // IID_NULL
    let mut result = VARIANT::default(); // 结果
    let mut excep = EXCEPINFO::default(); // 异常
    let mut arg_err = 0u32; // 参数错误索引
    let hr = unsafe { obj.disp.Invoke(dispid, &iid_null, LOCALE_USER_DEFAULT, flags.value(), &params, Some(&mut result), Some(&mut excep), Some(&mut arg_err)) }; // 调用
    if let Err(err) = hr { return Err(map_hresult(&err, name)); } // 映射
    let value = Variant::from_raw(&result)?; // 转回
    unsafe { let _ = windows::Win32::System::Variant::VariantClear(&mut result); } // 释放结果
    for raw in raw_named.iter_mut() { unsafe { let _ = windows::Win32::System::Variant::VariantClear(raw); } } // 释放输入
    Ok(value) // 返回
}
pub fn call(obj: &ComObj, name: &str, args: &[Variant]) -> ComResult<Variant> { // 方法调用
    invoke(obj, name, Flags::Method, args) // 委托
}
pub fn invoke(obj: &ComObj, name: &str, flags: Flags, args: &[Variant]) -> ComResult<Variant> { // IDispatch 晚绑定调用（含 COM_BUSY 退避）
    let mut backoff = [100u64, 300, 800]; // 退避序列
    for attempt in 0..4 { // 最多 4 次（首次 + 3 退避）
        match invoke_once(obj, name, flags, args) { // 单次调用
            Ok(value) => return Ok(value), // 成功
            Err(err) => { // 失败
                if err.code == "COM_BUSY" && attempt < 3 { // 忙碌可重试
                    sleep(Duration::from_millis(backoff[attempt])); // 退避
                    continue; // 重试
                }
                return Err(err); // 不可重试直接返回
            }
        }
    }
    Err(ToolError::new("COM_BUSY", "COM 多次忙碌仍无法完成")) // 兜底
}
fn invoke_once(obj: &ComObj, name: &str, flags: Flags, args: &[Variant]) -> ComResult<Variant> { // 单次 IDispatch 调用
    let dispid = get_id(obj, name)?; // 取 DISPID
    let mut raw_args: Vec<VARIANT> = args.iter().rev().map(|arg| arg.as_raw()).collect(); // 参数转原生（COM rgvarg 逆序）
    let mut params = DISPPARAMS { // 参数描述
        rgvarg: raw_args.as_ptr() as *mut VARIANT, // 参数数组
        rgdispidNamedArgs: std::ptr::null_mut(), // 命名参数
        cArgs: raw_args.len() as u32, // 参数数量
        cNamedArgs: 0, // 命名参数数量
    };
    let mut named = [DISPID_PROPERTYPUT]; // 属性写命名参数
    if matches!(flags, Flags::Put) { // 属性写
        params.rgdispidNamedArgs = named.as_mut_ptr(); // 绑定命名参数
        params.cNamedArgs = 1; // 一个命名参数
    }
    let iid_null = GUID::zeroed(); // IID_NULL
    let mut result = VARIANT::default(); // 结果容器
    let mut excep = EXCEPINFO::default(); // 异常信息
    let mut arg_err = 0u32; // 参数错误索引
    let hr = unsafe { // 执行 Invoke
        obj.disp.Invoke(dispid, &iid_null, LOCALE_USER_DEFAULT, flags.value(), &params, Some(&mut result), Some(&mut excep), Some(&mut arg_err)) // 调用
    };
    if let Err(err) = hr { return Err(map_hresult(&err, name)); } // HRESULT 映射
    let value = Variant::from_raw(&result)?; // 转回 VARIANT
    unsafe { let _ = windows::Win32::System::Variant::VariantClear(&mut result); } // 释放结果 BSTR
    for raw in raw_args.iter_mut() { unsafe { let _ = windows::Win32::System::Variant::VariantClear(raw); } } // 释放输入 SAFEARRAY/BSTR
    Ok(value) // 返回
}
fn get_id(obj: &ComObj, name: &str) -> ComResult<i32> { // GetIDsOfNames 取 DISPID
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect(); // UTF-16
    let names = [windows::core::PCWSTR::from_raw(wide.as_ptr())]; // 名称指针
    let iid_null = GUID::zeroed(); // IID_NULL
    let mut dispid = 0i32; // 接收 DISPID
    let hr = unsafe { // 执行查询
        obj.disp.GetIDsOfNames(&iid_null, names.as_ptr(), 1, LOCALE_USER_DEFAULT, &mut dispid) // 调用
    };
    if let Err(err) = hr { // 失败
        if err.code().0 == (0x80020003u32 as i32) { return Err(ToolError::new("COM_MEMBER_NOT_FOUND", format!("Office 不支持成员 {name}"))); } // 成员不存在
        return Err(map_hresult(&err, name)); // 映射
    }
    Ok(dispid) // 返回
}
pub fn map_hresult(err: &windows::core::Error, op: &str) -> ToolError { // HRESULT → 错误码
    let code = err.code().0 as u32; // 取 HRESULT
    match code { // 分类
        0x800401F3 => ToolError::new("APP_NOT_INSTALLED", format!("程序未安装：{op}")), // ProgID 未注册
        0x8001010A | 0x80010001 => ToolError::new("COM_BUSY", format!("Office 忙碌：{op}")).recoverable("稍后重试"), // 忙碌
        0x80010108 | 0x800706BA => ToolError::new("APP_GONE", format!("Office 已断开：{op}")).recoverable("工具将重建会话"), // 断开
        0x80020003 => ToolError::new("COM_MEMBER_NOT_FOUND", format!("成员不存在：{op}")), // 成员不存在
        code if code >= 0x800A0000 && code <= 0x800AFFFF => ToolError::new("OFFICE_ERROR", format!("Office 错误 {code:#x}：{op}")), // Office 业务错误
        _ => ToolError::new("COM_ERROR", format!("COM 错误 {code:#x}：{op}")), // 通用错误
    }
}
pub fn com_error(op: &str, hresult: i32) -> ToolError { // 从 HRESULT 整数构造错误
    let code = hresult as u32; // 转无符号
    match code { // 分类
        0x800401F3 => ToolError::new("APP_NOT_INSTALLED", format!("程序未安装：{op}")), // ProgID 未注册
        0x8001010A | 0x80010001 => ToolError::new("COM_BUSY", format!("Office 忙碌：{op}")).recoverable("稍后重试"), // 忙碌
        0x80010108 | 0x800706BA => ToolError::new("APP_GONE", format!("Office 已断开：{op}")).recoverable("重试"), // 断开
        code if code >= 0x800A0000 && code <= 0x800AFFFF => ToolError::new("OFFICE_ERROR", format!("Office 错误 {code:#x}：{op}")), // Office 错误
        _ => ToolError::new("COM_ERROR", format!("COM 错误 {code:#x}：{op}")), // 通用
    }
}
#[cfg(test)] // 单元测试
mod tests { // 测试模块
    use super::*; // 引入被测项
    #[test] // HRESULT 映射
    fn maps_hresult() { // 验证分类
        assert_eq!(com_error("op", 0x8001010Au32 as i32).code, "COM_BUSY"); // 忙碌
        assert_eq!(com_error("op", 0x80010108u32 as i32).code, "APP_GONE"); // 断开
        assert_eq!(com_error("op", 0x80020003u32 as i32).code, "COM_ERROR"); // 通用（com_error 不特判成员）
        assert_eq!(com_error("op", 0x800A0001u32 as i32).code, "OFFICE_ERROR"); // Office 错误
    }
    #[test] // JSON 转换
    fn json_roundtrip() { // 验证类型映射
        assert!(matches!(Variant::from_json(&json!(5)).unwrap(), Variant::I4(5))); // 整数
        assert!(matches!(Variant::from_json(&json!(true)).unwrap(), Variant::Bool(true))); // 布尔
        assert!(matches!(Variant::from_json(&json!("abc")).unwrap(), Variant::Str(_))); // 字符串
        assert!(matches!(Variant::from_json(&Value::Null).unwrap(), Variant::Empty)); // 空
    }
    #[test] // I4 原生往返
    fn i4_raw_roundtrip() { // 验证 union 读写
        let variant = Variant::I4(42); // 构造
        let raw = variant.as_raw(); // 转原生
        let back = Variant::from_raw(&raw).unwrap(); // 转回
        assert_eq!(back.as_i32(), 42); // 断言
    }
}
