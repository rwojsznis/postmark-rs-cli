//! One module per resource; one `Leaf` per endpoint (decision 11).

pub(crate) mod server;

use crate::error::Error;
use crate::request::GetRequest;
use crate::spec::EndpointSpec;

/// A command that calls one endpoint. The dispatcher in `cli` does the rest:
/// execution, pagination, output and errors are written once.
pub(crate) trait Leaf {
    /// The endpoint this command calls.
    fn spec(&self) -> &'static EndpointSpec;

    /// The request for the parsed args. Pure: no network, so a unit test
    /// covers every flag.
    fn request(&self) -> Result<GetRequest, Error>;
}
