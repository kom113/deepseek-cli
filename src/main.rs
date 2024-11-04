mod renderer;

use clap::Parser;
use dirs;
use reqwest::header::{HeaderMap, AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Client, Response};
use rustix::process;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Read, Write},
};

use renderer::Renderer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let init_args = CliArgs::parse();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();
    let mut first_prompt = true;

    loop {
        let mut prompt = String::new();

        if first_prompt {
            prompt = init_args.prompt.join(" ");
            first_prompt = false;
        } else {
            print!("You: ");
            io::stdout().flush()?;
            io::stdin().read_line(&mut prompt)?;
        }

        let prompt = prompt.trim();

        if prompt == "exit" {
            break;
        }

        // Load the config from the environment
        let config = Config::from_env().expect("Failed to get API config");

        // load the chatlog for this terminal window
        let chatlog_path = dirs::home_dir()
            .expect("Failed to get home directory")
            .join(".chatgpt")
            .join(
                process::getppid()
                    .expect("Failed to get parent process id")
                    .as_raw_nonzero()
                    .to_string(),
            )
            .join(format!("{}", timestamp))
            .join("chatlog.json");

        fs::create_dir_all(chatlog_path.parent().unwrap())?;

        let mut file = OpenOptions::new()
            .create(true) // create the file if it doesn't exist
            .append(true) // don't overwrite the contents
            .read(true)
            .open(&chatlog_path)
            .unwrap();

        let mut chatlog_text = String::new();
        file.read_to_string(&mut chatlog_text)?;

        // get the messages from the chatlog. limit the total number of tokens
        let messages = get_messages_from_chatlog(&chatlog_text, &prompt)?;

        // send the POST request to OpenAI
        let client = Client::new();
        let data = ModelRequest {
            model: config.model.to_string(),
            stream: config.stream,
            messages,
        };

        let response = send_request_to_openai(&client, data, &config).await?;
        let stream_renderer = Renderer::new();
        let answer = stream_renderer.render(response).await?;

        update_chatlog(chatlog_path.as_path(), prompt, &answer)?;
    }
    Ok(())
}

// get version from Cargo.toml
#[derive(Parser, Debug)]
#[clap(version = env!("CARGO_PKG_VERSION"), author = "Qiuchu Yu<yuqc2001@gmail.com>")]
struct CliArgs {
    /// The prompt to send to ChatGPT
    #[clap(name = "prompt")]
    prompt: Vec<String>,

    /// The ChatGPT model to use (default: gpt-3.5-turbo)
    #[clap(short, long)]
    model: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Log {
    role: String,
    content: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ModelRequest {
    #[serde(rename = "model")]
    model: String,
    #[serde(rename = "stream")]
    stream: bool,
    #[serde(rename = "messages")]
    messages: Vec<Message>,
}

struct Config {
    api_key: String,
    model: String,
    timeout: u64,
    stream: bool,
}

impl Config {
    fn from_env() -> Result<Self, env::VarError> {
        Ok(Config {
            api_key: env::var("DEEPSEEK_API_KEY")?,
            model: env::var("CHATGPT_CLI_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string()),
            timeout: env::var("CHATGPT_CLI_REQUEST_TIMEOUT_SECS")
                .ok()
                .and_then(|x| x.parse().ok())
                .unwrap_or(120),
            stream: true,
        })
    }
}

fn get_messages_from_chatlog(
    chatlog_text: &str,
    prompt: &str,
) -> Result<Vec<Message>, serde_json::Error> {
    let mut messages = vec![];
    if !chatlog_text.is_empty() {
        let chatlog: Vec<Log> = serde_json::from_str(chatlog_text)?;
        for log in chatlog.iter() {
            messages.push(Message {
                role: log.role.clone(),
                content: log.content.clone(),
            });
        }
        messages.push(Message {
            role: "user".to_string(),
            content: prompt.to_string(),
        });
    } else {
        messages.push(Message {
            role: "system".to_string(),
            content: "You are a chatbot.".to_string(),
        });
        messages.push(Message {
            role: "user".to_string(),
            content: prompt.to_string(),
        });
    }
    Ok(messages)
}

async fn send_request_to_openai(
    client: &Client,
    data: ModelRequest,
    config: &Config,
) -> Result<Response, reqwest::Error> {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        format!("Bearer {}", config.api_key).parse().unwrap(),
    );
    headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
    let json_data = serde_json::to_string(&data).unwrap();

    // Send the request and await the response
    let response = client
        .post("https://api.deepseek.com/chat/completions")
        .timeout(Duration::from_secs(config.timeout))
        .headers(headers)
        .body(json_data)
        .send()
        .await?;

    Ok(response)
}

fn update_chatlog(
    chatlog_path: &std::path::Path,
    prompt: &str,
    answer: &str,
) -> std::io::Result<()> {
    // Open the chatlog file and read its content
    let mut file = OpenOptions::new()
        .create(true) // create the file if it doesn't exist
        .read(true)
        .write(true) // we will write back the updated log later
        .open(chatlog_path)?;

    // Read existing content and parse it as JSON
    let mut chatlog_text = String::new();

    file.read_to_string(&mut chatlog_text)?;

    // Parse the existing chatlog or start with an empty Vec if the file is empty
    let mut chatlog: Vec<Log> = vec![];
    if !chatlog_text.is_empty() {
        chatlog = serde_json::from_str(&chatlog_text)?;
    }

    // Append the new logs
    chatlog.push(Log {
        role: "user".to_string(),
        content: prompt.to_string(),
    });
    chatlog.push(Log {
        role: "assistant".to_string(),
        content: answer.to_string(),
    });

    // Serialize the updated chatlog to JSON
    let updated_chatlog_text = serde_json::to_string(&chatlog)?;

    fs::write(chatlog_path, updated_chatlog_text)?;

    Ok(())
}
