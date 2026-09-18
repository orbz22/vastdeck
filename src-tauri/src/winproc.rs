//! Thin Win32 helpers: is a pid still alive, and where is its terminal window.
//!
//! The registry in `~/.claude/sessions` is keyed by pid, and Windows recycles
//! pids, so a stale file can point at an unrelated process. Every check here
//! therefore also compares the process creation time against the `procStart`
//! the CLI recorded.

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, HWND, LPARAM};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowTextLengthW, GetWindowThreadProcessId, IsIconic,
        IsWindowVisible, SetForegroundWindow, ShowWindow, GW_OWNER, SW_RESTORE,
    };

    fn filetime_to_u64(ft: FILETIME) -> u64 {
        ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
    }

    /// Process creation time as a FILETIME, which is exactly what the CLI stores
    /// in `procStart`.
    pub fn process_start_time(pid: u32) -> Option<u64> {
        unsafe {
            let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return None;
            }
            let mut creation = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
            let mut exit = creation;
            let mut kernel = creation;
            let mut user = creation;
            let ok = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user);
            CloseHandle(handle);
            (ok != 0).then(|| filetime_to_u64(creation))
        }
    }

    /// True only if the pid is live *and*, when `expected_start` is given, it is
    /// the same process the registry recorded rather than a recycled pid.
    pub fn is_alive(pid: u32, expected_start: Option<u64>) -> bool {
        match (process_start_time(pid), expected_start) {
            (Some(actual), Some(expected)) => actual == expected,
            (Some(_), None) => true,
            (None, _) => false,
        }
    }

    pub fn parent_pid(pid: u32) -> Option<u32> {
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot.is_null() {
                return None;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut found = None;
            if Process32FirstW(snapshot, &mut entry) != 0 {
                loop {
                    if entry.th32ProcessID == pid {
                        found = Some(entry.th32ParentProcessID);
                        break;
                    }
                    if Process32NextW(snapshot, &mut entry) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snapshot);
            found
        }
    }


    struct Search {
        pid: u32,
        hwnd: HWND,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
        let search = &mut *(lparam as *mut Search);
        let mut owner = 0u32;
        GetWindowThreadProcessId(hwnd, &mut owner);
        if owner != search.pid {
            return 1;
        }
        // Only top-level, visible, titled windows are worth raising.
        if IsWindowVisible(hwnd) == 0
            || !GetWindow(hwnd, GW_OWNER).is_null()
            || GetWindowTextLengthW(hwnd) == 0
        {
            return 1;
        }
        search.hwnd = hwnd;
        0
    }

    fn main_window_of(pid: u32) -> Option<HWND> {
        let mut search = Search { pid, hwnd: std::ptr::null_mut() };
        unsafe {
            EnumWindows(Some(enum_proc), &mut search as *mut Search as LPARAM);
        }
        (!search.hwnd.is_null()).then_some(search.hwnd)
    }

    unsafe fn raise(hwnd: HWND) -> bool {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd) != 0
    }

    /// Raises the terminal hosting `pid`. The CLI process itself owns no window —
    /// its console host or Windows Terminal does — so walk up the process tree
    /// until a window turns up.
    ///
    /// Shells inside Windows Terminal are the exception: ConPTY gives them a
    /// fake parent, so the chain walk stops at a pid that owns no window even
    /// though a perfectly good terminal window is open on screen. When the walk
    /// comes up empty, raise any Windows Terminal window — not necessarily the
    /// right tab, but far better than reporting the window as gone.
    pub fn focus_window_for_pid(pid: u32) -> bool {
        let mut current = pid;
        for _ in 0..6 {
            if let Some(hwnd) = main_window_of(current) {
                return unsafe { raise(hwnd) };
            }
            match parent_pid(current) {
                Some(parent) if parent != 0 && parent != current => current = parent,
                _ => break,
            }
        }
        focus_any_windows_terminal()
    }

    fn exe_name_of(entry: &PROCESSENTRY32W) -> String {
        let len = entry
            .szExeFile
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(entry.szExeFile.len());
        String::from_utf16_lossy(&entry.szExeFile[..len])
    }

    fn focus_any_windows_terminal() -> bool {
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot.is_null() {
                return false;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut raised = false;
            if Process32FirstW(snapshot, &mut entry) != 0 {
                loop {
                    if exe_name_of(&entry).eq_ignore_ascii_case("WindowsTerminal.exe") {
                        if let Some(hwnd) = main_window_of(entry.th32ProcessID) {
                            raised = raise(hwnd);
                            break;
                        }
                    }
                    if Process32NextW(snapshot, &mut entry) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snapshot);
            raised
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn process_start_time(_pid: u32) -> Option<u64> {
        None
    }
    pub fn is_alive(_pid: u32, _expected_start: Option<u64>) -> bool {
        false
    }
    pub fn parent_pid(_pid: u32) -> Option<u32> {
        None
    }
    pub fn focus_window_for_pid(_pid: u32) -> bool {
        false
    }
}

pub use imp::{focus_window_for_pid, is_alive};
