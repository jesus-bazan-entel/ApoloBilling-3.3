// src/services/mod.rs
pub mod authorization;
pub mod reservation_manager;
pub mod realtime_biller;
pub mod cdr_generator;
pub mod call_simulator;

pub use authorization::AuthorizationService;
pub use reservation_manager::{ReservationManager, ExtensionResult};
pub use realtime_biller::RealtimeBiller;
pub use cdr_generator::{CdrGenerator, HangupEvent};
pub use call_simulator::{CallSimulator, SimulateCallRequest, SimulateCallResponse, SimulationScenario};
