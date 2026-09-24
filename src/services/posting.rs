use crate::auth;
use crate::error::Error;
use crate::logic::{feed, posting};
use crate::state::ServerState;
use crate::types::GrpcDomainType;
use crate::types::common::Timestamp;
use crate::types::posting::{Post, PostReaction};
use crate::types::resource::Content;
use crate::utils::generate_unique_id;
use aura_rust::posting::v1::posting_service_server::PostingService;
use aura_rust::posting::v1::{
    FeedRequest, FeedResponse, GetOfRequest, GetOfResponse, GetRequest, GetResponse,
    PublishRequest, PublishResponse, ReactRequest, ReactResponse, SearchRequest, SearchResponse,
    UnpublishRequest, UnpublishResponse, publish_response,
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

    async fn _feed(&self, request: Request<FeedRequest>) -> Result<FeedResponse, Error> {
        let mut database = self.state.database().await?;
        let (user, _) = auth::verify(&mut database, &request).await?;
        let req = request.into_inner();

        let user_vector = feed::fetch_user_vector(&mut database, &user.user_id)
            .await
            .map_err(|e| Error::internal(format!("Failed fetching user vector: {e}")))?;

        let post_ids = feed::fetch_feed(
            &mut database,
            &user.user_id,
            user_vector,
            req.limit as usize,
        )
        .await
        .map_err(|e| Error::internal(format!("Failed fetching feed: {e}")))?;

        Ok(FeedResponse {
            post_ids,
            error: None,
        })
    }

    async fn _publish(&self, request: Request<PublishRequest>) -> Result<PublishResponse, Error> {
        let mut database = self.state.database().await?;
        let (user, _) = auth::verify(&mut database, &request).await?;
        let args = request.into_inner();

        let post = posting::create(
            &mut database,
            self.state.embedder(),
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

        posting::delete(&mut database, &args.post_id, &user.user_id).await?;

        Ok(UnpublishResponse { error: None })
    }

    async fn _get(&self, request: Request<GetRequest>) -> Result<GetResponse, Error> {
        let mut database = self.state.database().await?;
        let (user, _) = auth::verify(&mut database, &request).await?;
        let args = request.into_inner();

        let posts = posting::get(&mut database, args.posts.as_slice(), Some(&user.user_id)).await?;

        Ok(GetResponse {
            posts: posts
                .into_iter()
                .map(|(id, post)| Ok((id, post.into_grpc()?)))
                .collect::<Result<std::collections::HashMap<_, _>, Error>>()?,
            error: None,
        })
    }

    async fn _get_of(&self, request: Request<GetOfRequest>) -> Result<GetOfResponse, Error> {
        let mut database = self.state.database().await?;
        let (user, _) = auth::verify(&mut database, &request).await?;
        let args = request.into_inner();

        let posts = posting::get_of(
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

        Ok(GetOfResponse {
            error: None,
            posts: posts
                .into_iter()
                .map(|p| p.into_grpc())
                .collect::<Result<Vec<_>, Error>>()?,
        })
    }

    async fn _search_posts(
        &self,
        request: Request<SearchRequest>,
    ) -> Result<SearchResponse, Error> {
        let mut database = self.state.database().await?;
        let (user, _) = auth::verify(&mut database, &request).await?;
        let args = request.into_inner();

        let posts = posting::search(
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

        Ok(SearchResponse {
            error: None,
            posts: posts
                .into_iter()
                .map(|p| p.into_grpc())
                .collect::<Result<Vec<_>, Error>>()?,
        })
    }

    async fn _react_to_post(&self, request: Request<ReactRequest>) -> Result<ReactResponse, Error> {
        let mut database = self.state.database().await?;
        let (user, _) = auth::verify(&mut database, &request).await?;
        let args = request.into_inner();

        posting::react(
            &mut database,
            &args.post_id,
            &user.user_id,
            PostReaction::from_grpc(args.reaction())?,
        )
        .await?;

        Ok(ReactResponse { error: None })
    }
}

#[tonic::async_trait]
impl PostingService for Service {
    async fn feed(&self, request: Request<FeedRequest>) -> Result<Response<FeedResponse>, Status> {
        let resp = self
            ._feed(request)
            .await
            .unwrap_or_else(|err| FeedResponse {
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

    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let resp = self._get(request).await.unwrap_or_else(|err| GetResponse {
            posts: std::collections::HashMap::new(),
            error: Some(err.into()),
        });

        Ok(Response::new(resp))
    }

    async fn get_of(
        &self,
        request: Request<GetOfRequest>,
    ) -> Result<Response<GetOfResponse>, Status> {
        let resp = self
            ._get_of(request)
            .await
            .unwrap_or_else(|err| GetOfResponse {
                posts: Vec::new(),
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn search(
        &self,
        request: Request<SearchRequest>,
    ) -> Result<Response<SearchResponse>, Status> {
        let resp = self
            ._search_posts(request)
            .await
            .unwrap_or_else(|err| SearchResponse {
                posts: Vec::new(),
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn react(
        &self,
        request: Request<ReactRequest>,
    ) -> Result<Response<ReactResponse>, Status> {
        let resp = self
            ._react_to_post(request)
            .await
            .unwrap_or_else(|err| ReactResponse {
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }
}
