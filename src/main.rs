use std::env;
use std::time::{Duration, SystemTime};

use futures::StreamExt;
use telegram_bot::prelude::*;
use telegram_bot::*;

use std::collections::HashMap;

#[derive(Debug, Clone)]
enum UserQueueState {
    OnboardInitial,
    OnboardInquiryReason,
    OnboardPersonalMessage,
    InQueue,
}

#[derive(Debug, Clone)]
enum UserInquiryReason {
    Unknown,
    Technical,
    Content,
    Press,
    Community,
    Events,
    Emergency,
    Custom(String),
}

#[derive(Debug, Clone)]
struct UserInquiry {
    reason: UserInquiryReason,
    personal_message: Option<String>,
}

#[derive(Debug, Clone)]
struct User {
    user_id: UserId,
    inquiry: UserInquiry,
    last_contact: SystemTime,
    state: UserQueueState,
    messages: Vec<Message>,
}

impl User {
    fn initial(user_id: &UserId) -> User {
        User {
            user_id: user_id.clone(),
            inquiry: UserInquiry{
                reason: UserInquiryReason::Unknown,
                personal_message: None,
            },
            last_contact: SystemTime::now(),
            state: UserQueueState::OnboardInitial,
            messages: Vec::new(),
        }
    }
}

struct Queue {
    order: Vec<UserId>,
    users: HashMap<UserId, User>,
    current: Option<UserId>,
}

impl Queue {
    fn new() -> Queue {
        Queue {
            order: Vec::new(),
            users: HashMap::new(),
            current: None,
        }
    }

    fn user_pos(
        &self,
        user_id: &UserId,
    ) -> isize {
        for (pos, &q_user_id) in self.order.iter().enumerate() {
            if q_user_id == *user_id {
                return pos as isize;
            }
        }

        -1isize
    }

    fn user(
        &mut self,
        user_id: &UserId,
    ) -> &mut User {
        let pos = self.user_pos(user_id);

        let user = self
            .users
            .entry(user_id.clone())
            .or_insert(User::initial(user_id));

        if pos == -1 {
            self.order.push(user_id.clone());
        }

        user
    }
}

struct Artemis {
    arena_chat: SupergroupId,

    api: Api,
    queue: Queue,
}

impl Artemis {
    fn new(
        api: Api,
        arena_chat_id: i64,
    ) -> Artemis {
        Artemis {
            arena_chat: SupergroupId::new(arena_chat_id),

            api,
            queue: Queue::new(),
        }
    }

    async fn handle_update(
        &mut self,
        update: Update,
    ) {
        let update = dbg!(update);

        match update.kind {
            UpdateKind::Message(ref message) => {
                self.handle_update_message(
                        message.clone(),
                        update,
                    )
                    .await;
            },
            _ => {}
        }
    }

    async fn handle_update_message(
        &mut self,
        message: Message,
        update: Update,
    ) {
        {
            let user =
                self.queue
                    .user(&message.from.id);

            user.messages.push(message.clone());
        }

        {
            if let MessageKind::Text { ref data, .. } = &message.kind {
                match &**data {
                    "+accept"
                    | "+reject"
                    | "+done"
                    | "+next"
                    | "+queue"
                    => {
                        self.handle_admin_ticket_command(
                            data.clone(),
                            message,
                            update,
                        )
                            .await;
                    },
                    _ => {
                        self.handle_update_message_text(
                                data.clone(),
                                message,
                                update,
                            )
                            .await;
                    }
                }
            }
        }

        return;
    }

    async fn handle_admin_ticket_command(
        &mut self,
        data: String,
        _message: Message,
        _update: Update,
    ) {
        match &*data {
            "+accept" => {},
            "+reject" => {},
            "+done" => {},
            "+next" => {},
            "+queue" => {},
            _ => {},
        }
    }

    async fn handle_update_message_text(
        &mut self,
        data: String,
        message: Message,
        _update: Update,
    ) {
        let state =
            self.queue
                .user(&message.from.id)
                .clone()
                .state;

        match state {
            UserQueueState::OnboardInitial =>
                self.handle_user_state_onboard_initial(
                    &message
                ).await,
            UserQueueState::OnboardInquiryReason =>
                self.handle_user_state_onboard_inquiry_reason(
                    &message,
                    data,
                ).await,
            UserQueueState::OnboardPersonalMessage =>
                self.handle_user_state_onboard_personal_message(
                    &message,
                    data,
                ).await,
            _ => {}
        }
    }

    async fn handle_user_state_onboard_initial(
        &mut self,
        message: &Message,
    ) {
        let user = self.queue.user(&message.from.id);

        user.state = UserQueueState::OnboardInquiryReason;

        let reply_keyboard = reply_markup!(reply_keyboard, selective, one_time, resize,
            ["technical", "content & substances"],
            ["press & journalists", "community & outreach"],
            ["events & invitations", "emergency"]
        );

        let msg = self.api.send(
            SendMessage::new(
                &message.from.id,
                "Thanks for contacting PsychonautWiki support. Please let us know the nature of your inquiry:",
            )
                .reply_markup(reply_keyboard)
        ).await;

        if let Ok(msg) = msg {
            if let MessageOrChannelPost::Message(msg) = msg {
                user.messages.push(msg.clone())
            }
        }
    }

    async fn handle_user_state_onboard_inquiry_reason(
        &mut self,
        message: &Message,
        data: String,
    ) {
        let user = self.queue.user(&message.from.id);

        let inquiry_reason = match &*data {
            "technical" => UserInquiryReason::Technical,
            "content & substances" => UserInquiryReason::Content,
            "press & journalists" => UserInquiryReason::Press,
            "community & outreach" => UserInquiryReason::Community,
            "events & invitations" => UserInquiryReason::Events,
            "emergency" => UserInquiryReason::Emergency,
            _ => UserInquiryReason::Custom(data),
        };

        user.inquiry.reason = inquiry_reason;

        user.state = UserQueueState::OnboardPersonalMessage;

        let msg = self.api.send(
            SendMessage::new(
                &message.from.id,
                "Thanks. Please give us a short summary of your inquiry:",
            )
        ).await;

        if let Ok(msg) = msg {
            if let MessageOrChannelPost::Message(msg) = msg {
                user.messages.push(msg.clone())
            }
        }
    }

    async fn handle_user_state_onboard_personal_message(
        &mut self,
        message: &Message,
        data: String,
    ) {
        let user = self.queue.user(&message.from.id);

        user.inquiry.personal_message = Some(data);

        user.state = UserQueueState::InQueue;

        for msg in user.messages.iter() {
            self.api.send(msg.delete()).await;
        }

        tokio::time::delay_for(
            Duration::new(0, 2e9 as u32),
        );

        let msg = self.api.send(
            SendMessage::new(
                &message.from.id,
                format!(
                    "Thanks, you're in queue! Please wait for a staff member to accept your request.",
                ),
            )
        ).await;

        if let Ok(msg) = msg {
            if let MessageOrChannelPost::Message(msg) = msg {
                user.messages.push(msg.clone())
            }
        }

        user.messages = Vec::new();
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");

    let arena_chat_id = env::var("ARENA_CHAT_ID")
        .expect("ARENA_CHAT_ID not set")
        .parse::<i64>()
        .expect("ARENA_CHAT_ID should be a number");

    let api = Api::new(token);

    let mut artemis = Artemis::new(
        api.clone(),
        arena_chat_id,
    );

    let mut stream = api.stream();

    while let Some(update) = stream.next().await {
        let update = update?;

        artemis.handle_update(update)
               .await;
    }

    Ok(())
}
