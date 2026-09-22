use crate::auth;
use crate::error::Error;
use crate::logic::{posting, recommendations};
use crate::state::ServerState;
use crate::types::GrpcDomainType;
use crate::types::common::Timestamp;
use crate::types::posting::{Post, PostReaction};
use crate::types::resource::Content;
use crate::utils::generate_unique_id;
use aura_rust::posting::v1::posting_service_server::PostingService;
use aura_rust::posting::v1::{
    GetPostRequest, GetPostResponse, GetPostsOfRequest, GetPostsOfResponse, PublishRequest,
    PublishResponse, ReactToPostRequest, ReactToPostResponse, RecommendationsRequest,
    RecommendationsResponse, SearchPostsRequest, SearchPostsResponse, UnpublishRequest,
    UnpublishResponse, get_post_response, publish_response,
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

    async fn _recommendations(
        &self,
        request: Request<RecommendationsRequest>,
    ) -> Result<RecommendationsResponse, Error> {
        let mut database = self.state.database().await?;
        let (user, _) = auth::verify(&mut database, &request).await?;
        let req = request.into_inner();

        let user_vector = recommendations::fetch_user_vector(&mut database, &user.user_id)
            .await
            .map_err(|e| Error::internal(format!("Failed fetching user vector: {e}")))?;

        let post_ids = recommendations::get_recommendations(
            &mut database,
            &user.user_id,
            user_vector,
            req.limit as usize,
        )
        .await
        .map_err(|e| Error::internal(format!("Failed getting recommendations: {e}")))?;

        Ok(RecommendationsResponse {
            post_ids,
            error: None,
        })
    }

    async fn _publish(&self, request: Request<PublishRequest>) -> Result<PublishResponse, Error> {
        let mut database = self.state.database().await?;
        let mut embedder = self.state.embedder()?;
        let (user, _) = auth::verify(&mut database, &request).await?;
        let args = request.into_inner();

        let post = posting::create_post(
            &mut database,
            &mut embedder,
            Post {
                post_id: generate_unique_id(),
                author_id: user.user_id,
                content: Content::from_grpc(
                    args.content
                        .ok_or(Error::invalid_format("Content not provided"))?,
                )?,
                timestamp: Timestamp::now(),
                parent: args.parent,
                reactions: Default::default(),
                reaction: PostReaction::None,
            },
        )
        .await?;

        Ok(PublishResponse {
            result: Some(publish_response::Result::Post(post.into_grpc()?)),
        })
    }

    async fn _unpublish(
        &self,
        request: Request<UnpublishRequest>,
    ) -> Result<UnpublishResponse, Error> {
        let mut database = self.state.database().await?;
        let (user, _) = auth::verify(&mut database, &request).await?;
        let args = request.into_inner();

        posting::delete_post(&mut database, &args.post_id, &user.user_id).await?;

        Ok(UnpublishResponse { error: None })
    }

    async fn _get_post(&self, request: Request<GetPostRequest>) -> Result<GetPostResponse, Error> {
        let mut database = self.state.database().await?;
        let (user, _) = auth::verify(&mut database, &request).await?;
        let args = request.into_inner();

        let post = posting::get_post(&mut database, &args.post_id, Some(&user.user_id))
            .await?
            .ok_or(Error::not_found("Post not found"))?;

        Ok(GetPostResponse {
            result: Some(get_post_response::Result::Post(post.into_grpc()?)),
        })
    }

    async fn _get_posts_of(
        &self,
        request: Request<GetPostsOfRequest>,
    ) -> Result<GetPostsOfResponse, Error> {
        let mut database = self.state.database().await?;
        let (user, _) = auth::verify(&mut database, &request).await?;
        let args = request.into_inner();

        let posts = posting::get_posts_of(
            &mut database,
            &args.author_id,
            args.limit,
            Timestamp::from_grpc(
                args.start_at
                    .ok_or(Error::invalid_format("Start at not provided"))?,
            )?,
            Some(&user.user_id),
        )
        .await?;

        Ok(GetPostsOfResponse {
            error: None,
            posts: posts
                .into_iter()
                .map(|p| p.into_grpc())
                .collect::<Result<Vec<_>, Error>>()?,
        })
    }

    async fn _search_posts(
        &self,
        request: Request<SearchPostsRequest>,
    ) -> Result<SearchPostsResponse, Error> {
        let mut database = self.state.database().await?;
        let (user, _) = auth::verify(&mut database, &request).await?;
        let args = request.into_inner();

        let posts = posting::search_posts(
            &mut database,
            &args.query,
            args.limit,
            Timestamp::from_grpc(
                args.start_at
                    .ok_or(Error::invalid_format("Start at not provided"))?,
            )?,
            Some(&user.user_id),
        )
        .await?;

        Ok(SearchPostsResponse {
            error: None,
            posts: posts
                .into_iter()
                .map(|p| p.into_grpc())
                .collect::<Result<Vec<_>, Error>>()?,
        })
    }

    async fn _react_to_post(
        &self,
        request: Request<ReactToPostRequest>,
    ) -> Result<ReactToPostResponse, Error> {
        let mut database = self.state.database().await?;
        let (user, _) = auth::verify(&mut database, &request).await?;
        let args = request.into_inner();

        posting::react_to_post(
            &mut database,
            &args.post_id,
            &user.user_id,
            PostReaction::from_grpc(args.reaction())?,
        )
        .await?;

        Ok(ReactToPostResponse { error: None })
    }
}

#[tonic::async_trait]
impl PostingService for Service {
    async fn recommendations(
        &self,
        request: Request<RecommendationsRequest>,
    ) -> Result<Response<RecommendationsResponse>, Status> {
        let resp =
            self._recommendations(request)
                .await
                .unwrap_or_else(|err| RecommendationsResponse {
                    post_ids: Vec::new(),
                    error: Some(err.into()),
                });

        Ok(Response::new(resp))
    }

    async fn publish(
        &self,
        request: Request<PublishRequest>,
    ) -> Result<Response<PublishResponse>, Status> {
        let resp = self
            ._publish(request)
            .await
            .unwrap_or_else(|err| PublishResponse {
                result: Some(publish_response::Result::Error(err.into())),
            });

        Ok(Response::new(resp))
    }

    async fn unpublish(
        &self,
        request: Request<UnpublishRequest>,
    ) -> Result<Response<UnpublishResponse>, Status> {
        let resp = self
            ._unpublish(request)
            .await
            .unwrap_or_else(|err| UnpublishResponse {
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn get_post(
        &self,
        request: Request<GetPostRequest>,
    ) -> Result<Response<GetPostResponse>, Status> {
        let resp = self
            ._get_post(request)
            .await
            .unwrap_or_else(|err| GetPostResponse {
                result: Some(get_post_response::Result::Error(err.into())),
            });

        Ok(Response::new(resp))
    }

    async fn get_posts_of(
        &self,
        request: Request<GetPostsOfRequest>,
    ) -> Result<Response<GetPostsOfResponse>, Status> {
        let resp = self
            ._get_posts_of(request)
            .await
            .unwrap_or_else(|err| GetPostsOfResponse {
                posts: Vec::new(),
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn search_posts(
        &self,
        request: Request<SearchPostsRequest>,
    ) -> Result<Response<SearchPostsResponse>, Status> {
        let resp = self
            ._search_posts(request)
            .await
            .unwrap_or_else(|err| SearchPostsResponse {
                posts: Vec::new(),
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn react_to_post(
        &self,
        request: Request<ReactToPostRequest>,
    ) -> Result<Response<ReactToPostResponse>, Status> {
        let resp = self
            ._react_to_post(request)
            .await
            .unwrap_or_else(|err| ReactToPostResponse {
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }
}
