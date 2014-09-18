use std::kinds::marker;
use std::mem;
use std::raw as stdraw;
use std::str;
use libc::{c_uint, c_int, c_void, c_long};

use {raw, Error, DisconnectCode, ByApplication, SessionFlag, HostKeyType};
use {MethodType, Agent, Channel};

pub struct Session {
    raw: *mut raw::LIBSSH2_SESSION,
    marker: marker::NoSync,
}

impl Session {
    /// Initializes an SSH session object.
    pub fn new() -> Option<Session> {
        ::init();
        unsafe {
            let ret = raw::libssh2_session_init_ex(None, None, None);
            if ret.is_null() { return None  }
            Some(Session::from_raw(ret))
        }
    }

    /// Takes ownership of the given raw pointer and wraps it in a session.
    ///
    /// This is unsafe as there is no guarantee about the validity of `raw`.
    pub unsafe fn from_raw(raw: *mut raw::LIBSSH2_SESSION) -> Session {
        Session {
            raw: raw,
            marker: marker::NoSync,
        }
    }

    /// Get the remote banner
    ///
    /// Once the session has been setup and handshake() has completed
    /// successfully, this function can be used to get the server id from the
    /// banner each server presents.
    ///
    /// May return `None` on invalid utf-8 or if an error has ocurred.
    pub fn banner(&self) -> Option<&str> {
        self.banner_bytes().and_then(str::from_utf8)
    }

    /// See `banner`.
    ///
    /// Will only return `None` if an error has ocurred.
    pub fn banner_bytes(&self) -> Option<&[u8]> {
        unsafe { ::opt_bytes(self, raw::libssh2_session_banner_get(self.raw)) }
    }

    /// Set the SSH protocol banner for the local client
    ///
    /// Set the banner that will be sent to the remote host when the SSH session
    /// is started with handshake(). This is optional; a banner
    /// corresponding to the protocol and libssh2 version will be sent by
    /// default.
    pub fn set_banner(&self, banner: &str) -> Result<(), Error> {
        let banner = banner.to_c_str();
        unsafe {
            self.rc(raw::libssh2_session_banner_set(self.raw, banner.as_ptr()))
        }
    }

    /// Terminate the transport layer.
    ///
    /// Send a disconnect message to the remote host associated with session,
    /// along with a reason symbol and a verbose description.
    pub fn disconnect(&self,
                      reason: Option<DisconnectCode>,
                      description: &str,
                      lang: Option<&str>) -> Result<(), Error> {
        let reason = reason.unwrap_or(ByApplication) as c_int;
        let description = description.to_c_str();
        let lang = lang.unwrap_or("").to_c_str();
        unsafe {
            self.rc(raw::libssh2_session_disconnect_ex(self.raw,
                                                       reason,
                                                       description.as_ptr(),
                                                       lang.as_ptr()))
        }
    }

    /// Enable or disable a flag for this session.
    pub fn flag(&self, flag: SessionFlag, enable: bool) -> Result<(), Error> {
        unsafe {
            self.rc(raw::libssh2_session_flag(self.raw, flag as c_int,
                                              enable as c_int))
        }
    }

    /// Returns whether the session was previously set to nonblocking.
    pub fn is_blocking(&self) -> bool {
        unsafe { raw::libssh2_session_get_blocking(self.raw) != 0 }
    }

    /// Set or clear blocking mode on session
    ///
    /// Set or clear blocking mode on the selected on the session. This will
    /// instantly affect any channels associated with this session. If a read
    /// is performed on a session with no data currently available, a blocking
    /// session will wait for data to arrive and return what it receives. A
    /// non-blocking session will return immediately with an empty buffer. If a
    /// write is performed on a session with no room for more data, a blocking
    /// session will wait for room. A non-blocking session will return
    /// immediately without writing anything.
    pub fn set_blocking(&self, blocking: bool) {
        unsafe {
            raw::libssh2_session_set_blocking(self.raw, blocking as c_int)
        }
    }

    /// Returns the timeout, in milliseconds, for how long blocking calls may
    /// wait until they time out.
    ///
    /// A timeout of 0 signifies no timeout.
    pub fn timeout(&self) -> uint {
        unsafe { raw::libssh2_session_get_timeout(self.raw) as uint }
    }

    /// Set timeout for blocking functions.
    ///
    /// Set the timeout in milliseconds for how long a blocking the libssh2
    /// function calls may wait until they consider the situation an error and
    /// return an error.
    ///
    /// By default or if you set the timeout to zero, libssh2 has no timeout
    /// for blocking functions.
    pub fn set_timeout(&self, timeout_ms: uint) {
        let timeout_ms = timeout_ms as c_long;
        unsafe { raw::libssh2_session_set_timeout(self.raw, timeout_ms) }
    }

    /// Get the remote key.
    ///
    /// Returns `None` if something went wrong.
    pub fn host_key(&self) -> Option<(&[u8], HostKeyType)> {
        let mut len = 0;
        let mut kind = 0;
        unsafe {
            let ret = raw::libssh2_session_hostkey(self.raw, &mut len, &mut kind);
            if ret.is_null() { return None }
            let data: &[u8] = mem::transmute(stdraw::Slice {
                data: ret as *const u8,
                len: len as uint,
            });
            let kind = match kind {
                raw::LIBSSH2_HOSTKEY_TYPE_RSA => ::TypeRsa,
                raw::LIBSSH2_HOSTKEY_TYPE_DSS => ::TypeDss,
                _ => ::TypeUnknown,
            };
            Some((data, kind))
        }
    }

    /// Set preferred key exchange method
    ///
    /// The preferences provided are a comma delimited list of preferred methods
    /// to use with the most preferred listed first and the least preferred
    /// listed last. If a method is listed which is not supported by libssh2 it
    /// will be ignored and not sent to the remote host during protocol
    /// negotiation.
    pub fn method_pref(&self,
                       method_type: MethodType,
                       prefs: &str) -> Result<(), Error> {
        let prefs = prefs.to_c_str();
        unsafe {
            self.rc(raw::libssh2_session_method_pref(self.raw,
                                                     method_type as c_int,
                                                     prefs.as_ptr()))
        }
    }

    /// Return the currently active algorithms.
    ///
    /// Returns the actual method negotiated for a particular transport
    /// parameter. May return `None` if the session has not yet been started.
    pub fn methods(&self, method_type: MethodType) -> Option<&str> {
        unsafe {
            let ptr = raw::libssh2_session_methods(self.raw,
                                                   method_type as c_int);
            ::opt_bytes(self, ptr).and_then(str::from_utf8)
        }
    }

    /// Get list of supported algorithms.
    pub fn supported_algs(&self, method_type: MethodType)
                          -> Result<Vec<&'static str>, Error> {
        let method_type = method_type as c_int;
        let mut ret = Vec::new();
        unsafe {
            let mut ptr = 0 as *mut _;
            let rc = raw::libssh2_session_supported_algs(self.raw, method_type,
                                                         &mut ptr);
            if rc <= 0 { try!(self.rc(rc)) }
            for i in range(0, rc as int) {
                ret.push(str::raw::c_str_to_static_slice(*ptr.offset(i)));
            }
            raw::libssh2_free(self.raw, ptr as *mut c_void);
        }
        Ok(ret)
    }

    /// Init an ssh-agent handle.
    ///
    /// The returned agent will still need to be connected manually before use.
    pub fn agent(&self) -> Result<Agent, Error> {
        unsafe {
            let ptr = raw::libssh2_agent_init(self.raw);
            if ptr.is_null() {
                Err(Error::last_error(self).unwrap())
            } else {
                Ok(Agent::from_raw(self, ptr))
            }
        }
    }

    /// Begin transport layer protocol negotiation with the connected host.
    ///
    /// The socket provided is a connected socket descriptor. Typically a TCP
    /// connection though the protocol allows for any reliable transport and
    /// the library will attempt to use any berkeley socket.
    pub fn handshake(&self, socket: raw::libssh2_socket_t) -> Result<(), Error> {
        unsafe {
            self.rc(raw::libssh2_session_handshake(self.raw, socket))
        }
    }

    /// Allocate a new channel for exchanging data with the server.
    ///
    /// This is typically not called directly but rather through
    /// `channel_open_session`, `channel_direct_tcpip`, or
    /// `channel_forward_listen`.
    pub fn channel_open(&self, channel_type: &str,
                        window_size: uint, packet_size: uint,
                        message: Option<&str>) -> Result<Channel, Error> {
        let ret = unsafe {
            let channel_type_len = channel_type.len();
            let channel_type = channel_type.to_c_str();
            let message_len = message.map(|s| s.len()).unwrap_or(0);
            let message = message.map(|s| s.to_c_str());
            raw::libssh2_channel_open_ex(self.raw,
                                         channel_type.as_ptr(),
                                         channel_type_len as c_uint,
                                         window_size as c_uint,
                                         packet_size as c_uint,
                                         message.as_ref().map(|s| s.as_ptr())
                                                .unwrap_or(0 as *const _),
                                         message_len as c_uint)
        };
        if ret.is_null() {
            Err(Error::last_error(self).unwrap())
        } else {
            Ok(unsafe { Channel::from_raw(self, ret) })
        }
    }

    /// Establish a new session-based channel.
    pub fn channel_session(&self) -> Result<Channel, Error> {
        self.channel_open("session",
                          raw::LIBSSH2_CHANNEL_WINDOW_DEFAULT as uint,
                          raw::LIBSSH2_CHANNEL_PACKET_DEFAULT as uint, None)
    }

    /// Indicates whether or not the named session has been successfully
    /// authenticated.
    pub fn authenticated(&self) -> bool {
        unsafe { raw::libssh2_userauth_authenticated(self.raw) != 0 }
    }

    /// Send a SSH_USERAUTH_NONE request to the remote host.
    ///
    /// Unless the remote host is configured to accept none as a viable
    /// authentication scheme (unlikely), it will return SSH_USERAUTH_FAILURE
    /// along with a listing of what authentication schemes it does support. In
    /// the unlikely event that none authentication succeeds, this method with
    /// return NULL. This case may be distinguished from a failing case by
    /// examining libssh2_userauth_authenticated.
    ///
    /// The return value is a comma-separated string of supported auth schemes.
    pub fn auth_methods(&self, username: &str) -> Result<&str, Error> {
        let len = username.len();
        let username = username.to_c_str();
        unsafe {
            let ret = raw::libssh2_userauth_list(self.raw, username.as_ptr(),
                                                 len as c_uint);
            if ret.is_null() {
                Err(Error::last_error(self).unwrap())
            } else {
                Ok(str::raw::c_str_to_static_slice(ret))
            }
        }
    }

    /// Gain access to the underlying raw libssh2 session pointer.
    pub fn raw(&self) -> *mut raw::LIBSSH2_SESSION { self.raw }

    /// Translate a return code into a Rust-`Result`.
    pub fn rc(&self, rc: c_int) -> Result<(), Error> {
        if rc == 0 {
            Ok(())
        } else {
            match Error::last_error(self) {
                Some(e) => Err(e),
                None => Ok(()),
            }
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            assert_eq!(raw::libssh2_session_free(self.raw), 0);
        }
    }
}
