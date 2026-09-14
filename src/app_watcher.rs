use std::sync::mpsc;
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use windows::core::PWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetWindowTextW, GetWindowThreadProcessId, PeekMessageW, TranslateMessage,
    EVENT_SYSTEM_FOREGROUND, MSG, PM_REMOVE, WINEVENT_OUTOFCONTEXT,
};

const DEBOUNCE: Duration = Duration::from_millis(500);

static EVENT_TX: Mutex<Option<mpsc::Sender<ForegroundInfo>>> = Mutex::new(None);

#[derive(Debug, Clone)]
pub struct ForegroundInfo {
    pub process_exe: String,
    pub window_title: String,
}

pub struct AppWatcher {
    stop_tx: Option<mpsc::Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl AppWatcher {
    pub fn start<F>(enabled: bool, on_change: F) -> Self
    where
        F: Fn(ForegroundInfo) + Send + 'static,
    {
        let (stop_tx, stop_rx) = mpsc::channel();
        if !enabled {
            return Self {
                stop_tx: Some(stop_tx),
                worker: None,
            };
        }

        let worker = thread::spawn(move || watcher_loop(stop_rx, on_change));
        Self {
            stop_tx: Some(stop_tx),
            worker: Some(worker),
        }
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for AppWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

fn watcher_loop(stop_rx: mpsc::Receiver<()>, on_change: impl Fn(ForegroundInfo)) {
    let (event_tx, event_rx) = mpsc::channel();
    {
        let mut guard = EVENT_TX.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(event_tx);
    }

    let hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };

    if hook.0.is_null() {
        eprintln!("app_watcher: SetWinEventHook échoué");
        let mut guard = EVENT_TX.lock().unwrap_or_else(|e| e.into_inner());
        *guard = None;
        return;
    }

    let mut last_emit = std::time::Instant::now()
        .checked_sub(DEBOUNCE)
        .unwrap_or_else(std::time::Instant::now);
    let mut last_key = String::new();

    loop {
        // WINEVENT_OUTOFCONTEXT exige une pompe de messages sur le thread du hook.
        pump_messages();

        match stop_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }

        while let Ok(info) = event_rx.try_recv() {
            let key = format!("{}|{}", info.process_exe, info.window_title);
            let now = std::time::Instant::now();
            if key == last_key {
                continue;
            }
            if now.duration_since(last_emit) < DEBOUNCE {
                continue;
            }
            last_key = key;
            last_emit = now;
            on_change(info);
        }
    }

    unsafe {
        let _ = UnhookWinEvent(hook);
    }
    if let Ok(mut guard) = EVENT_TX.lock() {
        *guard = None;
    }
}

fn pump_messages() {
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _id_obj: i32,
    _id_child: i32,
    _tid: u32,
    _time: u32,
) {
    if event != EVENT_SYSTEM_FOREGROUND || hwnd.0.is_null() {
        return;
    }
    let Some(info) = foreground_from_hwnd(hwnd) else {
        return;
    };
    if let Ok(guard) = EVENT_TX.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(info);
        }
    }
}

fn foreground_from_hwnd(hwnd: HWND) -> Option<ForegroundInfo> {
    let title = window_title(hwnd);
    let process_exe = process_for_hwnd(hwnd)?;
    Some(ForegroundInfo {
        process_exe,
        window_title: title,
    })
}

fn window_title(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
    if len <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..len as usize])
}

fn process_for_hwnd(hwnd: HWND) -> Option<String> {
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid == 0 {
        return None;
    }

    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        )
        .ok()?;
        let path = String::from_utf16_lossy(&buf[..size as usize]);
        let _ = windows::Win32::Foundation::CloseHandle(process);
        Some(
            path.rsplit(['\\', '/'])
                .next()
                .unwrap_or(&path)
                .to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    #[test]
    fn read_current_foreground() {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.0.is_null() {
            return;
        }
        let info = foreground_from_hwnd(hwnd);
        assert!(info.is_some());
    }
}
