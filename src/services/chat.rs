use crate::error::Error;
use crate::state::ServerState;
use crate::{auth, chat, utils};
use aura_rust::chat::v1::chat_service_server::ChatService;
use aura_rust::chat::v1::{
    ChannelPermission, CreateChannelRequest, CreateChannelResponse, DeleteMessageRequest,
    DeleteMessageResponse, ReadMessagesRequest, ReadMessagesResponse, SendMessageRequest,
    SendMessageResponse, UpdateMessageRequest, UpdateMessageResponse, create_channel_response,
    send_message_response, update_message_response,
};
use aura_rust::common::v1::ErrorCode;
use aura_rust::{Channel, Content, Message, Timestamp};
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
        let database = self.state.database();

        auth::verify(database, &request).await?;
        let channel_args = request.into_inner();
        let channel_id = chat::build_channel_id(database).await?;

        let channel = chat::create_channel(
            database,
            Channel {
                channel_id,
                name: channel_args.name,
                description: channel_args.description,
                members: channel_args.members,
            },
        )
        .await?;

        Ok(CreateChannelResponse {
            result: Some(create_channel_response::Result::Channel(channel.into())),
        })
    }

    async fn _read_messages(
        &self,
        request: Request<ReadMessagesRequest>,
    ) -> Result<ReadMessagesResponse, Error> {
        let database = self.state.database();

        let user = auth::verify(database, &request).await?;

        let msg_args = request.into_inner();

        let channel = chat::get_channel(database, &msg_args.channel_id)
            .await?
            .ok_or(Error::new(ErrorCode::NotFound, "Channel not found"))?;

        if !channel.members.contains_key(&user.user_id) {
            return Err(Error::new(ErrorCode::Unauthorized, "User not in channel"));
        }

        let messages = chat::read_messages(
            database,
            msg_args.channel_id,
            msg_args.limit,
            Timestamp::try_from(msg_args.start_time.ok_or(Error::invalid_argument())?)?,
        )
        .await?;

        Ok(ReadMessagesResponse {
            error: None,
            messages: messages.into_iter().map(|m| m.into()).collect(),
        })
    }

    async fn _send_message(
        &self,
        request: Request<SendMessageRequest>,
    ) -> Result<SendMessageResponse, Error> {
        let database = self.state.database();

        let user = auth::verify(database, &request).await?;
        let msg_args = request.into_inner();

        let mut content = msg_args.content.ok_or(Error::invalid_argument())?;

        content.created_at = Some(utils::get_timestamp().into());

        let perm =
            chat::get_channel_member_perm(database, &msg_args.channel_id, &user.user_id).await?;

        if perm == ChannelPermission::ReadWrite || perm == ChannelPermission::Manager {
            let id = chat::build_message_id(database).await?;
            let msg = chat::send(
                database,
                Message {
                    message_id: id,
                    user_id: user.user_id,
                    channel_id: msg_args.channel_id,
                    content: content.try_into()?,
                },
            )
            .await?;

            Ok(SendMessageResponse {
                result: Some(send_message_response::Result::Message(msg.into())),
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
        let database = self.state.database();

        let user = auth::verify(database, &request).await?;
        let message = chat::get_msg(database, &request.into_inner().message_id)
            .await?
            .ok_or(Error::new(ErrorCode::NotFound, "Message not found"))?;

        let perm =
            chat::get_channel_member_perm(database, &message.channel_id, &user.user_id).await?;

        if perm == ChannelPermission::Manager
            || (perm == ChannelPermission::ReadWrite && message.user_id == user.user_id)
        {
            chat::delete_message(database, &message.message_id).await?;

            Ok(DeleteMessageResponse { error: None })
        } else {
            Err(Error::new(
                ErrorCode::Unauthorized,
                "User has no permission to delete this message",
            ))
        }
    }

    async fn _update_message(
        &self,
        request: Request<UpdateMessageRequest>,
    ) -> Result<UpdateMessageResponse, Error> {
        let database = self.state.database();

        let user = auth::verify(database, &request).await?;
        let msg_args = request.into_inner();
        let message = chat::get_msg(database, &msg_args.message_id)
            .await?
            .ok_or(Error::new(ErrorCode::NotFound, "Message not found"))?;
        let mut content = Content::try_from(msg_args.content.ok_or(Error::invalid_argument())?)?;

        let perm =
            chat::get_channel_member_perm(database, &message.channel_id, &user.user_id).await?;

        if perm == ChannelPermission::Manager
            || (perm == ChannelPermission::ReadWrite && message.user_id == user.user_id)
        {
            content.created_at = utils::get_timestamp();

            let message = chat::update_message(database, &message.message_id, content).await?;

            Ok(UpdateMessageResponse {
                result: Some(update_message_response::Result::Message(message.into())),
            })
        } else {
            Err(Error::new(
                ErrorCode::Unauthorized,
                "User has no permission to update this message",
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

    async fn update_message(
        &self,
        request: Request<UpdateMessageRequest>,
    ) -> Result<Response<UpdateMessageResponse>, Status> {
        let resp =
            self._update_message(request)
                .await
                .unwrap_or_else(|err| UpdateMessageResponse {
                    result: Some(update_message_response::Result::Error(err.into())),
                });

        Ok(Response::new(resp))
    }
}
