//! OLE drag-and-drop: files dropped onto the island (a text file for the
//! Script page).
//!
//! It runs on the UI thread (an OLE STA initialised by app.rs): OLE calls the
//! drop target from the message loop.

use crate::events::{AppEvent, EventSink};
use std::cell::Cell;
use windows::core::{implement, Result};
use windows::Win32::Foundation::{DRAGDROP_E_NOTREGISTERED, HWND, POINT, POINTL, S_OK};
use windows::Win32::System::Com::{CoCreateInstance, IDataObject, CLSCTX_INPROC_SERVER, DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL};
use windows::Win32::System::Ole::{
    IDropTarget, IDropTarget_Impl, RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop, CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_LINK,
    DROPEFFECT_NONE, MK_ALT,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::{DragQueryFileW, IDropTargetHelper, CLSID_DragDropHelper, HDROP};

/// At most this many paths are read from one drop.
const MAX_DROPPED: usize = 64;

fn hdrop_format() -> FORMATETC {
    FORMATETC { cfFormat: CF_HDROP.0, ptd: std::ptr::null_mut(), dwAspect: DVASPECT_CONTENT.0, lindex: -1, tymed: TYMED_HGLOBAL.0 as u32 }
}

fn point(pt: &POINTL) -> POINT {
    POINT { x: pt.x, y: pt.y }
}

// ---------------------------------------------------------------- drop target

#[implement(IDropTarget)]
struct DropTarget {
    hwnd: HWND,
    sink: EventSink,
    /// Shell helper that draws the dragged items' image over our window.
    helper: Option<IDropTargetHelper>,
    /// The current drag carries files we can take.
    accept: Cell<bool>,
    /// We told the app a drag is hovering (so DragLeave/Drop must undo it).
    hovering: Cell<bool>,
}

impl DropTarget {
    /// Copy by default, link with Alt, within what the source allows. Never
    /// "move": the island only reads the file, a move would lose it.
    fn effect(&self, keys: MODIFIERKEYS_FLAGS, allowed: DROPEFFECT) -> DROPEFFECT {
        if !self.accept.get() {
            return DROPEFFECT_NONE;
        }
        let want = if keys.0 & MK_ALT != 0 { DROPEFFECT_LINK } else { DROPEFFECT_COPY };
        [want, DROPEFFECT_COPY, DROPEFFECT_LINK].into_iter().find(|e| allowed.0 & e.0 != 0).unwrap_or(DROPEFFECT_NONE)
    }

    fn set_hover(&self, on: bool) {
        if self.hovering.replace(on) != on {
            self.sink.send(AppEvent::DragHover(on));
        }
    }
}

/// Read and write the in/out `pdweffect` parameter.
fn allowed_effects(p: *mut DROPEFFECT) -> DROPEFFECT {
    if p.is_null() {
        DROPEFFECT_COPY | DROPEFFECT_LINK
    } else {
        unsafe { *p }
    }
}

fn set_effect(p: *mut DROPEFFECT, e: DROPEFFECT) {
    if !p.is_null() {
        unsafe { *p = e };
    }
}

impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(&self, pdataobj: Option<&IDataObject>, grfkeystate: MODIFIERKEYS_FLAGS, pt: &POINTL, pdweffect: *mut DROPEFFECT) -> Result<()> {
        let has_files = pdataobj.is_some_and(|d| unsafe { d.QueryGetData(&hdrop_format()) } == S_OK);
        self.accept.set(has_files);
        let effect = self.effect(grfkeystate, allowed_effects(pdweffect));
        set_effect(pdweffect, effect);
        if let (Some(h), Some(d)) = (&self.helper, pdataobj) {
            unsafe {
                let _ = h.DragEnter(self.hwnd, d, &point(pt), effect);
            }
        }
        if self.accept.get() {
            self.set_hover(true);
        }
        Ok(())
    }

    fn DragOver(&self, grfkeystate: MODIFIERKEYS_FLAGS, pt: &POINTL, pdweffect: *mut DROPEFFECT) -> Result<()> {
        let effect = self.effect(grfkeystate, allowed_effects(pdweffect));
        set_effect(pdweffect, effect);
        if let Some(h) = &self.helper {
            unsafe {
                let _ = h.DragOver(&point(pt), effect);
            }
        }
        Ok(())
    }

    fn DragLeave(&self) -> Result<()> {
        if let Some(h) = &self.helper {
            unsafe {
                let _ = h.DragLeave();
            }
        }
        self.accept.set(false);
        self.set_hover(false);
        Ok(())
    }

    fn Drop(&self, pdataobj: Option<&IDataObject>, grfkeystate: MODIFIERKEYS_FLAGS, pt: &POINTL, pdweffect: *mut DROPEFFECT) -> Result<()> {
        let mut effect = self.effect(grfkeystate, allowed_effects(pdweffect));
        let mut paths = Vec::new();
        if effect != DROPEFFECT_NONE {
            if let Some(d) = pdataobj {
                match unsafe { dropped_paths(d) } {
                    Ok(p) => paths = p,
                    Err(e) => crate::pwarn!("drop: reading file list failed: {e}"),
                }
            }
        }
        if paths.is_empty() {
            effect = DROPEFFECT_NONE;
        }
        set_effect(pdweffect, effect);
        if let (Some(h), Some(d)) = (&self.helper, pdataobj) {
            unsafe {
                let _ = h.Drop(d, &point(pt), effect);
            }
        }
        self.accept.set(false);
        if paths.is_empty() {
            self.set_hover(false);
        } else {
            // FilesDropped also ends the hover state on the app side.
            self.hovering.set(false);
            self.sink.send(AppEvent::FilesDropped(paths));
        }
        Ok(())
    }
}

/// All file system paths in a CF_HDROP payload, in the order they were dragged.
unsafe fn dropped_paths(data: &IDataObject) -> Result<Vec<String>> {
    let mut medium = data.GetData(&hdrop_format())?;
    let mut out = Vec::new();
    if medium.tymed == TYMED_HGLOBAL.0 as u32 {
        let hdrop = HDROP(medium.u.hGlobal.0);
        let count = DragQueryFileW(hdrop, u32::MAX, None);
        for i in 0..count {
            let len = DragQueryFileW(hdrop, i, None);
            if len == 0 {
                continue;
            }
            let mut buf = vec![0u16; len as usize + 1];
            let n = DragQueryFileW(hdrop, i, Some(&mut buf)) as usize;
            if n > 0 {
                out.push(String::from_utf16_lossy(&buf[..n.min(len as usize)]));
            }
            if out.len() >= MAX_DROPPED {
                break;
            }
        }
    }
    ReleaseStgMedium(&mut medium);
    Ok(out)
}

/// Make the island a drop target for files.
pub fn register(hwnd: HWND, sink: EventSink) -> windows::core::Result<()> {
    let helper: Option<IDropTargetHelper> = match unsafe { CoCreateInstance(&CLSID_DragDropHelper, None, CLSCTX_INPROC_SERVER) } {
        Ok(h) => Some(h),
        Err(e) => {
            crate::pinfo!("drop target: no drag image helper ({e})");
            None
        }
    };
    let target: IDropTarget = DropTarget { hwnd, sink, helper, accept: Cell::new(false), hovering: Cell::new(false) }.into();
    // OLE keeps its own reference until RevokeDragDrop.
    unsafe { RegisterDragDrop(hwnd, &target)? };
    Ok(())
}

pub fn revoke(hwnd: HWND) {
    if let Err(e) = unsafe { RevokeDragDrop(hwnd) } {
        if e.code() != DRAGDROP_E_NOTREGISTERED {
            crate::pwarn!("RevokeDragDrop failed: {e}");
        }
    }
}
