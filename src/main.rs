extern crate futures;
extern crate telegram_bot;
extern crate tokio_core;

use std::env;

use futures::Stream;
use telegram_bot::*;
use tokio_core::reactor::Core;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Mutex;
use std::collections::HashMap;

enum QueueState {
    OnboardInitial,
    OnboardInquiryReason,
    OnboardPersonalMessage,
    InQueue,
}

enum UserInquiryReason {
    Unknown,
    Technical,
    Content,
    Press,
    Community,
    Events,
    Emergency,
}

struct UserInquiry {
    reason: UserInquiryReason,
    personal_message: Option<String>,
}

struct User {
    user_id: UserId,
    inquiry: UserInquiry,
    last_contact: SystemTime,
    state: QueueState,
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
            state: QueueState::OnboardInitial,
        }
    }
}

struct Queue {
    order: Vec<UserId>,
    users: HashMap<UserId, User>,
}

impl Queue {
    fn new() -> Queue {
        Queue {
            order: Vec::new(),
            users: HashMap::new(),
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
        user_id: UserId,
    ) -> (usize, &User) {
        let mut pos = self.user_pos(&user_id);

        let user = self
            .users
            .entry(user_id)
            .or_insert(User::initial(&user_id));

        match pos {
            -1 => {
                self.order.push(user_id);

                (self.order.len(), &*user)
            },
            _ => (pos as usize, &*user)
        }
    }
}

fn something(api: &Api) {
    let user = UserId::new(51594512);

    let reply_keyboard = reply_markup!(reply_keyboard, selective, one_time, resize,
            ["technical", "content & substances"],
            ["press & journalists", "community & outreach"],
            ["events & invitations", "emergency"]
        );

    api.spawn(
        SendMessage::new(
            user,
            "Thanks for contacting PsychonautWiki support. Please let us know the nature of your inquiry:",
        )
            .reply_markup(reply_keyboard)
    );
}

struct Artemis {
    api: Api,
    queue: Queue,
}

impl Artemis {
    fn new(api: Api) -> Artemis {
        Artemis {
            api,
            queue: Queue::new(),
        }
    }

    fn handle_update(
        &self,
        update: Update,
    ) {
        let update = dbg!(update);

        match update.kind {
            UpdateKind::Message(ref message) => {
                self.handle_update_message(
                    message.clone(),
                    update,
                );
            },
            _ => {}
        }
    }

    fn handle_update_message(
        &self,
        message: Message,
        update: Update,
    ) {
        if let MessageKind::Text { ref data, .. } = &message.kind {
            self.handle_update_message_text(
                data.clone(),
                message,
                update,
            );

            return;
        }

        if let MessageKind::NewChatMembers { ref data, .. } = &message.kind {
            self.handle_update_message_new_members(
                data.iter().map(|user| user.id.clone()).collect(),
                message,
                update,
            );

            return;
        }
    }

    fn handle_update_message_new_members(
        &self,
        data: Vec<UserId>,
        message: Message,
        update: Update,
    ) {
    }

    fn handle_update_message_text(
        &self,
        data: String,
        message: Message,
        update: Update,
    ) {
        self.api.spawn(
            message
                .text_reply(
                    format!(
                        "Hi, {}! You just wrote '{}'",
                        &message.from.first_name, data
                    )
                )
        );
    }
}

fn run(token: String) {
    let mut core = Core::new().unwrap();

    let api = Api::configure(token)
        .build(core.handle())
        .unwrap();

    let artemis = Artemis::new(api.clone());

    let artm_mtx = Mutex::new(artemis);

    let future =
        api
            .stream()
            .for_each(|update| {
                artm_mtx
                    .lock()
                    .unwrap()
                    .handle_update(update);

                Ok(())
        });

    core.run(future).unwrap();
}

fn main() {
    let token = env::var("TELEGRAM_BOT_TOKEN")
        .expect("TELEGRAM_BOT_TOKEN not set");

    run(token);
}
