#![no_main]
#![no_std]
#![feature(error_in_core)]
#![feature(proc_macro_hygiene)]

extern crate alloc;

use alloc::{boxed::Box, string::String};
use core::{
    error::Error,
    ffi::c_char,
    fmt::Write,
    hint::spin_loop,
    ops::{Deref, DerefMut},
    time::Duration,
};

use rquickjs::{
    loader::{BuiltinLoader, BuiltinResolver, FileResolver, ModuleLoader, ScriptLoader},
    module::ModuleDef,
    Context, Ctx, FromJs, Function, Module, Runtime,
};
use vex_sdk::v5_image;
use vexide::prelude::*;

mod polyfill {
    use core::ffi::c_void;

    use vexide::core::{io::Write, print, println, time::Instant};

    static mut PROGRAM_START: Option<Instant> = None;

    // libc polyfills
    #[no_mangle]
    pub extern "C" fn _isatty(_fd: i32) -> i32 {
        println!("isatty");
        vexide::core::program::exit()
    }

    #[no_mangle]
    pub extern "C" fn _write(_fd: i32, buf: *const u8, count: usize) -> isize {
        let buf = unsafe { core::slice::from_raw_parts(buf, count) };
        vexide::core::io::stdout().write(buf).unwrap() as isize
    }

    #[no_mangle]
    pub extern "C" fn _lseek(_fd: i32, _offset: isize, _whence: i32) -> isize {
        println!("lseek");
        vexide::core::program::exit()
    }

    #[no_mangle]
    pub extern "C" fn _close(_fd: i32) -> i32 {
        println!("close");
        vexide::core::program::exit()
    }

    #[no_mangle]
    pub extern "C" fn _fstat(_fd: i32, _buf: *mut u8) -> i32 {
        println!("fstat");
        vexide::core::program::exit()
    }

    #[no_mangle]
    pub extern "C" fn _getpid() -> i32 {
        println!("getpid");
        vexide::core::program::exit()
    }

    #[no_mangle]
    pub extern "C" fn _kill(_pid: i32, _sig: i32) -> i32 {
        println!("kill");
        vexide::core::program::exit()
    }
    //     struct timeval {
    //         time_t       tv_sec;   /* seconds since Jan. 1, 1970 */
    //         suseconds_t  tv_usec;  /* and microseconds */
    // };

    #[repr(C)]
    pub struct timeval {
        tv_sec: i64,
        tv_usec: i64,
    }

    #[no_mangle]
    pub extern "C" fn _gettimeofday(tp: *mut timeval, tzp: *mut c_void) -> i32 {
        unsafe {
            if PROGRAM_START.is_none() {
                PROGRAM_START = Some(Instant::now());
            }

            let now = PROGRAM_START.unwrap().elapsed();
            let tv = timeval {
                tv_sec: now.as_secs() as i64,
                tv_usec: now.subsec_micros() as i64,
            };
            core::ptr::write(tp, tv);

            if !tzp.is_null() {
                println!("tzp is not null");
                vexide::core::program::exit();
            }

            0
        }
    }

    #[no_mangle]
    pub extern "C" fn _sbrk(incr: isize) -> *mut u8 {
        println!("sbrk: {}", incr);
        vexide::core::program::exit()
    }

    #[no_mangle]
    pub extern "C" fn _read(_fd: i32, _buf: *mut u8, _count: usize) -> isize {
        println!("read");
        vexide::core::program::exit()
    }

    #[no_mangle]
    pub extern "C" fn _exit(_status: i32) -> ! {
        println!("exit");
        vexide::core::program::exit();
    }
}

#[repr(transparent)]
struct Ptr<T>(*mut T);

impl<'js, T> FromJs<'js> for Ptr<T> {
    fn from_js(ctx: &Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        let int = i32::from_js(ctx, value)?;
        let ptr = usize::try_from(int)
            .map_err(|_| rquickjs::Error::new_from_js("number", "memory address"))?;
        Ok(Ptr(ptr as *mut T))
    }
}

impl<T> From<Ptr<T>> for *mut T {
    fn from(ptr: Ptr<T>) -> Self {
        ptr.0
    }
}

impl<T> From<Ptr<T>> for *const T {
    fn from(ptr: Ptr<T>) -> Self {
        ptr.0
    }
}

macro_rules! create_sdk_module {
    (
        $(
            fn $name:ident($($arg:ident: $arg_ty:ty $(,)?),*) $(-> $ret:ty)?
        ),+ $(,)?
    ) => {
        struct VexSdk;

        impl ModuleDef for VexSdk {
            fn declare(decl: &rquickjs::module::Declarations) -> rquickjs::Result<()> {
                $(
                    decl.declare(stringify!($name))?;
                )+
                Ok(())
            }

            #[allow(non_snake_case)]
            fn evaluate<'js>(
                ctx: &Ctx<'js>,
                exports: &rquickjs::module::Exports<'js>,
            ) -> rquickjs::Result<()> {
                $(
                    exports.export(
                        stringify!($name),
                        Function::new(ctx.clone(), |$($arg: $arg_ty),*| unsafe {
                            vex_sdk::$name($($arg.into(),)*)
                        }),
                    )?;
                )+
                Ok(())
            }
        }
    };
}

create_sdk_module! {
    fn vexDisplayForegroundColor(col: u32),
    fn vexDisplayBackgroundColor(col: u32),
    fn vexDisplayErase(),
    fn vexDisplayScroll(nStartLine: i32, nLines: i32),
    fn vexDisplayScrollRect(x1: i32, y1: i32, x2: i32, y2: i32, nLines: i32),
    fn vexDisplayCopyRect(x1: i32, y1: i32, x2: i32, y2: i32, pSrc: Ptr<u32>, srcStride: i32),
    fn vexDisplayPixelSet(x: u32, y: u32),
    fn vexDisplayPixelClear(x: u32, y: u32),
    fn vexDisplayLineDraw(x1: i32, y1: i32, x2: i32, y2: i32),
    fn vexDisplayLineClear(x1: i32, y1: i32, x2: i32, y2: i32),
    fn vexDisplayRectDraw(x1: i32, y1: i32, x2: i32, y2: i32),
    fn vexDisplayRectClear(x1: i32, y1: i32, x2: i32, y2: i32),
    fn vexDisplayRectFill(x1: i32, y1: i32, x2: i32, y2: i32),
    fn vexDisplayCircleDraw(xc: i32, yc: i32, radius: i32),
    fn vexDisplayCircleClear(xc: i32, yc: i32, radius: i32),
    fn vexDisplayCircleFill(xc: i32, yc: i32, radius: i32),
    fn vexDisplayTextSize(n: u32, d: u32),
    fn vexDisplayFontNamedSet(pFontName: Ptr<c_char>),
    fn vexDisplayForegroundColorGet() -> u32,
    fn vexDisplayBackgroundColorGet() -> u32,
    fn vexDisplayStringWidthGet(pString: Ptr<c_char>) -> i32,
    fn vexDisplayStringHeightGet(pString: Ptr<c_char>) -> i32,
    fn vexDisplayPenSizeSet(width: u32),
    fn vexDisplayPenSizeGet() -> u32,
    fn vexDisplayClipRegionSet(x1: i32, y1: i32, x2: i32, y2: i32),
    fn vexDisplayRender(bVsyncWait: bool, bRunScheduler: bool),
    fn vexDisplayDoubleBufferDisable(),
    fn vexDisplayClipRegionSetWithIndex(index: i32, x1: i32, y1: i32, x2: i32, y2: i32),
    fn vexImageBmpRead(ibuf: Ptr<u8>, oBuf: Ptr<v5_image>, maxw: u32, maxh: u32) -> u32,
    fn vexImagePngRead(ibuf: Ptr<u8>, oBuf: Ptr<v5_image>, maxw: u32, maxh: u32, ibuflen: u32) -> u32,
    // fn vexDisplayVPrintf(xpos: i32, ypos: i32, bOpaque: i32, format: *const c_char, args: VaList),
    // fn vexDisplayVString(nLineNumber: i32, format: *const c_char, args: VaList),
    // fn vexDisplayVStringAt(xpos: i32, ypos: i32, format: *const c_char, args: VaList),
    // fn vexDisplayVBigString(nLineNumber: i32, format: *const c_char, args: VaList),
    // fn vexDisplayVBigStringAt(xpos: i32, ypos: i32, format: *const c_char, args: VaList),
    // fn vexDisplayVSmallStringAt(xpos: i32, ypos: i32, format: *const c_char, args: VaList),
    // fn vexDisplayVCenteredString(nLineNumber: i32, format: *const c_char, args: VaList),
    // fn vexDisplayVBigCenteredString(nLineNumber: i32, format: *const c_char, args: VaList),
}

// const SCRIPT_MODULE: &str = r#"
// export const n = 123;
// export const s = "abc";
// export const f = (a, b) => (a + b) * 0.5;
// "#;

#[vexide::main]
async fn main(peripherals: Peripherals) -> Result<(), Box<dyn Error>> {
    let rt = Runtime::new().unwrap();
    let resolver = BuiltinResolver::default().with_module("vex_sdk");
    let loader = ModuleLoader::default().with_module("vex_sdk", VexSdk);
    rt.set_loader(resolver, loader);

    let ctx = Context::full(&rt).unwrap();
    ctx.with(|ctx| {
        let global = ctx.globals();
        global
            .set(
                "print",
                Function::new(ctx.clone(), |msg: String| {
                    println!("{msg}");
                })
                .unwrap(),
            )
            .unwrap();

        Module::evaluate(ctx.clone(), "test", include_bytes!("./index.js"))
            .unwrap()
            .finish::<()>()
            .unwrap();
    });

    loop {
        spin_loop();
    }
}
