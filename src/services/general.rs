use crate::state::ServerState;
use crate::utils;
use crate::utils::RESOURCE_CHUNK_SIZE;
use aura_rust::general::v1::general_service_server::GeneralService;
use aura_rust::general::v1::{
    ClearStateRequest, ClearStateResponse, GetConfigRequest, GetConfigResponse,
    GetEmailTokenRequest, GetEmailTokenResponse,
};
use tonic::{Request, Response, Status};

#[cfg(feature = "testing")]
const TEST_NEW_USER_NAME: &str = "user";
#[cfg(feature = "testing")]
const TEST_NEW_USER_PASS: &str = "user";

#[cfg(feature = "testing")]
const TEST_SUPERVISOR_NAME: &str = "supervisor";
#[cfg(feature = "testing")]
const TEST_SUPERVISOR_PASS: &str = "supervisor";

#[cfg(feature = "testing")]
const TEST_ADMIN_NAME: &str = "admin";
#[cfg(feature = "testing")]
const TEST_ADMIN_PASS: &str = "admin";

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
            clear_state(&self.state).await;

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
}

#[cfg(feature = "testing")]
async fn clear_state(state: &ServerState) {
    let database = state.database();

    tracing::info!("Detected test environment. Clearing database...");

    database
        .query(
            r#"
REMOVE TABLE user;
REMOVE TABLE channel;
REMOVE TABLE message;
REMOVE TABLE resource;
REMOVE TABLE blocked;
"#,
        )
        .await
        .expect("Failed to drop user table");

    // Setup database again, since we just cleared all tables
    database.setup().await;

    tracing::info!("Creating test user with role user...");
    crate::user::create(
        database,
        aura_rust::User {
            user_id: TEST_NEW_USER_NAME.to_string(),
            username: TEST_NEW_USER_NAME.to_string(),
            email: "foo@bar.baz".to_string(),
            password: crate::auth::hash(TEST_NEW_USER_PASS.to_string())
                .expect("Failed to hash new user password"),
            role: aura_rust::user::v1::UserRole::UserUnspecified as i32,
            icon: crate::resource::build_user_avatar_id(TEST_NEW_USER_NAME),
        },
    )
    .await
    .expect("Failed to create new test user");

    tracing::info!("Creating test user with role supervisor...");
    crate::user::create(
        database,
        aura_rust::User {
            user_id: TEST_SUPERVISOR_NAME.to_string(),
            username: TEST_SUPERVISOR_NAME.to_string(),
            email: "foo@bar.baz".to_string(),
            password: crate::auth::hash(TEST_SUPERVISOR_PASS.to_string())
                .expect("Failed to hash supervisor password"),
            role: aura_rust::user::v1::UserRole::Moderator as i32,
            icon: crate::resource::build_user_avatar_id(TEST_SUPERVISOR_NAME),
        },
    )
    .await
    .expect("Failed to create new admin test user");

    tracing::info!("Creating test user with role admin...");
    crate::user::create(
        database,
        aura_rust::User {
            user_id: TEST_ADMIN_NAME.to_string(),
            username: TEST_ADMIN_NAME.to_string(),
            email: "foo@bar.baz".to_string(),
            password: crate::auth::hash(TEST_ADMIN_PASS.to_string())
                .expect("Failed to hash admin password"),
            role: aura_rust::user::v1::UserRole::Admin as i32,
            icon: crate::resource::build_user_avatar_id(TEST_ADMIN_NAME),
        },
    )
    .await
    .expect("Failed to create new supervisor test user");
}
