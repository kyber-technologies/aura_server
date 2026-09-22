use crate::auth;
use crate::error::Error;
use crate::logic::resource;
use crate::state::ServerState;
use crate::types::GrpcDomainType;
use crate::types::common::Timestamp;
use crate::types::resource::{ResourceDescriptor, ResourceId, ResourceMeta, ResourceNamespace};
use crate::utils::{SafeStreaming, generate_unique_id};
use aura_rust::resource::v1::resource_service_server::ResourceService;
use aura_rust::resource::v1::upload_request::Payload;
use aura_rust::resource::v1::{
    DownloadRequest, DownloadResponse, MetaRequest, MetaResponse, UploadRequest, UploadResponse,
    download_response, meta_response, upload_response,
};
use tonic::codegen::BoxStream;
use tonic::codegen::tokio_stream::StreamExt;
use tonic::{Request, Response, Status, Streaming};

pub struct Service {
    state: ServerState,
}

impl Service {
    pub fn new(state: ServerState) -> Self {
        Self { state }
    }

    async fn _upload(
        &self,
        request: Request<Streaming<UploadRequest>>,
    ) -> Result<UploadResponse, Error> {
        let mut database = self.state.database().await?;
        let (user, _) = auth::verify(&mut database, &request).await?;
        let mut stream = SafeStreaming::new(request.into_inner());

        let meta_req = stream
            .next_safe()
            .await
            .ok_or(Error::invalid_format("First request must be meta request"))??;

        let resource_id = ResourceId {
            namespace: ResourceNamespace::from_grpc(
                meta_req
                    .namespace
                    .ok_or(Error::invalid_format("Namespace not provided"))?,
            )?,
            key: generate_unique_id(),
        };

        let mut meta = ResourceMeta::from_grpc(match meta_req
            .payload
            .ok_or(Error::invalid_format("Payload not provided"))?
        {
            Payload::Meta(meta) => Ok(meta),
            Payload::Data(_) => Err(Error::invalid_format("First payload must be meta")),
        }?)?;

        meta.timestamp = Timestamp::now();

        let desc = ResourceDescriptor {
            resource_id: resource_id.clone(),
            meta,
            user_id: user.user_id.clone(),
        };

        if !resource::is_upload_authorized(&mut database, &desc, &user.user_id).await? {
            return Err(Error::restricted("User does not have write permissions"));
        }

        let desc = if let Some(desc) = resource::get(&mut database, &desc.resource_id).await? {
            desc
        } else {
            resource::create(&mut database, desc).await?
        };

        let stream = stream.into_inner().map(|req| match req {
            Ok(req) => match req
                .payload
                .ok_or(Error::invalid_format("Payload not provided"))?
            {
                Payload::Meta(_) => Err(Error::invalid_format(
                    "Meta only allowed in first upload request",
                )),
                Payload::Data(data) => Ok(data),
            },

            Err(err) => Err(Error::internal(format!(
                "Got invalid upload request: {err}"
            ))),
        });

        resource::write(desc.resource_id, stream).await?;

        Ok(UploadResponse {
            result: Some(upload_response::Result::ResourceId(
                resource_id.into_grpc()?,
            )),
        })
    }

    async fn _download(
        &self,
        request: Request<DownloadRequest>,
    ) -> Result<BoxStream<DownloadResponse>, Error> {
        let mut database = self.state.database().await?;

        let (user, _) = auth::verify(&mut database, &request).await?;
        let resource_id = ResourceId::from_grpc(
            request
                .into_inner()
                .resource_id
                .ok_or(Error::invalid_format("Resource ID not provided"))?,
        )?;
        let desc = resource::get(&mut database, &resource_id)
            .await?
            .ok_or(Error::not_found("Resource not found"))?;

        if !resource::is_download_authorized(&mut database, &desc, &user.user_id).await? {
            return Err(Error::restricted("User not permitted"));
        }

        let meta_stream = tokio_stream::once(Ok(DownloadResponse {
            result: Some(download_response::Result::Meta(desc.meta.into_grpc()?)),
        }));

        let stream = resource::read(resource_id).await?.map(|res| match res {
            Ok(data) => Ok(DownloadResponse {
                result: Some(download_response::Result::Data(data.to_vec())),
            }),
            Err(err) => Ok(DownloadResponse {
                result: Some(download_response::Result::Error(err.into())),
            }),
        });

        Ok(Box::pin(meta_stream.chain(stream)))
    }

    async fn _meta(&self, request: Request<MetaRequest>) -> Result<MetaResponse, Error> {
        let mut database = self.state.database().await?;

        let (user, _) = auth::verify(&mut database, &request).await?;
        let resource_id = ResourceId::from_grpc(
            request
                .into_inner()
                .resource_id
                .ok_or(Error::invalid_format("Resource ID not provided"))?,
        )?;

        let desc = resource::get(&mut database, &resource_id)
            .await?
            .ok_or(Error::not_found("Resource not found"))?;

        if !resource::is_download_authorized(&mut database, &desc, &user.user_id).await? {
            return Err(Error::unauthorized("User not authorized"));
        }

        Ok(MetaResponse {
            result: Some(meta_response::Result::Meta(desc.meta.into_grpc()?)),
        })
    }
}

#[tonic::async_trait]
impl ResourceService for Service {
    async fn upload(
        &self,
        request: Request<Streaming<UploadRequest>>,
    ) -> Result<Response<UploadResponse>, Status> {
        let resp = self
            ._upload(request)
            .await
            .unwrap_or_else(|err| UploadResponse {
                result: Some(upload_response::Result::Error(err.into())),
            });

        Ok(Response::new(resp))
    }

    type DownloadStream = BoxStream<DownloadResponse>;

    async fn download(
        &self,
        request: Request<DownloadRequest>,
    ) -> Result<Response<Self::DownloadStream>, Status> {
        let resp = self._download(request).await.unwrap_or_else(|err| {
            Box::pin(tokio_stream::once(Ok(DownloadResponse {
                result: Some(download_response::Result::Error(err.into())),
            })))
        });

        Ok(Response::new(resp))
    }

    async fn meta(&self, request: Request<MetaRequest>) -> Result<Response<MetaResponse>, Status> {
        let resp = self
            ._meta(request)
            .await
            .unwrap_or_else(|err| MetaResponse {
                result: Some(meta_response::Result::Error(err.into())),
            });

        Ok(Response::new(resp))
    }
}
