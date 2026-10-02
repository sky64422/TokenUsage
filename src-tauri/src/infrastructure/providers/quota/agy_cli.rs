//! ConPTY quota transport adapted from CodeNotch (MIT); see third-party/codenotch.
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

/// Discover installed official Antigravity CLI (`agy.exe`).
/// Checks `%LOCALAPPDATA%\agy\bin\agy.exe` and `PATH` only (only `.exe` binaries).
pub fn find_agy() -> Option<PathBuf> {
    if let Some(local) = dirs::data_local_dir() {
        let candidate = local.join("agy").join("bin").join("agy.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Some(path_var) = std::env::var_os("PATH") {
        return find_agy_in(&std::env::split_paths(&path_var).filter(|p| p.is_absolute()).collect::<Vec<_>>());
    }
    None
}

/// Helper for testing discovery in explicit directories without touching environment.
fn find_agy_in(dirs: &[PathBuf]) -> Option<PathBuf> {
    for dir in dirs {
        let candidate = dir.join("agy.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Strips ANSI escape sequences (CSI, OSC, 2-character escapes) and normalizes line endings.
pub(super) fn sanitize_terminal_output(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.peek() {
                Some(&'[') => {
                    chars.next();
                    // CSI sequence: consumes parameter and intermediate bytes, ends with 0x40..=0x7E
                    while let Some(&next) = chars.peek() {
                        chars.next();
                        if ('\x40'..='\x7e').contains(&next) {
                            break;
                        }
                    }
                }
                Some(&']') => {
                    chars.next();
                    // OSC sequence: consumes until BEL (\x07) or ST (\x1b\\)
                    while let Some(next) = chars.next() {
                        if next == '\x07' {
                            break;
                        }
                        if next == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                Some(&next) if ('\x40'..='\x5f').contains(&next) => {
                    // 2-character escape sequence Fe
                    chars.next();
                }
                _ => {}
            }
        } else if c == '\r' {
            if chars.peek() == Some(&'\n') {
                // CRLF -> keep \n on next iteration
                continue;
            } else {
                out.push('\n');
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Quotes a command line argument according to Windows CommandLineToArgvW rules.
fn quote_arg(arg: &str) -> String {
    if arg.is_empty() {
        return "\"\"".to_string();
    }
    if !arg.contains([' ', '\t', '\n', '\x0b', '\"']) {
        return arg.to_string();
    }
    let mut res = String::with_capacity(arg.len() + 2);
    res.push('"');
    let mut backslashes = 0;
    for c in arg.chars() {
        if c == '\\' {
            backslashes += 1;
        } else {
            for _ in 0..(if c == '"' {backslashes * 2 + 1} else {backslashes}) { res.push('\\'); }
            backslashes = 0;
            res.push(c);
        }
    }
    for _ in 0..backslashes * 2 {
        res.push('\\');
    }
    res.push('"');
    res
}

/// Spawns a hidden process connected to a native Windows ConPTY (Pseudo Console) inside a JobObject.
/// Drains up to 64 KB of stdout concurrently, enforces timeout, and cleans up all descendants.
#[cfg(windows)]
pub(super) fn run_cmd_conpty(
    program: &Path,
    args: &[&str],
    cwd: Option<&Path>,
    timeout: Duration,
) -> Result<String, String> {
    use std::io::Read;
    use std::os::windows::io::FromRawHandle;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
    use windows::Win32::System::Console::{
        ClosePseudoConsole, CreatePseudoConsole, COORD, HPCON,
    };
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
        TerminateJobObject, JobObjectExtendedLimitInformation,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows::Win32::System::Pipes::CreatePipe;
    use windows::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
        InitializeProcThreadAttributeList, ResumeThread, UpdateProcThreadAttribute,
        WaitForSingleObject, CREATE_SUSPENDED,
        EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST,
        PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, STARTUPINFOEXW,
        STARTF_USESTDHANDLES, STARTF_USESHOWWINDOW,
    };

    if !program.is_file() {
        return Err(format!("Program not found: {}", program.display()));
    }

    struct Job(HANDLE);
    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    let job = unsafe {
        let h = CreateJobObjectW(None, PCWSTR::null())
            .map_err(|e| format!("Cannot create CLI process job: {e}"))?;
        let job = Job(h);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            std::mem::size_of_val(&limits) as u32,
        )
        .map_err(|e| format!("Cannot configure CLI process job limits: {e}"))?;
        job
    };

    let mut in_read = HANDLE::default();
    let mut in_write = HANDLE::default();
    let mut out_read = HANDLE::default();
    let mut out_write = HANDLE::default();

    unsafe {
        CreatePipe(&mut in_read, &mut in_write, None, 0)
            .map_err(|e| format!("CreatePipe(in) failed: {e}"))?;
        if let Err(e) = CreatePipe(&mut out_read, &mut out_write, None, 0) {
            let _ = CloseHandle(in_read);
            let _ = CloseHandle(in_write);
            return Err(format!("CreatePipe(out) failed: {e}"));
        }
    }

    let console_size = COORD { X: 160, Y: 60 };
    let hpc_res = unsafe { CreatePseudoConsole(console_size, in_read, out_write, 0) };

    unsafe {
        let _ = CloseHandle(in_read);
        let _ = CloseHandle(out_write);
    }

    let hpc = match hpc_res {
        Ok(h) => h,
        Err(e) => {
            unsafe {
                let _ = CloseHandle(in_write);
                let _ = CloseHandle(out_read);
            }
            return Err(format!("CreatePseudoConsole failed: {e}"));
        }
    };

    struct PseudoConsoleGuard(HPCON);
    impl Drop for PseudoConsoleGuard {
        fn drop(&mut self) {
            unsafe {
                ClosePseudoConsole(self.0);
            }
        }
    }
    let _input_guard = Job(in_write);
    let pty_guard = PseudoConsoleGuard(hpc);
    let mut file = unsafe { std::fs::File::from_raw_handle(out_read.0 as _) };
    let reader_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        while let Ok(n) = file.read(&mut chunk) {
            if n == 0 { break; }
            let keep = n.min(65537usize.saturating_sub(buf.len()));
            buf.extend_from_slice(&chunk[..keep]);
        }
        buf
    });

    let mut attr_size = 0usize;
    let _ = unsafe {
        InitializeProcThreadAttributeList(
            None,
            1,
            None,
            &mut attr_size,
        )
    };

    let mut attr_storage = vec![0usize; attr_size.div_ceil(std::mem::size_of::<usize>())];
    let attr_list = LPPROC_THREAD_ATTRIBUTE_LIST(attr_storage.as_mut_ptr() as *mut _);

    struct AttrListGuard(LPPROC_THREAD_ATTRIBUTE_LIST);
    impl Drop for AttrListGuard {
        fn drop(&mut self) {
            unsafe {
                DeleteProcThreadAttributeList(self.0);
            }
        }
    }

    let _attr_guard = unsafe {
        InitializeProcThreadAttributeList(Some(attr_list), 1, None, &mut attr_size)
            .map_err(|e| format!("InitializeProcThreadAttributeList failed: {e}"))?;
        AttrListGuard(attr_list)
    };

    unsafe {
        UpdateProcThreadAttribute(
            attr_list,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
            Some(hpc.0 as *const core::ffi::c_void),
            std::mem::size_of::<HPCON>(),
            None,
            None,
        )
        .map_err(|e| format!("UpdateProcThreadAttribute failed: {e}"))?;
    }

    let mut cmd_line_str = quote_arg(program.to_str().unwrap_or_default());
    let program_u16: Vec<u16> = program.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    for arg in args {
        cmd_line_str.push(' ');
        cmd_line_str.push_str(&quote_arg(arg));
    }
    let mut cmd_line_u16: Vec<u16> = cmd_line_str.encode_utf16().chain(std::iter::once(0)).collect();

    let cwd_u16: Option<Vec<u16>> = cwd.map(|p| p.as_os_str().encode_wide().chain(std::iter::once(0)).collect());

    let mut si_ex = STARTUPINFOEXW::default();
    si_ex.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    // Prevent inherited parent output handles from bypassing the pseudo console.
    si_ex.StartupInfo.dwFlags = STARTF_USESTDHANDLES | STARTF_USESHOWWINDOW;
    si_ex.StartupInfo.wShowWindow = 0; // SW_HIDE: this is a background quota reader.
    si_ex.lpAttributeList = attr_list;

    let mut proc_info = PROCESS_INFORMATION::default();

    let spawn_res = unsafe {
        CreateProcessW(
            PCWSTR(program_u16.as_ptr()),
            Some(PWSTR(cmd_line_u16.as_mut_ptr())),
            None,
            None,
            false,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_SUSPENDED,
            None,
            cwd_u16.as_ref().map_or(PCWSTR::null(), |v| PCWSTR(v.as_ptr())),
            &si_ex.StartupInfo,
            &mut proc_info,
        )
    };

    if let Err(e) = spawn_res {
        return Err(format!("CreateProcessW failed: {e}"));
    }

    if unsafe { AssignProcessToJobObject(job.0, proc_info.hProcess).is_err() } {
        unsafe {
            let _ = windows::Win32::System::Threading::TerminateProcess(proc_info.hProcess, 1);
            let _ = CloseHandle(proc_info.hThread);
            let _ = CloseHandle(proc_info.hProcess);
        }
        return Err("Cannot attach CLI process to job object".into());
    }

    let resumed = unsafe {
        let result = ResumeThread(proc_info.hThread);
        let _ = CloseHandle(proc_info.hThread);
        result != u32::MAX
    };
    let _process_guard = Job(proc_info.hProcess);
    if !resumed { drop(job); return Err("Cannot resume CLI process".into()); }

    let started = std::time::Instant::now();
    let mut exit_code = 0u32;
    let mut timed_out = false;

    loop {
        let wait = unsafe { WaitForSingleObject(proc_info.hProcess, 100) };
        if wait == WAIT_OBJECT_0 {
            let _ = unsafe { GetExitCodeProcess(proc_info.hProcess, &mut exit_code) };
            break;
        }
        if started.elapsed() >= timeout {
            timed_out = true;
            unsafe {
                let _ = TerminateJobObject(job.0, 1);
            }
            break;
        }
    }

    // Stop our remaining descendants before closing their console.
    drop(job);
    drop(pty_guard);

    let raw_bytes = reader_thread
        .join()
        .map_err(|_| "CLI output reader thread panicked".to_string())?;


    if timed_out {
        return Err("Antigravity CLI quota request timed out".into());
    }

    if exit_code != 0 {
        return Err(format!("Antigravity CLI failed with exit code {exit_code}"));
    }

    if raw_bytes.len() > 65536 {
        return Err("CLI quota output is too large".into());
    }

    let text = String::from_utf8_lossy(&raw_bytes);
    Ok(sanitize_terminal_output(&text))
}

#[cfg(not(windows))]
pub(super) fn run_cmd_conpty(
    _program: &Path,
    _args: &[&str],
    _cwd: Option<&Path>,
    _timeout: Duration,
) -> Result<String, String> {
    Err("Antigravity CLI runner requires Windows".into())
}

