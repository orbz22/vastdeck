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
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP, VK_MENU};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsIconic, IsWindowVisible, SetForegroundWindow, ShowWindow, GW_OWNER, SW_RESTORE,
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


    struct WindowList {
        pid: u32,
        windows: Vec<HWND>,
    }

    unsafe extern "system" fn collect_windows(hwnd: HWND, lparam: LPARAM) -> i32 {
        let list = &mut *(lparam as *mut WindowList);
        let mut owner = 0u32;
        GetWindowThreadProcessId(hwnd, &mut owner);
        if owner != list.pid {
            return 1;
        }
        // Only top-level, visible, titled windows are worth raising.
        if IsWindowVisible(hwnd) == 0
            || !GetWindow(hwnd, GW_OWNER).is_null()
            || GetWindowTextLengthW(hwnd) == 0
        {
            return 1;
        }
        list.windows.push(hwnd);
        1
    }

    /// Every raisable top-level window a process owns — Windows Terminal
    /// deliberately makes one window handle per tab, all under a single pid.
    fn windows_of(pid: u32) -> Vec<HWND> {
        let mut list = WindowList { pid, windows: Vec::new() };
        unsafe {
            EnumWindows(Some(collect_windows), &mut list as *mut WindowList as LPARAM);
        }
        list.windows
    }

    unsafe fn raise(hwnd: HWND) -> bool {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        // Windows refuses SetForegroundWindow from a process that is not the
        // foreground process itself — the call silently no-ops. Tapping a
        // modifier key first sells the click that just happened to us as the
        // user's own input, which is the standard workaround for that lock.
        keybd_event(VK_MENU as u8, 0, 0, 0);
        let ok = SetForegroundWindow(hwnd) != 0;
        keybd_event(VK_MENU as u8, 0, KEYEVENTF_KEYUP, 0);
        ok
    }

    /// Raises the terminal hosting `pid`. The CLI process itself owns no window —
    /// its console host or Windows Terminal does — so walk up the process tree
    /// until a window turns up.
    ///
    /// Shells inside Windows Terminal are the exception: ConPTY gives them a
    /// fake parent, so the chain walk stops at a pid that owns no window even
    /// though a perfectly good terminal window is open on screen. When the walk
    /// comes up empty, fall back to a Windows Terminal window — preferring one
    /// whose title carries the session's name (WT shows one top-level window
    /// per tab, titled after the running shell), else any of them.
    pub fn focus_window_for_pid(pid: u32, hint: Option<&str>) -> bool {
        let mut current = pid;
        for _ in 0..6 {
            let windows = windows_of(current);
            if windows.len() == 1 {
                return unsafe { raise(windows[0]) };
            }
            // One pid owning several raisable windows is the Windows Terminal
            // shape: every tab is its own window under one shared pid, so the
            // pid alone cannot tell which tab runs this session — pick by
            // title, else keep walking.
            if let Some(hwnd) = match_by_title(&windows, hint) {
                return unsafe { raise(hwnd) };
            }
            match parent_pid(current) {
                Some(parent) if parent != 0 && parent != current => current = parent,
                _ => break,
            }
        }
        focus_windows_terminal(hint)
    }

    /// First window whose title contains `hint`, case-insensitively.
    fn match_by_title(windows: &[HWND], hint: Option<&str>) -> Option<HWND> {
        let hint = hint.filter(|h| !h.is_empty())?;
        let lowered = hint.to_lowercase();
        windows.iter().copied().find(|hwnd| {
            window_title(*hwnd).is_some_and(|title| title.to_lowercase().contains(&lowered))
        })
    }

    fn exe_name_of(entry: &PROCESSENTRY32W) -> String {
        let len = entry
            .szExeFile
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(entry.szExeFile.len());
        String::from_utf16_lossy(&entry.szExeFile[..len])
    }

    fn focus_windows_terminal(hint: Option<&str>) -> bool {
        // Windows Terminal hosts every tab in one process but gives each tab
        // its own top-level window, so the pid list has to be collected before
        // any window enumeration can pick a specific one.
        let mut pids = Vec::new();
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot.is_null() {
                return false;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snapshot, &mut entry) != 0 {
                loop {
                    if exe_name_of(&entry).eq_ignore_ascii_case("WindowsTerminal.exe") {
                        pids.push(entry.th32ProcessID);
                    }
                    if Process32NextW(snapshot, &mut entry) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snapshot);
        }

        let mut windows: Vec<HWND> = Vec::new();
        for pid in pids {
            windows.extend(windows_of(pid));
        }
        if windows.is_empty() {
            return false;
        }

        // The tab running this session is titled after it; a name match is the
        // closest a pid-less lookup can get to the right terminal.
        if let Some(hwnd) = match_by_title(&windows, hint) {
            return unsafe { raise(hwnd) };
        }
        unsafe { raise(windows[0]) }
    }

    fn window_title(hwnd: HWND) -> Option<String> {
        unsafe {
            let len = GetWindowTextLengthW(hwnd);
            if len == 0 {
                return None;
            }
            let mut buf = vec![0u16; len as usize + 1];
            let copied = GetWindowTextW(hwnd, buf.as_mut_ptr(), len as i32 + 1);
            if copied == 0 {
                return None;
            }
            Some(String::from_utf16_lossy(&buf[..copied as usize]))
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
    pub fn focus_window_for_pid(_pid: u32, _hint: Option<&str>) -> bool {
        false
    }
}

pub use imp::{focus_window_for_pid, is_alive};
