use crate::error::Error;
use crate::state::ServerState;
use crate::{auth, user};
use aura_rust::common::v1::ErrorCode;
use aura_rust::user::v1::user_service_server::UserService;
use aura_rust::user::v1::{
    AuthUserRequest, AuthUserResponse, CreateUserRequest, CreateUserResponse, DeleteUserRequest,
    DeleteUserResponse, GetUserRequest, GetUserResponse, SearchUsersRequest, SearchUsersResponse,
    UpdateUserAvatarRequest, UpdateUserAvatarResponse, UpdateUserRequest, UpdateUserResponse,
    UserRole, auth_user_response, get_user_response,
};
use aura_rust::{ResourceId, User};
use tonic::{Request, Response, Status};

pub struct Service {
    state: ServerState,
}

impl Service {
    pub fn new(state: ServerState) -> Self {
        Self { state }
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

    async fn _create_user(
        &self,
        request: Request<CreateUserRequest>,
    ) -> Result<CreateUserResponse, Error> {
        let database = self.state.database();

        auth::verify_role(database, &request, UserRole::Admin).await?;

        let mut user = request.into_inner().user.ok_or(Error::invalid_argument())?;

        user.password = auth::hash(user.password).map_err(|err| {
            Error::new(
                ErrorCode::Internal,
                format!("Hashing password failed: {err}"),
            )
        })?;

        user::create(database, User::try_from(user.clone())?).await?;

        Ok(CreateUserResponse { error: None })
    }

    async fn _delete_user(
        &self,
        request: Request<DeleteUserRequest>,
    ) -> Result<DeleteUserResponse, Error> {
        let database = self.state.database();

        auth::verify_role(database, &request, UserRole::Admin).await?;

        let user = request.into_inner().user_id;

        user::delete(database, &user).await?;

        Ok(DeleteUserResponse { error: None })
    }

    async fn _update_user(
        &self,
        request: Request<UpdateUserRequest>,
    ) -> Result<UpdateUserResponse, Error> {
        let database = self.state.database();

        auth::verify_role(database, &request, UserRole::Admin).await?;

        let mut user = request.into_inner().user.ok_or(Error::invalid_argument())?;

        user.password = auth::hash(user.password).map_err(|err| {
            Error::new(
                ErrorCode::Internal,
                format!("Hashing password failed: {err}"),
            )
        })?;

        user::update(database, User::try_from(user)?).await?;

        Ok(UpdateUserResponse { error: None })
    }

    async fn _update_user_avatar(
        &self,
        request: Request<UpdateUserAvatarRequest>,
    ) -> Result<UpdateUserAvatarResponse, Error> {
        let database = self.state.database();

        let user = auth::verify(database, &request).await?;

        let avatar = request
            .into_inner()
            .avatar
            .ok_or(Error::invalid_argument())?;

        let mut user = user::get(database, &user.user_id)
            .await?
            .ok_or(Error::new(ErrorCode::NotFound, "User not found"))?;

        user.icon = ResourceId::try_from(avatar)?;

        user::update(database, user).await?;

        Ok(UpdateUserAvatarResponse { error: None })
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
}

#[tonic::async_trait]
impl UserService for Service {
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

    async fn update_user_avatar(
        &self,
        request: Request<UpdateUserAvatarRequest>,
    ) -> Result<Response<UpdateUserAvatarResponse>, Status> {
        let resp = self
            ._update_user_avatar(request)
            .await
            .unwrap_or_else(|err| UpdateUserAvatarResponse {
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
}
