use std::convert::TryFrom;

use cfg_if::cfg_if;
use ed25519_dalek::{
    PublicKey,
    Signature,
    Verifier,
    PUBLIC_KEY_LENGTH,
    SIGNATURE_LENGTH,
};
use hex::FromHex;
use serde_json::json;
use twilight_model::{
    application::{
        component::{button::ButtonStyle, ActionRow, Button, Component},
        interaction::{
            application_command::{CommandDataOption, CommandOptionValue},
            Interaction,
            InteractionData,
            InteractionType,
        },
    },
    channel::ReactionType,
    http::interaction::{
        InteractionResponse,
        InteractionResponseData,
        InteractionResponseType,
    },
};
use wasm_bindgen::JsValue;
use worker::*;

fn make_interaction_url(id: u64, token: &String) -> String {
    format!(
        "https://discord.com/api/v10/interactions/{}/{}/callback",
        id, token,
    )
}

#[event(fetch)]
pub async fn main(
    req: Request,
    env: Env,
    _ctx: worker::Context,
) -> Result<Response> {
    set_panic_hook();

    Router::new()
        .get("/", |_, _| Response::ok("owo"))
        .post_async("/i", |mut req, ctx| async move {
            let body_bytes = req.bytes().await?;

            let time = match req.headers().get("x-signature-timestamp") {
                Ok(Some(time)) => time,
                _ => return Response::error("", 400),
            };

            let sig_header = match req.headers().get("x-signature-ed25519") {
                Ok(Some(sig)) => {
                    match <[u8; SIGNATURE_LENGTH]>::from_hex(sig) {
                        Ok(b) => b,
                        Err(_) => return Response::error("", 400),
                    }
                },
                _ => return Response::error("", 400),
            };

            let sig = match Signature::try_from(&sig_header[..]) {
                Ok(sig) => sig,
                Err(_) => return Response::error("", 500),
            };

            let public_key = {
                let public_key = ctx.secret("DISCORD_PUBLIC_KEY")?.to_string();

                PublicKey::from_bytes(
                    &<[u8; PUBLIC_KEY_LENGTH]>::from_hex(public_key).unwrap()[..],
                )
                .expect("failed to read public key")
            };

            let is_verified = public_key
                .verify(
                    vec![time.as_bytes(), &body_bytes].concat().as_ref(),
                    &sig,
                )
                .is_ok();

            if !is_verified {
                return Response::error("", 401);
            }

            let body: Interaction =
                serde_json::from_slice(&body_bytes).unwrap();

            if body.kind == InteractionType::Ping {
                return Response::from_json(&json!({"type": 1}));
            }

            if let Some(data) = &body.data {
                match data {
                    InteractionData::ApplicationCommand(cmd) => {
                        match cmd.name.as_str() {
                            "poll" => {
                                match &cmd.options[0] {
                                    // TODO: is there a better way to pattern amtch a `String`
                                    CommandDataOption { name, value: CommandOptionValue::SubCommand(value) } if name == "start" => {
                                        let id = "id-here".to_string();

                                        let topic = value.iter().find(|v| &v.name == "topic").unwrap();
                                        if let CommandOptionValue::String(topic) = &topic.value {

                                            let init = {
                                                let response = {
                                                    let components = Component::ActionRow(ActionRow {
                                                        components: vec![
                                                            Component::Button(
                                                                Button {
                                                                    custom_id: Some(format!("{}:yes", &id)),
                                                                    emoji: Some(ReactionType::Unicode { name: "👍".into() }),
                                                                    label: None,
                                                                    style: ButtonStyle::Success,
                                                                    url: None,
                                                                    disabled: false,
                                                                }
                                                            ),
                                                            Component::Button(
                                                                Button {
                                                                    custom_id: Some(format!("{}:no", &id)),
                                                                    emoji: Some(ReactionType::Unicode { name: "👎".into() }),
                                                                    label: None,
                                                                    style: ButtonStyle::Danger,
                                                                    url: None,
                                                                    disabled: false,
                                                                }
                                                            )
                                                        ]
                                                    });

                                                    InteractionResponse {
                                                        kind:InteractionResponseType::ChannelMessageWithSource,
                                                        data: Some(InteractionResponseData {
                                                            content: Some(topic.clone()),
                                                            components: Some(vec![components]),
                                                            ..Default::default()
                                                        })
                                                    }
                                                };


                                                let req_body_str = serde_json::to_string(&response)?;
                                                let req_body = JsValue::from_str(&req_body_str);

                                                let mut headers = Headers::new();
                                                headers.set("Content-Type", "application/json")?;

                                                let mut init = RequestInit::new();
                                                init.with_method(Method::Post);
                                                init.with_body(Some(req_body));
                                                init.with_headers(headers);

                                                init
                                            };

                                            let url = make_interaction_url(body.id.get(), &body.token);
                                            let outgoing_req = Request::new_with_init(&url, &init)?;

                                            Fetch::Request(outgoing_req).send().await?;
                                        }
                                    },

                                    // CommandDataOption { name, value: CommandOptionValue::SubCommand(value) } if name == "stop" => {
                                    //     let id = value.iter().find(|v| &v.name == "poll-msg-id").unwrap();
                                    // },


                                    &_ => {},
                                }
                            },

                            _ => {},
                        }
                    },

                    InteractionData::MessageComponent(component) => {
                        let msg = body.message.as_ref().unwrap();
                        let msg_id = msg.id.get();

                        console_log!(
                            "button {} pressed on message {}",
                            &component.custom_id,
                            msg_id
                        );
                    },

                    _ => {},
                }
            }

            Response::ok("")
        })
        .run(req, env)
        .await
}

cfg_if! {
    if #[cfg(feature = "console_error_panic_hook")] {
        extern crate console_error_panic_hook;
        pub use self::console_error_panic_hook::set_once as set_panic_hook;
    } else {
        #[inline]
        pub fn set_panic_hook() {}
    }
}
