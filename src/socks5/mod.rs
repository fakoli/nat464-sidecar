mod handshake;
mod relay;

pub use relay::handle_client;

/// SOCKS5 protocol constants (RFC 1928).
pub const SOCKS_VERSION: u8 = 0x05;
pub const AUTH_NO_AUTH: u8 = 0x00;
pub const CMD_CONNECT: u8 = 0x01;
pub const ATYP_IPV4: u8 = 0x01;
pub const ATYP_DOMAIN: u8 = 0x03;
pub const ATYP_IPV6: u8 = 0x04;
pub const REPLY_SUCCEEDED: u8 = 0x00;
pub const REPLY_GENERAL_FAILURE: u8 = 0x01;
pub const REPLY_COMMAND_NOT_SUPPORTED: u8 = 0x07;
pub const REPLY_ADDRESS_TYPE_NOT_SUPPORTED: u8 = 0x08;
