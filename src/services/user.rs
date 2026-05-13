use crate::error::Error;
use crate::state::ServerState;
use crate::{auth, user};
use aura_rust::common::v1::ErrorCode;
use aura_rust::user::v1::user_service_server::UserService;
use aura_rust::user::v1::{
    AuthUserRequest, AuthUserResponse, BlockUserRequest, BlockUserResponse, CreateUserRequest,
    CreateUserResponse, DeleteUserRequest, DeleteUserResponse, GetUserRequest, GetUserResponse,
    IsBlockedRequest, IsBlockedResponse, SearchUsersRequest, SearchUsersResponse,
    UpdateUserRequest, UpdateUserResponse, UserExistsRequest, UserExistsResponse, UserRole,
    VerifyEmailRequest, VerifyEmailResponse, auth_user_response, get_user_response,
    is_blocked_response,
};
use aura_rust::{DEFAULT_USER_ICON, User};
use tonic::{Request, Response, Status};

pub struct Service {
    state: ServerState,
}

impl Service {
    pub fn new(state: ServerState) -> Self {
        Self { state }
    }

    async fn _user_exists(
        &self,
        request: Request<UserExistsRequest>,
    ) -> Result<UserExistsResponse, Error> {
        let user_id = request.into_inner().user_id;
        let exists = user::exists(self.state.database(), &user_id).await?;

        Ok(UserExistsResponse {
            error: if exists {
                Some(Error::new(ErrorCode::AlreadyExists, "User already exists").into())
            } else {
                None
            },
        })
    }

    async fn _auth_user(
        &self,
        request: Request<AuthUserRequest>,
    ) -> Result<AuthUserResponse, Error> {
        let AuthUserRequest { user_id, password } = request.into_inner();

        let token = auth::auth(self.state.database(), user_id, password).await?;

        Ok(AuthUserResponse {
            result: Some(auth_user_response::Result::Token(token)),
        })
    }

    async fn _verify_email(
        &self,
        request: Request<VerifyEmailRequest>,
    ) -> Result<VerifyEmailResponse, Error> {
        let VerifyEmailRequest { email } = request.into_inner();

        self.state.emails().register_email(email)?;

        Ok(VerifyEmailResponse { error: None })
    }

    async fn _create_user(
        &self,
        request: Request<CreateUserRequest>,
    ) -> Result<CreateUserResponse, Error> {
        let database = self.state.database();

        let request = request.into_inner();

        self.state
            .emails()
            .verify_email(&request.email, request.verification_token)?;

        let mut user = User {
            user_id: request.user_id,
            username: request.username,
            email: request.email.clone(),
            password: request.password,
            role: UserRole::UserUnspecified as i32,
            icon: DEFAULT_USER_ICON.clone(),
        };

        user.password = auth::hash(user.password).map_err(|err| {
            Error::new(
                ErrorCode::Internal,
                format!("Hashing password failed: {err}"),
            )
        })?;

        user::create(database, user).await?;

        Ok(CreateUserResponse { error: None })
    }

    async fn _delete_user(
        &self,
        request: Request<DeleteUserRequest>,
    ) -> Result<DeleteUserResponse, Error> {
        let database = self.state.database();

        let user = auth::verify(database, &request).await?;
        let password = request.into_inner().password;

        auth::auth(database, user.user_id.clone(), password).await?;

        user::delete(database, &user.user_id).await?;

        Ok(DeleteUserResponse { error: None })
    }

    async fn _update_user(
        &self,
        request: Request<UpdateUserRequest>,
    ) -> Result<UpdateUserResponse, Error> {
        let database = self.state.database();

        let mut user = auth::verify(database, &request).await?;
        let request = request.into_inner();

        user.username = request.username.unwrap_or(user.username);
        user.email = request.email.unwrap_or(user.email);
        user.password = request
            .password
            .map(auth::hash)
            .unwrap_or(Ok(user.password))
            .map_err(|err| {
                Error::new(
                    ErrorCode::Internal,
                    format!("Hashing password failed: {err}"),
                )
            })?;

        user::update(database, user).await?;

        Ok(UpdateUserResponse { error: None })
    }

    async fn _get_user(&self, request: Request<GetUserRequest>) -> Result<GetUserResponse, Error> {
        let database = self.state.database();

        auth::verify(database, &request).await?;
        let user = request.into_inner().user_id;

        let user = user::get(database, &user)
            .await?
            .ok_or(Error::new(ErrorCode::NotFound, "User not found"))?;

        Ok(GetUserResponse {
            result: Some(get_user_response::Result::User(user::to_profile(user))),
        })
    }

    async fn _search_user(
        &self,
        request: Request<SearchUsersRequest>,
    ) -> Result<SearchUsersResponse, Error> {
        let database = self.state.database();

        auth::verify(database, &request).await?;
        let query = request.into_inner().query;

        let users = user::search(database, query).await?;

        Ok(SearchUsersResponse { users, error: None })
    }

    async fn _block_user(
        &self,
        request: Request<BlockUserRequest>,
    ) -> Result<BlockUserResponse, Error> {
        let database = self.state.database();
        let user = auth::verify(database, &request).await?;
        let BlockUserRequest { user_id, block } = request.into_inner();

        user::block(database, user.user_id, user_id, block).await?;

        Ok(BlockUserResponse { error: None })
    }

    async fn _is_blocked(
        &self,
        request: Request<IsBlockedRequest>,
    ) -> Result<IsBlockedResponse, Error> {
        let database = self.state.database();
        let user = auth::verify(database, &request).await?;
        let block_user_id = request.into_inner().user_id;

        let is_blocked = user::is_blocked_by(database, user.user_id, block_user_id).await?;

        Ok(IsBlockedResponse {
            result: Some(is_blocked_response::Result::Blocked(is_blocked)),
        })
    }
}

#[tonic::async_trait]
impl UserService for Service {
    async fn user_exists(
        &self,
        request: Request<UserExistsRequest>,
    ) -> Result<Response<UserExistsResponse>, Status> {
        let resp = self
            ._user_exists(request)
            .await
            .unwrap_or_else(|err| UserExistsResponse {
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn auth_user(
        &self,
        request: Request<AuthUserRequest>,
    ) -> Result<Response<AuthUserResponse>, Status> {
        let resp = self
            ._auth_user(request)
            .await
            .unwrap_or_else(|err| AuthUserResponse {
                result: Some(auth_user_response::Result::Error(err.into())),
            });

        Ok(Response::new(resp))
    }

    async fn verify_email(
        &self,
        request: Request<VerifyEmailRequest>,
    ) -> Result<Response<VerifyEmailResponse>, Status> {
        let resp = self
            ._verify_email(request)
            .await
            .unwrap_or_else(|err| VerifyEmailResponse {
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn create_user(
        &self,
        request: Request<CreateUserRequest>,
    ) -> Result<Response<CreateUserResponse>, Status> {
        let resp = self
            ._create_user(request)
            .await
            .unwrap_or_else(|err| CreateUserResponse {
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn delete_user(
        &self,
        request: Request<DeleteUserRequest>,
    ) -> Result<Response<DeleteUserResponse>, Status> {
        let resp = self
            ._delete_user(request)
            .await
            .unwrap_or_else(|err| DeleteUserResponse {
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn update_user(
        &self,
        request: Request<UpdateUserRequest>,
    ) -> Result<Response<UpdateUserResponse>, Status> {
        let resp = self
            ._update_user(request)
            .await
            .unwrap_or_else(|err| UpdateUserResponse {
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn get_user(
        &self,
        request: Request<GetUserRequest>,
    ) -> Result<Response<GetUserResponse>, Status> {
        let resp = self
            ._get_user(request)
            .await
            .unwrap_or_else(|err| GetUserResponse {
                result: Some(get_user_response::Result::Error(err.into())),
            });

        Ok(Response::new(resp))
    }

    async fn search_users(
        &self,
        request: Request<SearchUsersRequest>,
    ) -> Result<Response<SearchUsersResponse>, Status> {
        let resp = self
            ._search_user(request)
            .await
            .unwrap_or_else(|err| SearchUsersResponse {
                users: Vec::new(),
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn block_user(
        &self,
        request: Request<BlockUserRequest>,
    ) -> Result<Response<BlockUserResponse>, Status> {
        let resp = self
            ._block_user(request)
            .await
            .unwrap_or_else(|err| BlockUserResponse {
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn is_blocked(
        &self,
        request: Request<IsBlockedRequest>,
    ) -> Result<Response<IsBlockedResponse>, Status> {
        let resp = self
            ._is_blocked(request)
            .await
            .unwrap_or_else(|err| IsBlockedResponse {
                result: Some(is_blocked_response::Result::Error(err.into())),
            });

        Ok(Response::new(resp))
    }
}
