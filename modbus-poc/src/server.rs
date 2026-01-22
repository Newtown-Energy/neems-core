use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio_modbus::prelude::*;
use tokio_modbus::server::tcp::Server;

const NUM_REGISTERS: usize = 100;

/// Service implementation that handles Modbus requests
struct MockModbusService {
    holding_registers: Arc<Mutex<HashMap<u16, u16>>>,
}

impl tokio_modbus::server::Service for MockModbusService {
    type Request = Request<'static>;
    type Response = Response;
    type Exception = ExceptionCode;
    type Future = std::future::Ready<Result<Self::Response, Self::Exception>>;

    fn call(&self, request: Self::Request) -> Self::Future {
        let result = match request {
            Request::ReadHoldingRegisters(address, count) => {
                self.read_holding_registers(address, count)
            }
            Request::WriteSingleRegister(address, value) => {
                self.write_single_register(address, value)
            }
            Request::WriteMultipleRegisters(address, values) => {
                self.write_multiple_registers(address, &values)
            }
            _ => {
                println!("  Unsupported function code");
                Err(ExceptionCode::IllegalFunction)
            }
        };
        std::future::ready(result)
    }
}

impl MockModbusService {
    fn new(holding_registers: Arc<Mutex<HashMap<u16, u16>>>) -> Self {
        Self { holding_registers }
    }

    fn read_holding_registers(&self, address: u16, count: u16) -> Result<Response, ExceptionCode> {
        println!(
            "  READ holding registers: address={}, count={}",
            address, count
        );

        let registers = self.holding_registers.lock().unwrap();
        let mut values = Vec::with_capacity(count as usize);

        for i in 0..count {
            let addr = address + i;
            if addr as usize >= NUM_REGISTERS {
                println!("    ERROR: Address {} out of range", addr);
                return Err(ExceptionCode::IllegalDataAddress);
            }
            let value = registers.get(&addr).copied().unwrap_or(0);
            values.push(value);
        }

        println!("    Returning: {:?}", values);
        Ok(Response::ReadHoldingRegisters(values))
    }

    fn write_single_register(&self, address: u16, value: u16) -> Result<Response, ExceptionCode> {
        println!(
            "  WRITE single register: address={}, value={}",
            address, value
        );

        if address as usize >= NUM_REGISTERS {
            println!("    ERROR: Address out of range");
            return Err(ExceptionCode::IllegalDataAddress);
        }

        let mut registers = self.holding_registers.lock().unwrap();
        registers.insert(address, value);
        println!("    Successfully wrote value");
        Ok(Response::WriteSingleRegister(address, value))
    }

    fn write_multiple_registers(
        &self,
        address: u16,
        values: &[u16],
    ) -> Result<Response, ExceptionCode> {
        println!(
            "  WRITE multiple registers: address={}, values={:?}",
            address, values
        );

        let end = address as usize + values.len();
        if end > NUM_REGISTERS {
            println!("    ERROR: Address out of range");
            return Err(ExceptionCode::IllegalDataAddress);
        }

        let mut registers = self.holding_registers.lock().unwrap();
        for (i, value) in values.iter().enumerate() {
            registers.insert(address + i as u16, *value);
        }
        println!("    Successfully wrote {} values", values.len());
        Ok(Response::WriteMultipleRegisters(address, values.len() as u16))
    }
}

/// Run the mock Modbus TCP server
pub async fn run_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let socket_addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    println!("Starting mock Modbus TCP server on {}...", socket_addr);
    println!("Pre-populated registers:");
    println!("  Register 0: 1000 (voltage)");
    println!("  Register 1: 500 (current)");
    println!("  Register 2: 5000 (power)");
    println!("  Register 3: 100 (SOC)");
    println!("  Register 4: 25 (temperature)");
    println!();
    println!("Press Ctrl+C to stop the server.");
    println!();

    // Initialize holding registers with demo values
    let holding_registers = Arc::new(Mutex::new(HashMap::new()));
    {
        let mut regs = holding_registers.lock().unwrap();
        regs.insert(0, 1000); // Simulated voltage
        regs.insert(1, 500); // Simulated current
        regs.insert(2, 5000); // Simulated power
        regs.insert(3, 100); // Simulated SOC (state of charge)
        regs.insert(4, 25); // Simulated temperature
    }

    let listener = TcpListener::bind(socket_addr).await?;
    let server = Server::new(listener);

    let on_connected = {
        let registers = holding_registers.clone();
        move |stream, socket_addr| {
            let registers = registers.clone();
            async move {
                let service = MockModbusService::new(registers);
                println!("Client connected from {}", socket_addr);
                Ok(Some((service, stream)))
            }
        }
    };

    let on_process_error = |err| {
        eprintln!("Process error: {}", err);
    };

    server.serve(&on_connected, on_process_error).await?;

    Ok(())
}
