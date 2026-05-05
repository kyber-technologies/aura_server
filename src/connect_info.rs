use std::str::FromStr;
use tonic::Status;
use tonic::metadata::AsciiMetadataValue;
use tonic::service::Interceptor;

#[derive(Debug, Clone)]
pub struct ConnectInfoInterceptor;

impl Interceptor for ConnectInfoInterceptor {
    fn call(&mut self, mut request: tonic::Request<()>) -> Result<tonic::Request<()>, Status> {
        let addr = request.remote_addr().expect("Failed to get remote address");

        request.metadata_mut().insert(
            "forwarded",
            AsciiMetadataValue::from_str(&format!("for={addr}")).unwrap(),
        );

        Ok(request)
    }
}
