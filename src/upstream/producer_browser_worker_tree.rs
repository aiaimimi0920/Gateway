use tokio::process::{Child, Command};

#[cfg(unix)]
use std::os::unix::process::CommandExt as _;
#[cfg(windows)]
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

pub(super) struct ProcessTreeGuard {
    #[cfg(unix)]
    process_group_id: Option<i32>,
    #[cfg(windows)]
    job_handle: usize,
}

impl ProcessTreeGuard {
    pub(super) fn prepare(command: &mut Command) -> std::io::Result<Self> {
        #[cfg(unix)]
        {
            command.as_std_mut().process_group(0);
            Ok(Self {
                process_group_id: None,
            })
        }
        #[cfg(windows)]
        {
            let _ = command;
            let job_handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if job_handle.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let guard = Self {
                job_handle: job_handle as usize,
            };
            let mut information = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let configured = unsafe {
                SetInformationJobObject(
                    job_handle,
                    JobObjectExtendedLimitInformation,
                    std::ptr::addr_of!(information).cast(),
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            if configured == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(guard)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = command;
            Ok(Self {})
        }
    }

    pub(super) fn attach(&mut self, child: &Child) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            let process_id = child.id().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "worker process ID unavailable",
                )
            })?;
            self.process_group_id = Some(i32::try_from(process_id).map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "worker process ID exceeded the platform range",
                )
            })?);
        }
        #[cfg(windows)]
        {
            let process_handle = child.raw_handle().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "worker process handle unavailable",
                )
            })?;
            let assigned = unsafe {
                AssignProcessToJobObject(self.job_handle as HANDLE, process_handle as HANDLE)
            };
            if assigned == 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = child;
        }
        Ok(())
    }

    pub(super) fn terminate(&self) {
        #[cfg(unix)]
        if let Some(process_group_id) = self.process_group_id {
            let _ = unsafe { libc::kill(-process_group_id, libc::SIGKILL) };
        }
        #[cfg(windows)]
        {
            let _ = unsafe { TerminateJobObject(self.job_handle as HANDLE, 1) };
        }
    }
}

impl Drop for ProcessTreeGuard {
    fn drop(&mut self) {
        self.terminate();
        #[cfg(windows)]
        {
            let _ = unsafe { CloseHandle(self.job_handle as HANDLE) };
        }
    }
}
