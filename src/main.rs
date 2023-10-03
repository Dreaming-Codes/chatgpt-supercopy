#![windows_subsystem = "windows"]

use std::ops::Deref;
use std::sync::Arc;
use std::thread;
use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::{ChatCompletionRequestMessage, CreateChatCompletionRequestArgs, Role};

use enigo::KeyboardControllable;
use futures_util::stream::StreamExt;
use rdev::{EventType, listen};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;

mod settings;

#[tokio::main]
async fn main() {
    //Load settings from ./settings.json
    let settings = settings::Settings::load().await.expect("Failed to load settings");

    let openai_client = Client::with_config(OpenAIConfig::new().with_api_key(settings.api_key));

    let (schan, mut rchan) = mpsc::unbounded_channel();
    let _listener = thread::spawn(move || {
        listen(move |event| {
            schan
                .send(event)
                .unwrap_or_else(|e| println!("Could not send event: {}", e))
        })
            .expect("Could not create listener");
    });


    let task = tokio::spawn(async move {
        let mut clipboard_handle = arboard::Clipboard::new().expect("Failed to get clipboard");
        let enigo = Arc::new(Mutex::new(enigo::Enigo::new()));
        let mut ctrl_pressed = false;
        let mut write_task: Arc<RwLock<Option<JoinHandle<()>>>> = Arc::new(RwLock::new(None));
        loop {
            let event = rchan.recv().await;
            match event {
                Some(event) => {
                    if let rdev::EventType::KeyPress(key) = event.event_type {
                        if key == rdev::Key::ControlLeft || key == rdev::Key::ControlRight {
                            println!("Ctrl pressed");
                            ctrl_pressed = true;
                        } else if write_task.read().await.is_some() {
                            if key == rdev::Key::Escape {
                                println!("Aborting task");
                                let task = write_task.write().await.take();
                                if let Some(task) = task {
                                    task.abort();
                                }
                            }
                        } else if ctrl_pressed && key == rdev::Key::Space {
                            println!("Ctrl + Space pressed");

                            //Use arboard to get the clipboard contents
                            let clipboard_contents = clipboard_handle.get_text().expect("Failed to get clipboard contents");
                            //Create a new completion request
                            let chat_request = CreateChatCompletionRequestArgs::default()
                                .model(settings.model.clone())
                                .messages(vec![
                                    ChatCompletionRequestMessage {
                                        role: Role::User,
                                        content: Some(clipboard_contents),
                                        name: None,
                                        function_call: None,
                                    }
                                ])
                                .build().unwrap();

                            let Ok(mut stream) = openai_client.chat().create_stream(chat_request).await else {
                                continue;
                            };

                            let task = tokio::spawn({
                                let enigo = enigo.clone();
                                let write_task = write_task.clone();
                                async move {
                                    while let Some(response) = stream.next().await {
                                        if let Ok(response) = response {
                                            //use rdev to write the response simulating key presses
                                            let Some(chars) = &response.choices[0].delta.content else {
                                                continue;
                                            };

                                            for token in chars.chars() {
                                                let text = &token.to_string();

                                                println!("Writing: {}", text);

                                                for char in text.chars() {
                                                    let is_uppercase = char.is_uppercase();

                                                    if is_uppercase {
                                                        enigo.lock().await.key_down(enigo::Key::Shift);
                                                    }
                                                    enigo.lock().await.key_click(enigo::Key::Layout(char.to_lowercase().to_string().chars().next().unwrap()));
                                                    if is_uppercase {
                                                        enigo.lock().await.key_up(enigo::Key::Shift);
                                                    }
                                                    tokio::time::sleep(tokio::time::Duration::from_millis(settings.delay)).await;
                                                }
                                            }
                                        }
                                    }
                                    write_task.write().await.take();
                                }
                            });

                            #[allow(clippy::let_underscore_future)]
                            let _ = write_task.write().await.insert(task);
                        }
                    } else if let rdev::EventType::KeyRelease(key) = event.event_type {
                        if key == rdev::Key::ControlLeft || key == rdev::Key::ControlRight {
                            println!("Ctrl pressed");
                            ctrl_pressed = false;
                            continue;
                        }
                    }
                }
                None => println!("No event received")
            }
        }
    });

    task.await.unwrap();
}
