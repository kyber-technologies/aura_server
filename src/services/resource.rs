use crate::auth;
use crate::error::Error;
use crate::logic::resource;
use crate::state::ServerState;
use crate::types::GrpcDomainType;
use crate::types::common::Timestamp;
use crate::types::resource::{ResourceDescriptor, ResourceId, ResourceMeta};
use crate::utils::SafeStreaming;
use aura_rust::common::v1::ErrorCode;
use aura_rust::resource::v1::resource_service_server::ResourceService;
use aura_rust::resource::v1::upload_request::Payload;
use aura_rust::resource::v1::{
    DownloadRequest, DownloadResponse, GetResourceMetaRequest, GetResourceMetaResponse,
    UploadRequest, UploadResponse, download_response, get_resource_meta_response,
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
        let user = auth::verify(&mut database, &request).await?;
        let mut stream = SafeStreaming::new(request.into_inner());

        let meta_req = stream.next_safe().await.ok_or(Error::invalid_format())??;

        let resource_id =
            ResourceId::from_grpc(meta_req.resource_id.ok_or(Error::invalid_format())?)?;

        let mut meta =
            ResourceMeta::from_grpc(match meta_req.payload.ok_or(Error::invalid_format())? {
                Payload::Meta(meta) => Ok(meta),
                Payload::Data(_) => Err(Error::invalid_format()),
            }?)?;

        meta.timestamp = Timestamp::now();

        let desc = ResourceDescriptor {
            resource_id,
            meta,
            user_id: user.user_id.clone(),
        };

        if !resource::is_upload_authorized(&mut database, &desc, &user.user_id).await? {
            return Err(Error::new(
                ErrorCode::Unauthorized,
                "User does not have write permissions",
            ));
        }

        let desc = if let Some(desc) = resource::get(&mut database, &desc.resource_id).await? {
            desc
        } else {
            resource::create(&mut database, desc).await?
        };

        let stream = stream.into_inner().map(|req| match req {
            Ok(req) => match req.payload.ok_or(Error::invalid_format())? {
                Payload::Meta(_) => Err(Error::invalid_format()),
                Payload::Data(data) => Ok(data),
            },

            Err(err) => Err(Error::new(
                ErrorCode::Internal,
                format!("Got invalid upload request: {err}"),
            )),
        });

        resource::write(desc.resource_id, stream).await?;

        Ok(UploadResponse { error: None })
    }

    async fn _download(
        &self,
        request: Request<DownloadRequest>,
    ) -> Result<BoxStream<DownloadResponse>, Error> {
        let mut database = self.state.database().await?;

        let user = auth::verify(&mut database, &request).await?;
        let resource_id = ResourceId::from_grpc(
            request
                .into_inner()
                .resource_id
                .ok_or(Error::invalid_format())?,
        )?;
        let desc = resource::get(&mut database, &resource_id)
            .await?
            .ok_or(Error::new(ErrorCode::NotFound, "Resource not found"))?;

        if !resource::is_download_authorized(&mut database, &desc, &user.user_id).await? {
            return Err(Error::new(ErrorCode::Unauthorized, "User not authorized"));
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

    async fn _get_resource_meta(
        &self,
        request: Request<GetResourceMetaRequest>,
    ) -> Result<GetResourceMetaResponse, Error> {
        let mut database = self.state.database().await?;

        let user = auth::verify(&mut database, &request).await?;
        let resource_id = ResourceId::from_grpc(
            request
                .into_inner()
                .resource_id
                .ok_or(Error::invalid_format())?,
        )?;

        let desc = resource::get(&mut database, &resource_id)
            .await?
            .ok_or(Error::new(ErrorCode::NotFound, "Resource not found"))?;

        if !resource::is_download_authorized(&mut database, &desc, &user.user_id).await? {
            return Err(Error::new(ErrorCode::Unauthorized, "User not authorized"));
        }

        Ok(GetResourceMetaResponse {
            result: Some(get_resource_meta_response::Result::Meta(
                desc.meta.into_grpc()?,
            )),
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
                error: Some(err.into()),
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

    async fn get_resource_meta(
        &self,
        request: Request<GetResourceMetaRequest>,
    ) -> Result<Response<GetResourceMetaResponse>, Status> {
        let resp = self
            ._get_resource_meta(request)
            .await
            .unwrap_or_else(|err| GetResourceMetaResponse {
                result: Some(get_resource_meta_response::Result::Error(err.into())),
            });

        Ok(Response::new(resp))
    }
}
