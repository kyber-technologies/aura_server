use crate::auth;
use crate::error::Error;
use crate::logic::chat;
use crate::state::ServerState;
use crate::types::chat::{Channel, ChannelPermission, Content, Message};
use crate::types::common::Timestamp;
use crate::types::{FastMap, GrpcDomainType};
use aura_rust::chat::v1::chat_service_server::ChatService;
use aura_rust::chat::v1::{
    CreateChannelRequest, CreateChannelResponse, DeleteMessageRequest, DeleteMessageResponse,
    ReadMessagesRequest, ReadMessagesResponse, SendMessageRequest, SendMessageResponse,
    create_channel_response, send_message_response,
};
use aura_rust::common::v1::ErrorCode;
use tonic::{Request, Response, Status};

pub struct Service {
    state: ServerState,
}

impl Service {
    pub fn new(state: ServerState) -> Self {
        Self { state }
    }

    async fn _create_channel(
        &self,
        request: Request<CreateChannelRequest>,
    ) -> Result<CreateChannelResponse, Error> {
        let mut database = self.state.database().await?;

        let (user, _) = auth::verify(&mut database, &request).await?;
        let channel_args = request.into_inner();
        let channel_id = chat::build_channel_id(&mut database).await?;

        let channel = chat::create_channel(
            &mut database,
            Channel {
                channel_id,
                name: channel_args.name,
                description: channel_args.description,
                members: channel_args
                    .members
                    .into_iter()
                    .map(|(user, perm)| {
                        Ok((
                            user,
                            ChannelPermission::from_grpc(
                                aura_rust::chat::v1::ChannelPermission::try_from(perm)
                                    .map_err(|_| Error::invalid_format())?,
                            )?,
                        ))
                    })
                    .collect::<Result<FastMap<_, _>, Error>>()?,
            },
            user.user_id,
        )
        .await?;

        Ok(CreateChannelResponse {
            result: Some(create_channel_response::Result::Channel(
                channel.into_grpc()?,
            )),
        })
    }

    async fn _read_messages(
        &self,
        request: Request<ReadMessagesRequest>,
    ) -> Result<ReadMessagesResponse, Error> {
        let mut database = self.state.database().await?;

        let (user, _) = auth::verify(&mut database, &request).await?;

        let msg_args = request.into_inner();

        let channel = chat::get_channel(&mut database, &msg_args.channel_id)
            .await?
            .ok_or(Error::new(ErrorCode::NotFound, "Channel not found"))?;

        if !channel.members.contains_key(&user.user_id) {
            return Err(Error::new(ErrorCode::Unauthorized, "User not in channel"));
        }

        let messages = chat::read_messages(
            &mut database,
            &msg_args.channel_id,
            msg_args.limit,
            Timestamp::from_grpc(msg_args.start_time.ok_or(Error::invalid_format())?)?,
        )
        .await?;

        Ok(ReadMessagesResponse {
            error: None,
            messages: messages
                .into_iter()
                .map(|m| m.into_grpc())
                .collect::<Result<_, Error>>()?,
        })
    }

    async fn _send_message(
        &self,
        request: Request<SendMessageRequest>,
    ) -> Result<SendMessageResponse, Error> {
        let mut database = self.state.database().await?;

        let (user, _) = auth::verify(&mut database, &request).await?;
        let msg_args = request.into_inner();

        let content = msg_args.content.ok_or(Error::invalid_format())?;

        let perm =
            chat::get_channel_member_perm(&mut database, &msg_args.channel_id, &user.user_id)
                .await?;

        if perm == ChannelPermission::ReadWrite || perm == ChannelPermission::Manager {
            let id = chat::build_message_id(&mut database).await?;
            let msg = chat::send(
                &mut database,
                Message {
                    message_id: id,
                    user_id: user.user_id,
                    channel_id: msg_args.channel_id,
                    content: Content::from_grpc(content)?,
                    created_at: Timestamp::now(),
                },
            )
            .await?;

            Ok(SendMessageResponse {
                result: Some(send_message_response::Result::Message(msg.into_grpc()?)),
            })
        } else {
            Err(Error::new(
                ErrorCode::Unauthorized,
                "User has no permission to send messages",
            ))
        }
    }

    async fn _delete_message(
        &self,
        request: Request<DeleteMessageRequest>,
    ) -> Result<DeleteMessageResponse, Error> {
        let mut database = self.state.database().await?;

        let (user, _) = auth::verify(&mut database, &request).await?;
        let message = chat::get_msg(&mut database, &request.into_inner().message_id)
            .await?
            .ok_or(Error::new(ErrorCode::NotFound, "Message not found"))?;

        let perm = chat::get_channel_member_perm(&mut database, &message.channel_id, &user.user_id)
            .await?;

        if perm == ChannelPermission::Manager
            || (perm == ChannelPermission::ReadWrite && message.user_id == user.user_id)
        {
            chat::delete_message(&mut database, &message.message_id).await?;

            Ok(DeleteMessageResponse { error: None })
        } else {
            Err(Error::new(
                ErrorCode::Unauthorized,
                "User has no permission to delete this message",
            ))
        }
    }
}

#[tonic::async_trait]
impl ChatService for Service {
    async fn create_channel(
        &self,
        request: Request<CreateChannelRequest>,
    ) -> Result<Response<CreateChannelResponse>, Status> {
        let resp =
            self._create_channel(request)
                .await
                .unwrap_or_else(|err| CreateChannelResponse {
                    result: Some(create_channel_response::Result::Error(err.into())),
                });

        Ok(Response::new(resp))
    }

    async fn read_messages(
        &self,
        request: Request<ReadMessagesRequest>,
    ) -> Result<Response<ReadMessagesResponse>, Status> {
        let resp = self
            ._read_messages(request)
            .await
            .unwrap_or_else(|err| ReadMessagesResponse {
                messages: Vec::new(),
                error: Some(err.into()),
            });

        Ok(Response::new(resp))
    }

    async fn send_message(
        &self,
        request: Request<SendMessageRequest>,
    ) -> Result<Response<SendMessageResponse>, Status> {
        let resp = self
            ._send_message(request)
            .await
            .unwrap_or_else(|err| SendMessageResponse {
                result: Some(send_message_response::Result::Error(err.into())),
            });

        Ok(Response::new(resp))
    }

    async fn delete_message(
        &self,
        request: Request<DeleteMessageRequest>,
    ) -> Result<Response<DeleteMessageResponse>, Status> {
        let resp =
            self._delete_message(request)
                .await
                .unwrap_or_else(|err| DeleteMessageResponse {
                    error: Some(err.into()),
                });

        Ok(Response::new(resp))
    }
}
