use crate::state::ServerState;
use crate::utils;
use crate::utils::RESOURCE_CHUNK_SIZE;
use aura_rust::general::v1::general_service_server::GeneralService;
use aura_rust::general::v1::{
    ClearStateRequest, ClearStateResponse, GetConfigRequest, GetConfigResponse,
    GetEmailTokenRequest, GetEmailTokenResponse, GetServicesRequest, GetServicesResponse,
    GetTestUsersRequest, GetTestUsersResponse,
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
    async fn get_config(
        &self,
        _: Request<GetConfigRequest>,
    ) -> Result<Response<GetConfigResponse>, Status> {
        Ok(Response::new(GetConfigResponse {
            version: utils::VERSION.to_string(),
            resource_chunk_size: RESOURCE_CHUNK_SIZE as u32,
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

    async fn get_email_token(
        &self,
        _request: Request<GetEmailTokenRequest>,
    ) -> Result<Response<GetEmailTokenResponse>, Status> {
        #[cfg(feature = "testing")]
        {
            let email = _request.into_inner().email;
            let token = self
                .state
                .emails()
                .get_email_token(&email)
                .expect("Failed to get email token");

            Ok(Response::new(GetEmailTokenResponse { token }))
        }

        #[cfg(not(feature = "testing"))]
        Err(Status::failed_precondition("Server not in testing mode"))
    }

    async fn get_services(
        &self,
        _request: Request<GetServicesRequest>,
    ) -> Result<Response<GetServicesResponse>, Status> {
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

            Ok(Response::new(GetServicesResponse { services }))
        }

        #[cfg(not(feature = "testing"))]
        Err(Status::failed_precondition("Server not in testing mode"))
    }

    async fn get_test_users(
        &self,
        _: Request<GetTestUsersRequest>,
    ) -> Result<Response<GetTestUsersResponse>, Status> {
        #[cfg(feature = "testing")]
        {
            use crate::types::GrpcDomainType;

            Ok(Response::new(GetTestUsersResponse {
                user: Some(crate::testing::test_user(false).into_grpc().unwrap()),
                moderator: Some(crate::testing::moderator_user(false).into_grpc().unwrap()),
                admin: Some(crate::testing::admin_user(false).into_grpc().unwrap()),
            }))
        }

        #[cfg(not(feature = "testing"))]
        Err(Status::failed_precondition("Server not in testing mode"))
    }
}
