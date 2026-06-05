//! Modbus TCP server exposing the simulated RTAC.
//!
//! The server answers holding-register reads from the simulated state and
//! records command-register writes back into it. All connections share a single
//! [`SimState`] behind a mutex, which the physics tick loop also mutates.

use std::{
    future,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use tokio::net::TcpListener;
use tokio_modbus::{
    Exception, Request, Response,
    server::tcp::{Server, accept_tcp_connection},
};

use crate::state::SimState;

/// Shared handle to the simulated state.
pub type SharedState = Arc<Mutex<SimState>>;

/// A Modbus service backed by the shared simulated state.
struct SimService {
    state: SharedState,
}

impl tokio_modbus::server::Service for SimService {
    type Request = Request<'static>;
    type Response = Response;
    type Exception = Exception;
    type Future = future::Ready<Result<Self::Response, Self::Exception>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        match req {
            Request::ReadHoldingRegisters(addr, cnt) => {
                let regs = self.state.lock().unwrap().read_registers(addr, cnt);
                future::ready(Ok(Response::ReadHoldingRegisters(regs)))
            }
            // Some clients read status via input registers; serve the same map.
            Request::ReadInputRegisters(addr, cnt) => {
                let regs = self.state.lock().unwrap().read_registers(addr, cnt);
                future::ready(Ok(Response::ReadInputRegisters(regs)))
            }
            Request::WriteMultipleRegisters(addr, values) => {
                self.state.lock().unwrap().write_registers(addr, &values);
                future::ready(Ok(Response::WriteMultipleRegisters(addr, values.len() as u16)))
            }
            Request::WriteSingleRegister(addr, value) => {
                self.state.lock().unwrap().write_registers(addr, &[value]);
                future::ready(Ok(Response::WriteSingleRegister(addr, value)))
            }
            _ => future::ready(Err(Exception::IllegalFunction)),
        }
    }
}

/// Bind a TCP listener for the simulated RTAC and return it along with the
/// address actually bound (useful when binding to port 0 in tests).
pub async fn bind(addr: SocketAddr) -> std::io::Result<(TcpListener, SocketAddr)> {
    let listener = TcpListener::bind(addr).await?;
    let local = listener.local_addr()?;
    Ok((listener, local))
}

/// Serve the Modbus protocol on an already-bound listener until it errors.
///
/// All connections share `state`.
pub async fn serve(
    listener: TcpListener,
    state: SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server = Server::new(listener);

    let on_connected = move |stream, socket_addr| {
        let state = state.clone();
        async move {
            let new_service = move |_socket_addr| Ok(Some(SimService { state: state.clone() }));
            accept_tcp_connection(stream, socket_addr, new_service)
        }
    };
    let on_process_error = |err| {
        tracing::error!(error = %err, "modbus connection error");
    };

    server.serve(&on_connected, on_process_error).await?;
    Ok(())
}
