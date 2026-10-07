//! Executable entry point for platform dependency qualification probe.
//!
//! Emits qualification verification for `winit` and `windows` bindings on Windows.

use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(windows)]
    {
        println!("============================================================");
        println!("view - M0-06 Platform Dependency Qualification Probe (Windows)");
        println!("============================================================");
        println!("Host OS:           {}", std::env::consts::OS);
        println!("Target Arch:       {}", std::env::consts::ARCH);
        println!("Recorded baseline: winit 0.30.13, windows 0.62.2, rustc 1.98.1");
        println!("(Refer to Cargo.lock and `rustc --version` for active metadata)");
        println!("------------------------------------------------------------");

        let mut failed = false;

        // Compile-time signature verification
        platform_probe::assert_win32_signatures_compile();
        println!("[PASS] Win32 / DWM typed API function signature verification");

        match platform_probe::check_layout_and_constants() {
            Ok(()) => println!("[PASS] Win32 / DWM type layout and constants check"),
            Err(e) => {
                eprintln!("[FAIL] Win32 / DWM type layout check: {e}");
                failed = true;
            }
        }

        match platform_probe::check_winit_event_loop_builder() {
            Ok(()) => println!("[PASS] winit Windows EventLoopBuilder extension check"),
            Err(e) => {
                eprintln!("[FAIL] winit EventLoopBuilder check: {e}");
                failed = true;
            }
        }

        println!("------------------------------------------------------------");
        if failed {
            eprintln!("[FAILURE] Platform probe checks encountered failures.");
            ExitCode::FAILURE
        } else {
            println!("[SUCCESS] Platform probe completed all verification checks.");
            println!("Note: Native OS window lifecycle behavior remains untested.");
            ExitCode::SUCCESS
        }
    }
    #[cfg(not(windows))]
    {
        eprintln!("============================================================");
        eprintln!("view - M0-06 Platform Dependency Qualification Probe");
        eprintln!("============================================================");
        eprintln!("Host OS: {}", std::env::consts::OS);
        eprintln!("[SKIPPED] Platform qualification probe is configured for Windows.");
        eprintln!("Non-Windows platforms do not qualify Windows bindings.");
        ExitCode::from(2)
    }
}
