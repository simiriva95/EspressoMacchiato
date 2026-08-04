//! IOPMAssertion-based sleep inhibitor. Holds the assertion IDs in the
//! struct and releases them in `release()`/`Drop`. Deliberately not
//! `caffeinate`: no child process to orphan, and we want per-type control.

use core_foundation::base::TCFType;
use core_foundation::string::CFString;

use super::ffi;
use crate::platform::{InhibitOptions, PlatformError, SleepInhibitor};

#[derive(Default)]
pub struct MacInhibitor {
    system: Option<ffi::IOPMAssertionID>,
    display: Option<ffi::IOPMAssertionID>,
}

fn create_assertion(assertion_type: &str) -> Result<ffi::IOPMAssertionID, PlatformError> {
    let ty = CFString::new(assertion_type);
    let name = CFString::new("EspressoMacchiato: keep the machine awake and present");
    let mut id: ffi::IOPMAssertionID = 0;
    // Safety: both CFStrings outlive the call; IOKit copies them.
    let ret = unsafe {
        ffi::IOPMAssertionCreateWithName(
            ty.as_concrete_TypeRef(),
            ffi::kIOPMAssertionLevelOn,
            name.as_concrete_TypeRef(),
            &mut id,
        )
    };
    if ret == ffi::kIOReturnSuccess {
        Ok(id)
    } else {
        Err(PlatformError::Other(format!(
            "IOPMAssertionCreateWithName({assertion_type}) failed: IOReturn {ret:#x}"
        )))
    }
}

fn release_assertion(id: ffi::IOPMAssertionID) {
    // Safety: id came from IOPMAssertionCreateWithName and is released once.
    unsafe {
        ffi::IOPMAssertionRelease(id);
    }
}

impl SleepInhibitor for MacInhibitor {
    fn acquire(&mut self, opts: InhibitOptions) -> Result<(), PlatformError> {
        if self.system.is_none() {
            self.system = Some(create_assertion(ffi::ASSERTION_PREVENT_SYSTEM_SLEEP)?);
        }
        if opts.keep_display_on && self.display.is_none() {
            match create_assertion(ffi::ASSERTION_PREVENT_DISPLAY_SLEEP) {
                Ok(id) => self.display = Some(id),
                Err(e) => {
                    // Partial failure: keep the system assertion, report.
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    fn release(&mut self) -> Result<(), PlatformError> {
        if let Some(id) = self.system.take() {
            release_assertion(id);
        }
        if let Some(id) = self.display.take() {
            release_assertion(id);
        }
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.system.is_some() || self.display.is_some()
    }
}

impl Drop for MacInhibitor {
    fn drop(&mut self) {
        let _ = self.release();
    }
}
