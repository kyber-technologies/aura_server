use crate::error::Error;
use crate::resource::ResourceDescriptor;
use crate::state::ServerState;
use crate::utils::{SafeStreaming, VecStream};
use crate::{auth, resource, utils};
use aura_rust::common::v1::ErrorCode;
use aura_rust::resource::v1::resource_service_server::ResourceService;
use aura_rust::resource::v1::upload_request::Payload;
use aura_rust::resource::v1::{
    DownloadRequest, DownloadResponse, GetResourceMetaRequest, GetResourceMetaResponse,
    UploadRequest, UploadResponse, download_response, get_resource_meta_response,
};
use aura_rust::{ResourceId, ResourceMeta};
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
        let database = self.state.database();
        let user = auth::verify(database, &request).await?;
        let mut stream = SafeStreaming::new(request.into_inner());

        let meta_req = stream
            .next_safe()
            .await
            .ok_or(Error::invalid_argument())??;

        let resource_id =
            ResourceId::try_from(meta_req.resource_id.ok_or(Error::invalid_argument())?)?;

        let mut meta =
            ResourceMeta::try_from(match meta_req.payload.ok_or(Error::invalid_argument())? {
                Payload::Meta(meta) => Ok(meta),
                Payload::Data(_) => Err(Error::invalid_argument()),
            }?)?;

        meta.timestamp = utils::get_timestamp();

        let desc = ResourceDescriptor {
            resource_id,
            meta,
            user_id: user.user_id.clone(),
        };

        if !resource::is_upload_authorized(database, &desc, &user.user_id).await? {
            return Err(Error::new(
                ErrorCode::Unauthorized,
                "User does not have write permissions",
            ));
        }

        let desc = if let Some(desc) = resource::get(database, &desc.resource_id).await? {
            desc
        } else {
            resource::create(database, desc).await?
        };

        let stream = stream.into_inner().map(|req| match req {
            Ok(req) => match req.payload.ok_or(Error::invalid_argument())? {
                Payload::Meta(_) => Err(Error::invalid_argument()),
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
        let database = self.state.database();

        let user = auth::verify(database, &request).await?;
        let resource_id = ResourceId::try_from(
            request
                .into_inner()
                .resource_id
                .ok_or(Error::invalid_argument())?,
        )?;
        let desc = resource::get(database, &resource_id)
            .await?
            .ok_or(Error::new(ErrorCode::NotFound, "Resource not found"))?;

        if !resource::is_download_authorized(database, &desc, &user.user_id).await? {
            return Err(Error::new(ErrorCode::Unauthorized, "User not authorized"));
        }

        let meta_stream = VecStream::once(Ok(DownloadResponse {
            result: Some(download_response::Result::Meta(desc.meta.into())),
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
        let database = self.state.database();

        let user = auth::verify(database, &request).await?;
        let resource_id = ResourceId::try_from(
            request
                .into_inner()
                .resource_id
                .ok_or(Error::invalid_argument())?,
        )?;

        let desc = resource::get(database, &resource_id)
            .await?
            .ok_or(Error::new(ErrorCode::NotFound, "Resource not found"))?;

        if !resource::is_download_authorized(database, &desc, &user.user_id).await? {
            return Err(Error::new(ErrorCode::Unauthorized, "User not authorized"));
        }

        Ok(GetResourceMetaResponse {
            result: Some(get_resource_meta_response::Result::Meta(desc.meta.into())),
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
            Box::pin(VecStream::once(Ok(DownloadResponse {
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
