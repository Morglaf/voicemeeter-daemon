use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LMENU, VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

const SUPPRESS_WINDOW_MS: u64 = 600;

static HOOK_ENABLED: AtomicBool = AtomicBool::new(false);
static HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);
static HOOK_REFCOUNT: AtomicU32 = AtomicU32::new(0);
/// Deadline epoch-ms ; 0 = pas de suppression. Évite un Mutex dans le hot path.
static SUPPRESS_UNTIL_MS: AtomicU64 = AtomicU64::new(0);
static KEY_CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);
static CAPTURE_ACQUIRED: AtomicBool = AtomicBool::new(false);
static CAPTURE_RESULT_VK: AtomicU64 = AtomicU64::new(0);
static CAPTURE_PENDING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Default)]
pub struct CapturedKeys {
    pub vk: u16,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

/// Garde un acquire du hook (capture GUI).
#[allow(dead_code)]
pub struct KeyboardHookGuard;

impl KeyboardHookGuard {
    #[allow(dead_code)]
    pub fn acquire() -> Option<Self> {
        retain()?;
        Some(Self)
    }
}

impl Drop for KeyboardHookGuard {
    fn drop(&mut self) {
        release();
    }
}

/// Installe le hook LL si besoin (refcount).
pub fn retain() -> Option<()> {
    let prev = HOOK_REFCOUNT.fetch_add(1, Ordering::SeqCst);
    if prev == 0 {
        if !install_hook() {
            HOOK_REFCOUNT.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
    }
    Some(())
}

pub fn release() {
    let prev = HOOK_REFCOUNT.fetch_sub(1, Ordering::SeqCst);
    if prev == 1 {
        uninstall_hook();
    }
}

fn install_hook() -> bool {
    unsafe {
        match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0) {
            Ok(hook) => {
                HOOK_HANDLE.store(hook.0 as isize, Ordering::SeqCst);
                HOOK_ENABLED.store(true, Ordering::SeqCst);
                true
            }
            Err(_) => false,
        }
    }
}

fn uninstall_hook() {
    HOOK_ENABLED.store(false, Ordering::SeqCst);
    KEY_CAPTURE_ACTIVE.store(false, Ordering::SeqCst);
    let raw = HOOK_HANDLE.swap(0, Ordering::SeqCst);
    SUPPRESS_UNTIL_MS.store(0, Ordering::SeqCst);
    if raw != 0 {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(raw as _));
        }
    }
}

pub fn begin_key_capture() {
    if !CAPTURE_ACQUIRED.swap(true, Ordering::SeqCst) {
        let _ = retain();
    }
    CAPTURE_PENDING.store(false, Ordering::SeqCst);
    CAPTURE_RESULT_VK.store(0, Ordering::SeqCst);
    KEY_CAPTURE_ACTIVE.store(true, Ordering::SeqCst);
}

fn release_capture_acquire() {
    KEY_CAPTURE_ACTIVE.store(false, Ordering::SeqCst);
    CAPTURE_PENDING.store(false, Ordering::SeqCst);
    if CAPTURE_ACQUIRED.swap(false, Ordering::SeqCst) {
        release();
    }
}

pub fn cancel_key_capture() {
    release_capture_acquire();
}

pub fn poll_key_capture() -> Option<CapturedKeys> {
    if !CAPTURE_PENDING.swap(false, Ordering::SeqCst) {
        return None;
    }
    let packed = CAPTURE_RESULT_VK.load(Ordering::SeqCst);
    let vk = (packed & 0xFFFF) as u16;
    let ctrl = packed & (1 << 16) != 0;
    let alt = packed & (1 << 17) != 0;
    let shift = packed & (1 << 18) != 0;
    release_capture_acquire();
    Some(CapturedKeys {
        vk,
        ctrl,
        alt,
        shift,
    })
}

/// Bloque brièvement les touches média OS après une activité du mini-clavier.
pub fn suppress_os_media(briefly: bool) {
    if !briefly {
        return;
    }
    SUPPRESS_UNTIL_MS.store(now_ms().saturating_add(SUPPRESS_WINDOW_MS), Ordering::Relaxed);
}

pub fn is_modifier_vk(vk: u16) -> bool {
    matches!(vk, 0x10 | 0x11 | 0x12 | 0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA4 | 0xA5)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // Chemin ultra-court : si inactif, passer tout de suite.
    if code == HC_ACTION as i32 && HOOK_ENABLED.load(Ordering::Relaxed) {
        let msg = wparam.0 as u32;
        if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
            let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            // Ne jamais bloquer nos propres SendInput — sinon le mini-clavier "capture" mais n'injecte rien.
            const LLKHF_INJECTED: u32 = 0x0000_0010;
            if kb.flags.0 & LLKHF_INJECTED != 0 {
                return CallNextHookEx(hook_handle(), code, wparam, lparam);
            }
            let vk = kb.vkCode as u16;

            if KEY_CAPTURE_ACTIVE.load(Ordering::Relaxed) {
                if !is_modifier_vk(vk) {
                    let mut packed = vk as u64;
                    if modifier_down(VK_CONTROL.0 as i32) {
                        packed |= 1 << 16;
                    }
                    if modifier_down(VK_MENU.0 as i32) || modifier_down(VK_LMENU.0 as i32) {
                        packed |= 1 << 17;
                    }
                    if modifier_down(VK_SHIFT.0 as i32) {
                        packed |= 1 << 18;
                    }
                    CAPTURE_RESULT_VK.store(packed, Ordering::SeqCst);
                    CAPTURE_PENDING.store(true, Ordering::SeqCst);
                    KEY_CAPTURE_ACTIVE.store(false, Ordering::SeqCst);
                }
                return LRESULT(1);
            }

            if is_blockable_media_vk(vk) && should_suppress_now() {
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(hook_handle(), code, wparam, lparam)
}

fn hook_handle() -> Option<HHOOK> {
    let hook_raw = HOOK_HANDLE.load(Ordering::Relaxed);
    if hook_raw == 0 {
        None
    } else {
        Some(HHOOK(hook_raw as _))
    }
}

fn should_suppress_now() -> bool {
    let until = SUPPRESS_UNTIL_MS.load(Ordering::Relaxed);
    until != 0 && now_ms() < until
}

fn is_blockable_media_vk(vk: u16) -> bool {
    matches!(vk, 0xAD | 0xAE | 0xAF | 0xB0 | 0xB1 | 0xB3)
}

fn modifier_down(vk: i32) -> bool {
    unsafe { GetAsyncKeyState(vk) as u16 & 0x8000 != 0 }
}
