#![no_main]
#![no_std]
#![feature(error_in_core)]
#![feature(proc_macro_hygiene)]

extern crate alloc;

use alloc::{boxed::Box, string::String};
use core::{error::Error, fmt::Write, hint::spin_loop, time::Duration};

use rquickjs::{
    loader::{BuiltinLoader, BuiltinResolver, FileResolver, ModuleLoader, ScriptLoader},
    module::ModuleDef,
    Context, Ctx, Function, Module, Runtime,
};
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

struct VexSdk;

impl ModuleDef for VexSdk {
    fn declare(decl: &rquickjs::module::Declarations) -> rquickjs::Result<()> {
        decl.declare("vexDisplayRectFill")?;
        Ok(())
    }

    fn evaluate<'js>(
        ctx: &Ctx<'js>,
        exports: &rquickjs::module::Exports<'js>,
    ) -> rquickjs::Result<()> {
        exports.export(
            "vexDisplayRectFill",
            Function::new(ctx.clone(), |x1: i32, y1: i32, x2: i32, y2: i32| unsafe {
                vex_sdk::vexDisplayRectFill(x1, y1, x2, y2);
            }),
        )?;
        Ok(())
    }
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
