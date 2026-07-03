use std::ffi::{CStr, CString};
use std::io;
use std::os::unix::io::{FromRawFd, OwnedFd, RawFd};

pub struct Pty {
    master: OwnedFd,
    child_pid: libc::pid_t,
}

#[repr(C)]
struct Winsize {
    ws_row: libc::c_ushort,
    ws_col: libc::c_ushort,
    ws_xpixel: libc::c_ushort,
    ws_ypixel: libc::c_ushort,
}

impl Pty {
    // forkpty deadlocks in multithreaded processes (dyld atfork lock vs Metal
    // worker threads), so the child must be created with posix_spawn instead.
    pub fn spawn(cols: u16, rows: u16, shell: Option<&str>) -> io::Result<Self> {
        let _ = crate::shell_integration::ensure_installed();
        let shell = shell
            .map(String::from)
            .unwrap_or_else(Self::detect_shell);

        let shell_cstr = CString::new(shell.as_str())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid shell path"))?;
        let shell_base = shell.rsplit('/').next().unwrap_or("sh");
        let login_arg = CString::new(format!("-{}", shell_base)).unwrap();

        let mut ws = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let mut master_fd: RawFd = -1;
        let mut slave_fd: RawFd = -1;

        unsafe {
            if libc::openpty(
                &mut master_fd,
                &mut slave_fd,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut ws as *mut Winsize as *mut libc::winsize,
            ) != 0
            {
                return Err(io::Error::last_os_error());
            }
        }

        let slave_path = unsafe {
            let p = libc::ptsname(master_fd);
            if p.is_null() {
                libc::close(master_fd);
                libc::close(slave_fd);
                return Err(io::Error::last_os_error());
            }
            CStr::from_ptr(p).to_owned()
        };

        let envs: Vec<CString> = std::env::vars()
            .filter(|(k, _)| k != "TERM" && k != "ZENITH_SHELL_INTEGRATION")
            .filter_map(|(k, v)| CString::new(format!("{}={}", k, v)).ok())
            .chain(std::iter::once(
                CString::new("TERM=xterm-256color").unwrap(),
            ))
            .chain(std::iter::once(
                CString::new("ZENITH_SHELL_INTEGRATION=1").unwrap(),
            ))
            .collect();
        let mut envp: Vec<*mut libc::c_char> =
            envs.iter().map(|e| e.as_ptr() as *mut libc::c_char).collect();
        envp.push(std::ptr::null_mut());

        let argv: [*mut libc::c_char; 2] =
            [login_arg.as_ptr() as *mut libc::c_char, std::ptr::null_mut()];

        const POSIX_SPAWN_SETSID: libc::c_int = 0x0400;
        const POSIX_SPAWN_CLOEXEC_DEFAULT: libc::c_int = 0x4000;

        let mut pid: libc::pid_t = 0;
        let ret = unsafe {
            let mut attr: libc::posix_spawnattr_t = std::mem::zeroed();
            libc::posix_spawnattr_init(&mut attr);
            libc::posix_spawnattr_setflags(
                &mut attr,
                (POSIX_SPAWN_SETSID | POSIX_SPAWN_CLOEXEC_DEFAULT) as libc::c_short,
            );

            let mut fa: libc::posix_spawn_file_actions_t = std::mem::zeroed();
            libc::posix_spawn_file_actions_init(&mut fa);
            // opened after setsid, so the slave becomes the controlling tty
            libc::posix_spawn_file_actions_addopen(&mut fa, 0, slave_path.as_ptr(), libc::O_RDWR, 0);
            libc::posix_spawn_file_actions_adddup2(&mut fa, 0, 1);
            libc::posix_spawn_file_actions_adddup2(&mut fa, 0, 2);

            let ret = libc::posix_spawn(
                &mut pid,
                shell_cstr.as_ptr(),
                &fa,
                &attr,
                argv.as_ptr(),
                envp.as_ptr(),
            );

            libc::posix_spawn_file_actions_destroy(&mut fa);
            libc::posix_spawnattr_destroy(&mut attr);
            libc::close(slave_fd);
            ret
        };

        if ret != 0 {
            unsafe { libc::close(master_fd) };
            return Err(io::Error::from_raw_os_error(ret));
        }

        unsafe {
            let flags = libc::fcntl(master_fd, libc::F_GETFL);
            libc::fcntl(master_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }

        Ok(Self {
            master: unsafe { OwnedFd::from_raw_fd(master_fd) },
            child_pid: pid,
        })
    }

    fn detect_shell() -> String {
        std::env::var("SHELL").unwrap_or_else(|_| {
            let pw = unsafe { libc::getpwuid(libc::getuid()) };
            if !pw.is_null() {
                let shell = unsafe { CStr::from_ptr((*pw).pw_shell) };
                shell.to_str().unwrap_or("/bin/zsh").to_string()
            } else {
                "/bin/zsh".to_string()
            }
        })
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        use std::os::unix::io::AsRawFd;
        let fd = self.master.as_raw_fd();
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        if n < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }

    pub fn write_all(&self, data: &[u8]) -> io::Result<()> {
        use std::os::unix::io::AsRawFd;
        let fd = self.master.as_raw_fd();
        let mut written = 0;
        while written < data.len() {
            let n = unsafe {
                libc::write(
                    fd,
                    data[written..].as_ptr() as *const libc::c_void,
                    data.len() - written,
                )
            };
            if n < 0 {
                return Err(io::Error::last_os_error());
            }
            written += n as usize;
        }
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> io::Result<()> {
        use std::os::unix::io::AsRawFd;
        let ws = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let ret = unsafe {
            libc::ioctl(
                self.master.as_raw_fd(),
                libc::TIOCSWINSZ,
                &ws as *const Winsize,
            )
        };
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub fn fd(&self) -> RawFd {
        use std::os::unix::io::AsRawFd;
        self.master.as_raw_fd()
    }

    pub fn child_pid(&self) -> libc::pid_t {
        self.child_pid
    }

    pub fn child_exited(&self) -> Option<i32> {
        let mut status: libc::c_int = 0;
        let ret = unsafe { libc::waitpid(self.child_pid, &mut status, libc::WNOHANG) };
        if ret > 0 {
            if libc::WIFEXITED(status) {
                Some(libc::WEXITSTATUS(status))
            } else {
                Some(-1)
            }
        } else {
            None
        }
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        unsafe {
            libc::kill(self.child_pid, libc::SIGHUP);
        }
    }
}
