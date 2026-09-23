use crate::state::ServerState;
use crate::{config, utils};
use aura_rust::general::v1::general_service_server::GeneralService;
use aura_rust::general::v1::{
    ClearStateRequest, ClearStateResponse, ConfigRequest, ConfigResponse, EmailTokenRequest,
    EmailTokenResponse, ServicesRequest, ServicesResponse, TestUsersRequest, TestUsersResponse,
};
use tonic::{Request, Response, Status};

pub struct Service {
    #[allow(unused)]
    state: ServerState,
}

impl Service {
    pub fn new(state: ServerState) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl GeneralService for Service {
    async fn config(&self, _: Request<ConfigRequest>) -> Result<Response<ConfigResponse>, Status> {
        let config = config::get();

        Ok(Response::new(ConfigResponse {
            version: utils::VERSION.to_string(),
            resource_chunk_size: config.service.resource_chunk_size as u32,
            item_request_limit: config.service.item_request_limit,
        }))
    }

    async fn clear_state(
        &self,
        _: Request<ClearStateRequest>,
    ) -> Result<Response<ClearStateResponse>, Status> {
        #[cfg(feature = "testing")]
        {
            self.state
                .clear_state()
                .await
                .expect("Failed to clear state");

            Ok(Response::new(ClearStateResponse {}))
        }

        #[cfg(not(feature = "testing"))]
        Err(Status::failed_precondition("Server not in testing mode"))
    }

    async fn email_token(
        &self,
        _request: Request<EmailTokenRequest>,
    ) -> Result<Response<EmailTokenResponse>, Status> {
        #[cfg(feature = "testing")]
        {
            let email = _request.into_inner().email;
            let token = self
                .state
                .emails()
                .get_email_token(&email)
                .expect("Failed to get email token");

            Ok(Response::new(EmailTokenResponse { token }))
        }

        #[cfg(not(feature = "testing"))]
        Err(Status::failed_precondition("Server not in testing mode"))
    }

    async fn services(
        &self,
        _request: Request<ServicesRequest>,
    ) -> Result<Response<ServicesResponse>, Status> {
        #[cfg(feature = "testing")]
        {
            use aura_rust::types::FileDescriptorSet;
            use aura_rust::{FILE_DESCRIPTOR_SET, Message};

            let services = FileDescriptorSet::decode(FILE_DESCRIPTOR_SET)
                .map_err(|e| Status::internal(e.to_string()))?
                .file
                .into_iter()
                .flat_map(|desc| desc.service)
                .map(|serv| aura_rust::general::v1::ServiceDescriptor {
                    name: serv.name.unwrap_or_else(|| "<unknown>".to_string()),
                    methods: serv
                        .method
                        .into_iter()
                        .map(|meth| meth.name.unwrap_or_else(|| "<unknown>".to_string()))
                        .collect(),
                })
                .collect::<Vec<_>>();

            Ok(Response::new(ServicesResponse { services }))
        }

        #[cfg(not(feature = "testing"))]
        Err(Status::failed_precondition("Server not in testing mode"))
    }

    async fn test_users(
        &self,
        _: Request<TestUsersRequest>,
    ) -> Result<Response<TestUsersResponse>, Status> {
        #[cfg(feature = "testing")]
        {
            use crate::types::GrpcDomainType;

            Ok(Response::new(TestUsersResponse {
                user: Some(crate::testing::test_user(false).into_grpc().unwrap()),
                moderator: Some(crate::testing::moderator_user(false).into_grpc().unwrap()),
                admin: Some(crate::testing::admin_user(false).into_grpc().unwrap()),
            }))
        }

        #[cfg(not(feature = "testing"))]
        Err(Status::failed_precondition("Server not in testing mode"))
    }
}
