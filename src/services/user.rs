use crate::auth;
use crate::error::{Error, ErrorType};
use crate::logic::{feed, user};
use crate::state::ServerState;
use crate::types::GrpcDomainType;
use crate::types::common::Timestamp;
use crate::types::resource::ResourceId;
use crate::types::user::{Notifications, User, UserRole, UserSettings};
use crate::utils::ConvertHashMap;
use aura_rust::user::v1::user_service_server::UserService;
use aura_rust::user::v1::{
    AuthRequest, AuthResponse, BlockRequest, BlockResponse, CreateRequest, CreateResponse,
    DeleteRequest, DeleteResponse, ExistsRequest, ExistsResponse, FollowRequest, FollowResponse,
    GetRequest, GetResponse, IsBlockedRequest, IsBlockedResponse, SearchRequest, SearchResponse,
    SubscribeRequest, SubscribeResponse, UpdateRequest, UpdateResponse, VerifyEmailRequest,
    VerifyEmailResponse, is_blocked_response,
};
use smol_str::format_smolstr;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tonic::codegen::BoxStream;
use tonic::{Request, Response, Status};

pub struct Service {
    state: ServerState,
}

impl Service {
    pub fn new(state: ServerState) -> Self {
        Self { state }
    }

    async fn _exists(&self, request: Request<ExistsRequest>) -> Result<ExistsResponse, Error> {
        self.state.throttle(&request, "user.exists")?;

        let user_id = request.into_inner().user_id;
        let exists = user::exists(&mut self.state.database().await?, &user_id).await?;

        Ok(ExistsResponse {
            error: if exists {
                None
            } else {
                Some(Error::not_found("User not found").into())
            },
        })
    }

    async fn _auth(&self, request: Request<AuthRequest>) -> Result<AuthResponse, Error> {
        self.state.throttle(&request, "user.auth")?;

        let mut database = self.state.database().await?;
        let verify = auth::verify(&mut database, &request).await;
        let args = request.into_inner();

        match verify {
            Ok((user, token)) => Ok(AuthResponse {
                token,
                user: Some(user.into_grpc()?),
                error: None,
            }),
            Err(e) => {
                if let ErrorType::Unauthorized(_) = e.ty
                    && let Some(user_id) = args.user_id
                    && let Some(password) = args.password
                {
                    let (token, user) = auth::auth(&mut database, user_id, password).await?;

                    Ok(AuthResponse {
                        token,
                        user: Some(user.into_grpc()?),
                        error: None,
                    })
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn _subscribe(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<BoxStream<SubscribeResponse>, Status> {
        let mut database = self
            .state
            .database()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let (user, _) = auth::verify(&mut database, &request)
            .await
            .map_err(|e| Status::unauthenticated(e.to_string()))?;

        let rx = self
            .state
            .notifications()
            .subscribe(&user.user_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let stream = tokio_stream::wrappers::BroadcastStream::new(rx).map(|result| match result {
            Ok(notification) => match notification.into_grpc() {
                Ok(notification) => Ok(SubscribeResponse {
                    notifications: vec![notification],
                    error: None,
                }),
                Err(err) => Ok(SubscribeResponse {
                    notifications: Vec::new(),
                    error: Some(err.into()),
                }),
            },

            Err(BroadcastStreamRecvError::Lagged(skipped)) => Ok(SubscribeResponse {
                notifications: Vec::new(),
                error: Some(
                    Error::internal(format_smolstr!(
                        "Stream lagged, skipped {skipped} notifications"
                    ))
                    .into(),
                ),
            }),
        });

        Ok(Box::pin(stream))
    }

    async fn _verify_email(
        &self,
        request: Request<VerifyEmailRequest>,
    ) -> Result<VerifyEmailResponse, Error> {
        self.state.throttle(&request, "user.verify_email")?;

        let args = request.into_inner();

        self.state.emails().register_email(args.email).await?;

        Ok(VerifyEmailResponse { error: None })
    }

    async fn _create(&self, request: Request<CreateRequest>) -> Result<CreateResponse, Error> {
        self.state.throttle(&request, "user.create")?;

        let args = request.into_inner();

        if args.email.is_empty()
            || args.password.is_empty()
            || args.user_id.is_empty()
            || args.username.is_empty()
            || args.verification_token.is_empty()
        {
            return Err(Error::invalid_format(
                "Please specify: email, password, user_id, username and verification_token.",
            ));
        }

        self.state
            .emails()
            .verify_email(&args.email, args.verification_token)?;

        let mut database = self.state.database().await?;

        let settings = args
            .settings
            .ok_or(Error::invalid_format("Settings not provided"))?;

        if settings.reset_algo_tags.is_empty() {
            return Err(Error::invalid_format(
                "Reset algorithm tags cannot be empty",
            ));
        }

        let tags = settings.reset_algo_tags.clone();

        let settings = UserSettings::from_grpc(settings)?;

        let mut user = User {
            user_id: args.user_id.clone(),
            username: args.username,
            email: args.email.clone(),
            about: String::new(),
            password: args.password,
            role: UserRole::User,
            created_at: Timestamp::now(),
            icon: ResourceId::default_user_icon(),
            notifications: Notifications(Vec::new()),
            settings: settings.clone(),
            channels: Vec::new(),
            followers: Vec::new(),
            following: Vec::new(),
        };

        user.password = auth::hash(user.password)?;

        user::create(&mut database, user).await?;

        feed::init_user_vector(&mut database, self.state.embedder(), &args.user_id, tags).await?;

        Ok(CreateResponse { error: None })
    }

    async fn _delete(&self, request: Request<DeleteRequest>) -> Result<DeleteResponse, Error> {
        self.state.throttle(&request, "user.delete")?;

        let mut database = self.state.database().await?;

        let (user, _) = auth::verify(&mut database, &request).await?;
        let password = request.into_inner().password;

        auth::auth(&mut database, user.user_id.clone(), password).await?;

        user::delete(&mut database, &user.user_id).await?;

        Ok(DeleteResponse { error: None })
    }

    async fn _update(&self, request: Request<UpdateRequest>) -> Result<UpdateResponse, Error> {
        self.state.throttle(&request, "user.update")?;

        let mut database = self.state.database().await?;

        let (mut user, _) = auth::verify(&mut database, &request).await?;
        let args = request.into_inner();

        let settings = args
            .settings
            .ok_or(Error::invalid_format("Settings not provided"))?;

        let reset_algo_tags = settings.reset_algo_tags.clone();

        let settings = UserSettings::from_grpc(settings)?;

        if !reset_algo_tags.is_empty() {
            feed::reset_user_vector(&mut database, &user.user_id).await?;
            feed::init_user_vector(
                &mut database,
                self.state.embedder(),
                &user.user_id,
                reset_algo_tags,
            )
            .await?;
        }

        if let Some(email) = args.email {
            let token = args
                .token
                .ok_or(Error::invalid_format("Token not provided"))?;

            self.state.emails().verify_email(&email, token)?;

            user.email = email;
        }

        user.username = args.username.unwrap_or(user.username);
        user.about = args.about.unwrap_or(user.about);
        user.password = args
            .password
            .map(auth::hash)
            .unwrap_or(Ok(user.password))
            .map_err(|err| Error::internal(format_smolstr!("Hashing password failed: {err}")))?;
        user.settings = settings;

        user::update(&mut database, user).await?;

        Ok(UpdateResponse { error: None })
    }

    async fn _get(&self, request: Request<GetRequest>) -> Result<GetResponse, Error> {
        self.state.throttle(&request, "user.get")?;

        let mut database = self.state.database().await?;

        auth::verify(&mut database, &request).await?;
        let user = request.into_inner().user_id;

        let users = user::get(&mut database, &user).await?;

        Ok(GetResponse {
            users: users.map_convert(|id, user| Ok((id, user.into_profile().into_grpc()?)))?,
            error: None,
        })
    }

    async fn _search(&self, request: Request<SearchRequest>) -> Result<SearchResponse, Error> {
        self.state.throttle(&request, "user.search")?;

        let mut database = self.state.database().await?;
        auth::verify(&mut database, &request).await?;
        let args = request.into_inner();

        let users = user::search(&mut database, &args.query, args.limit as i64)
            .await?
            .into_iter()
            .map(|u| u.into_grpc())
            .collect::<Result<_, Error>>()?;

        Ok(SearchResponse { users, error: None })
    }

    async fn _block(&self, request: Request<BlockRequest>) -> Result<BlockResponse, Error> {
        self.state.throttle(&request, "user.block")?;

        let mut database = self.state.database().await?;
        let (user, _) = auth::verify(&mut database, &request).await?;
        let BlockRequest { user_id, block } = request.into_inner();

        user::block(&mut database, &user.user_id, &user_id, block).await?;

        Ok(BlockResponse { error: None })
    }

    async fn _is_blocked(
        &self,
        request: Request<IsBlockedRequest>,
    ) -> Result<IsBlockedResponse, Error> {
        self.state.throttle(&request, "user.is_blocked")?;

        let mut database = self.state.database().await?;
        let (user, _) = auth::verify(&mut database, &request).await?;
        let block_user_id = request.into_inner().user_id;

        let is_blocked = user::is_blocked_by(&mut database, &user.user_id, &block_user_id).await?;

        Ok(IsBlockedResponse {
            result: Some(is_blocked_response::Result::Blocked(is_blocked)),
        })
    }

    async fn _follow(&self, request: Request<FollowRequest>) -> Result<FollowResponse, Error> {
        self.state.throttle(&request, "user.follow")?;

        let mut database = self.state.database().await?;
        let (user, _) = auth::verify(&mut database, &request).await?;
        let args = request.into_inner();

        if args.unfollow {
            user::unfollow(&mut database, &user.user_id, &args.user_id).await?;
        } else {
            user::follow(&mut database, &user.user_id, &args.user_id).await?;
        }

        Ok(FollowResponse { error: None })
    }
}

#[tonic::async_trait]
impl UserService for Service {
    async fn exists(
        &self,
        request: Request<ExistsRequest>,
    ) -> Result<Response<ExistsResponse>, Status> {
        let resp = self
            ._exists(request)
            .await
            .unwrap_or_else(|err| ExistsResponse {
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn auth(&self, request: Request<AuthRequest>) -> Result<Response<AuthResponse>, Status> {
        let resp = self
            ._auth(request)
            .await
            .unwrap_or_else(|err| AuthResponse {
                token: String::new(),
                user: None,
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    type SubscribeStream = BoxStream<SubscribeResponse>;

    async fn subscribe(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let stream = self._subscribe(request).await?;
        Ok(Response::new(stream))
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

    async fn create(
        &self,
        request: Request<CreateRequest>,
    ) -> Result<Response<CreateResponse>, Status> {
        let resp = self
            ._create(request)
            .await
            .unwrap_or_else(|err| CreateResponse {
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn delete(
        &self,
        request: Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let resp = self
            ._delete(request)
            .await
            .unwrap_or_else(|err| DeleteResponse {
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn update(
        &self,
        request: Request<UpdateRequest>,
    ) -> Result<Response<UpdateResponse>, Status> {
        let resp = self
            ._update(request)
            .await
            .unwrap_or_else(|err| UpdateResponse {
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let resp = self._get(request).await.unwrap_or_else(|err| GetResponse {
            users: std::collections::HashMap::new(),
            error: Some(err.into()),
        });

        Ok(Response::new(resp))
    }

    async fn search(
        &self,
        request: Request<SearchRequest>,
    ) -> Result<Response<SearchResponse>, Status> {
        let resp = self
            ._search(request)
            .await
            .unwrap_or_else(|err| SearchResponse {
                users: Vec::new(),
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn block(
        &self,
        request: Request<BlockRequest>,
    ) -> Result<Response<BlockResponse>, Status> {
        let resp = self
            ._block(request)
            .await
            .unwrap_or_else(|err| BlockResponse {
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

    async fn follow(
        &self,
        request: Request<FollowRequest>,
    ) -> Result<Response<FollowResponse>, Status> {
        let resp = self
            ._follow(request)
            .await
            .unwrap_or_else(|err| FollowResponse {
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }
}
