//! Windows' own Open dialog, to pick a program or shortcut for the Dock.
//!
//! The dialog runs a modal loop on the UI thread (an OLE STA initialised by
//! app.rs), so the app calls it as deferred work, never while it is borrowed
//! (`app::Deferred::PickDockApp`). It opens on the Desktop the first time and
//! where the person last picked from after that.

use windows::core::{w, HSTRING};
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FileOpenDialog, IFileOpenDialog, IShellItem, SHCreateItemFromParsingName, SHGetKnownFolderPath, FOLDERID_Desktop, FOS_FILEMUSTEXIST,
    FOS_FORCEFILESYSTEM, FOS_NODEREFERENCELINKS, KF_FLAG_DEFAULT, SIGDN_FILESYSPATH,
};

/// Ask for one program or shortcut. `None` when the person cancels (or the
/// dialog can't open; that is logged).
pub fn pick_program(owner: HWND) -> Option<String> {
    match unsafe { show(owner) } {
        Ok(path) => path,
        Err(e) => {
            crate::pwarn!("dock: the Open dialog failed ({e})");
            None
        }
    }
}

unsafe fn show(owner: HWND) -> windows::core::Result<Option<String>> {
    let dlg: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
    // A shortcut stays a shortcut (its arguments and start folder come with
    // it): CBHub.lnk, not the wscript.exe it points at.
    dlg.SetOptions(dlg.GetOptions()? | FOS_FILEMUSTEXIST | FOS_FORCEFILESYSTEM | FOS_NODEREFERENCELINKS)?;
    let filters = [
        COMDLG_FILTERSPEC { pszName: w!("Programs and shortcuts"), pszSpec: w!("*.exe;*.lnk;*.bat;*.cmd;*.appref-ms") },
        COMDLG_FILTERSPEC { pszName: w!("All files"), pszSpec: w!("*.*") },
    ];
    dlg.SetFileTypes(&filters)?;
    dlg.SetTitle(w!("Add to the Dock"))?;
    dlg.SetOkButtonLabel(w!("Add"))?;
    if let Some(desktop) = desktop_item() {
        let _ = dlg.SetDefaultFolder(&desktop);
    }
    // Cancel comes back as an error (ERROR_CANCELLED): nothing picked.
    if dlg.Show(owner).is_err() {
        return Ok(None);
    }
    let name = dlg.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
    let path = name.to_string().ok();
    CoTaskMemFree(Some(name.0 as *const _));
    Ok(path.filter(|p| !p.is_empty()))
}

/// The Desktop folder (wherever OneDrive or the person moved it).
unsafe fn desktop_item() -> Option<IShellItem> {
    let p = SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, HANDLE::default()).ok()?;
    let s = p.to_string().ok();
    CoTaskMemFree(Some(p.0 as *const _));
    SHCreateItemFromParsingName(&HSTRING::from(s?), None).ok()
}
