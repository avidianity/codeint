#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

mod socket_probe {
    wit_bindgen::generate!({
        inline: "
            package wasi:sockets@0.2.0;

            interface network {
                enum ip-address-family {
                    ipv4,
                    ipv6,
                }
                enum error-code {
                    unknown,
                    access-denied,
                    not-supported,
                    invalid-argument,
                    out-of-memory,
                    timeout,
                    concurrency-conflict,
                    not-in-progress,
                    would-block,
                    invalid-state,
                    new-socket-limit,
                    address-not-bindable,
                    address-in-use,
                    remote-unreachable,
                    connection-refused,
                    connection-reset,
                    connection-aborted,
                    datagram-too-large,
                    name-unresolvable,
                    temporary-resolver-failure,
                    permanent-resolver-failure,
                }
            }

            interface tcp {
                resource tcp-socket;
            }

            interface tcp-create-socket {
                use network.{error-code, ip-address-family};
                use tcp.{tcp-socket};
                create-tcp-socket: func(address-family: ip-address-family) -> result<tcp-socket, error-code>;
            }

            world socket-opener {
                import tcp-create-socket;
            }
        ",
    });
}

wit_bindgen::generate!({
    world: "language-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::language::{
    DeclaredDep, FileModel, FrameworkProfileResult, FrameworkProfileSelection, Guest,
    ImportRequest, LowerError, Manifest, ReferenceRange, Resolution, WorkspaceError,
};

struct SocketOpenerPlugin;

export!(SocketOpenerPlugin);

impl Guest for SocketOpenerPlugin {
    fn extensions() -> Vec<String> {
        let _socket = socket_probe::wasi::sockets::tcp_create_socket::create_tcp_socket(
            socket_probe::wasi::sockets::network::IpAddressFamily::Ipv4,
        );
        vec!["sock".to_string()]
    }

    probe_runtime::capability_probe_language_tail!();
}
